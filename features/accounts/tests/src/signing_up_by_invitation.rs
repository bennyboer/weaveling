use accounts_core::{
    AccountEvent, AccountId, AccountServiceError, EmailAddress, EmailAddresses, InvitationTicket,
    InvitationTickets, InvitationToken, KIND, SigningUp,
};
use eventsourcing::{AggregateId, EventStore, Version};
use time::Duration;

use crate::wiring;
use crate::wiring::{ADA, HER_PASSWORD, Wired};

const TOKEN: &str = "a token drawn by invitations";

async fn invited(wired: &Wired, address: &str, token: &str) {
    wired
        .secrets
        .keep_ticket(
            &InvitationToken::presented(token).digest(),
            &InvitationTicket {
                invitation: "invitation_1".to_owned(),
                to: EmailAddress::new(address).expect("a plain address"),
                until: wiring::at(1_700_000_000) + Duration::days(7),
            },
        )
        .await
        .expect("keeping should succeed");
}

async fn signs_up(wired: &Wired, token: &str) -> Result<(), AccountServiceError> {
    wired
        .accounts
        .sign_up_by_invitation(token, "Ada", HER_PASSWORD)
        .await
        .map(|_| ())
}

async fn holder(wired: &Wired, address: &str) -> Option<AccountId> {
    wired
        .secrets
        .holder(&EmailAddress::new(address).expect("a plain address"))
        .await
        .expect("reading should succeed")
}

#[tokio::test]
async fn an_invitation_signs_its_address_up_confirmed_and_ready_to_sign_in() {
    let wired = wiring::wired();
    invited(&wired, ADA, TOKEN).await;

    let (session, signed_in) = wired
        .accounts
        .sign_up_by_invitation(TOKEN, "Ada", HER_PASSWORD)
        .await
        .expect("an open invitation lets its address sign up");

    let id = holder(&wired, ADA).await.expect("the address is held");
    assert_eq!(signed_in.session.account, id);
    assert_eq!(signed_in.account.email().as_str(), ADA);
    assert_eq!(
        wired
            .accounts
            .signed_in(&session)
            .await
            .expect("reading should succeed")
            .map(|signed_in| signed_in.session.account),
        Some(id),
        "whoever followed the link is signed in, needing nothing more"
    );
    assert_eq!(
        wired
            .accounts
            .sign_in(ADA, HER_PASSWORD)
            .await
            .expect("no code is owed"),
        id
    );
    assert!(
        wired.mailbox.sent_to(ADA).is_empty(),
        "the link reached the address already, so no code is mailed"
    );
    let told = wired
        .store
        .read_from(&AggregateId::from(&id), KIND, Version::of(1))
        .await
        .expect("reading should succeed");
    assert!(matches!(
        &told[0].event,
        AccountEvent::SignedUpByInvitation { invitation, .. } if invitation == "invitation_1"
    ));
}

#[tokio::test]
async fn a_token_copied_with_space_around_it_is_the_same_token() {
    let wired = wiring::wired();
    invited(&wired, ADA, TOKEN).await;

    signs_up(&wired, &format!("  {TOKEN}\n"))
        .await
        .expect("a token is read as it was meant");
}

#[tokio::test]
async fn an_invitation_lets_one_account_in() {
    let wired = wiring::wired();
    invited(&wired, ADA, TOKEN).await;
    signs_up(&wired, TOKEN)
        .await
        .expect("the first sign-up takes it");

    assert!(matches!(
        signs_up(&wired, TOKEN).await,
        Err(AccountServiceError::InvitationInvalid)
    ));
}

#[tokio::test]
async fn a_token_nobody_was_given_or_one_withdrawn_signs_nobody_up() {
    let wired = wiring::wired();
    invited(&wired, ADA, TOKEN).await;
    wired
        .secrets
        .forget_ticket("invitation_1")
        .await
        .expect("forgetting should succeed");

    for token in [TOKEN, "a token nobody was given"] {
        assert!(matches!(
            signs_up(&wired, token).await,
            Err(AccountServiceError::InvitationInvalid)
        ));
    }
    assert_eq!(holder(&wired, ADA).await, None);
}

#[tokio::test]
async fn an_invitation_holds_to_its_last_moment_and_no_longer() {
    let last = wiring::at(1_700_000_000) + Duration::days(7);
    let on_time = wiring::wired();
    invited(&on_time, ADA, TOKEN).await;
    on_time.clock.set(last);
    let late = wiring::wired();
    invited(&late, ADA, TOKEN).await;
    late.clock.set(last + Duration::seconds(1));

    signs_up(&on_time, TOKEN)
        .await
        .expect("an invitation still holds at its last moment");
    assert!(matches!(
        signs_up(&late, TOKEN).await,
        Err(AccountServiceError::InvitationExpired)
    ));
    assert_eq!(holder(&late, ADA).await, None);
}

#[tokio::test]
async fn a_name_or_password_refused_leaves_the_invitation_to_be_used() {
    let wired = wiring::wired();
    invited(&wired, ADA, TOKEN).await;

    assert!(matches!(
        wired
            .accounts
            .sign_up_by_invitation(TOKEN, "  ", HER_PASSWORD)
            .await,
        Err(AccountServiceError::InvalidName(_))
    ));
    assert!(matches!(
        wired
            .accounts
            .sign_up_by_invitation(TOKEN, "Ada", "short")
            .await,
        Err(AccountServiceError::InvalidPassword(_))
    ));

    signs_up(&wired, TOKEN)
        .await
        .expect("what was typed wrong is checked before the invitation is taken");
}

#[tokio::test]
async fn an_address_with_an_account_already_is_told_to_sign_in() {
    let wired = wiring::wired();
    wired
        .accounts
        .sign_up(ADA, "Ada", HER_PASSWORD)
        .await
        .expect("signing up should succeed");
    let first = holder(&wired, ADA).await;
    invited(&wired, ADA, TOKEN).await;

    assert!(matches!(
        signs_up(&wired, TOKEN).await,
        Err(AccountServiceError::EmailAddressHasAccount)
    ));
    assert_eq!(
        holder(&wired, ADA).await,
        first,
        "the account there stays the one there"
    );
}

#[tokio::test]
async fn where_signing_up_is_by_invitation_only_an_invitation_lets_anyone_in() {
    let wired = wiring::wired();
    let accounts = wired.accounts.clone().signing_up(SigningUp::ByInvitation);
    invited(&wired, ADA, TOKEN).await;

    assert!(matches!(
        accounts
            .sign_up("bob@example.com", "Bob", HER_PASSWORD)
            .await,
        Err(AccountServiceError::SigningUpByInvitationOnly)
    ));
    assert_eq!(holder(&wired, "bob@example.com").await, None);
    assert!(
        wired.mailbox.sent_to("bob@example.com").is_empty(),
        "nothing is mailed to whoever was not asked"
    );
    assert!(
        matches!(
            accounts.sign_up(ADA, "Ada", HER_PASSWORD).await,
            Err(AccountServiceError::SigningUpByInvitationOnly)
        ),
        "even an invited address comes in by its link, not around it"
    );

    accounts
        .sign_up_by_invitation(TOKEN, "Ada", HER_PASSWORD)
        .await
        .expect("an invitation is the way in");
}
