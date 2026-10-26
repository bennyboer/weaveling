use accounts_core::{
    AccountEvent, AccountId, AccountsError, EmailAddress, EmailAddresses, InvitationScope,
    InvitationTicket, InvitationTickets, InvitationToken, KIND, SigningUp,
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
                scope: InvitationScope::Instance,
                by_an_operator: true,
            },
        )
        .await
        .expect("keeping should succeed");
}

async fn signs_up(wired: &Wired, token: &str) -> Result<(), AccountsError> {
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
        AccountEvent::SignedUpByInvitation { email, .. } if email.as_str() == ADA
    ));
    assert!(matches!(
        &told[1].event,
        AccountEvent::InvitationTakenUp { invitation } if invitation == "invitation_1"
    ));
    assert_eq!(
        told[1].metadata.agent,
        id.acting(),
        "the new account takes it up itself"
    );
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
        Err(AccountsError::InvitationInvalid)
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
            Err(AccountsError::InvitationInvalid)
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
        Err(AccountsError::InvitationExpired)
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
        Err(AccountsError::InvalidName(_))
    ));
    assert!(matches!(
        wired
            .accounts
            .sign_up_by_invitation(TOKEN, "Ada", "short")
            .await,
        Err(AccountsError::InvalidPassword(_))
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
        Err(AccountsError::EmailAddressHasAccount)
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
        Err(AccountsError::SigningUpByInvitationOnly)
    ));
    assert_eq!(holder(&wired, "bob@example.com").await, None);
    assert!(
        wired.mailbox.sent_to("bob@example.com").is_empty(),
        "nothing is mailed to whoever was not asked"
    );
    assert!(
        matches!(
            accounts.sign_up(ADA, "Ada", HER_PASSWORD).await,
            Err(AccountsError::SigningUpByInvitationOnly)
        ),
        "even an invited address comes in by its link, not around it"
    );

    accounts
        .sign_up_by_invitation(TOKEN, "Ada", HER_PASSWORD)
        .await
        .expect("an invitation is the way in");
}

fn into_a_project(by_an_operator: bool) -> InvitationTicket {
    InvitationTicket {
        invitation: "invitation_1".to_owned(),
        to: EmailAddress::new(ADA).expect("a plain address"),
        until: wiring::at(1_700_000_000) + Duration::days(7),
        scope: InvitationScope::Project {
            project: "project_loom".to_owned(),
        },
        by_an_operator,
    }
}

async fn invited_into_a_project(wired: &Wired, by_an_operator: bool) {
    wired
        .secrets
        .keep_ticket(
            &InvitationToken::presented(TOKEN).digest(),
            &into_a_project(by_an_operator),
        )
        .await
        .expect("keeping should succeed");
}

async fn still_there(wired: &Wired) -> Option<InvitationTicket> {
    wired
        .secrets
        .find_ticket(&InvitationToken::presented(TOKEN).digest())
        .await
        .expect("finding should succeed")
}

#[tokio::test]
async fn where_anyone_may_sign_up_a_project_s_invitation_signs_its_address_up_and_is_taken_up() {
    let wired = wiring::wired();
    invited_into_a_project(&wired, false).await;

    signs_up(&wired, TOKEN)
        .await
        .expect("an owner may bring in whom anyone could bring in");

    let id = holder(&wired, ADA).await.expect("the address is held");
    let told = wired
        .store
        .read_from(&AggregateId::from(&id), KIND, Version::of(1))
        .await
        .expect("reading should succeed");
    assert!(
        matches!(
            &told[1].event,
            AccountEvent::InvitationTakenUp { invitation } if invitation == "invitation_1"
        ),
        "invitations accept it and members let her join from that"
    );
    assert_eq!(still_there(&wired).await, None);
}

#[tokio::test]
async fn where_signing_up_is_by_invitation_only_an_operator_s_invitation_into_a_project_lets_in() {
    let by_an_operator = wiring::wired();
    invited_into_a_project(&by_an_operator, true).await;
    let by_an_owner = wiring::wired();
    invited_into_a_project(&by_an_owner, false).await;

    by_an_operator
        .accounts
        .clone()
        .signing_up(SigningUp::ByInvitation)
        .sign_up_by_invitation(TOKEN, "Ada", HER_PASSWORD)
        .await
        .expect("an operator may bring in anyone");
    assert!(matches!(
        by_an_owner
            .accounts
            .clone()
            .signing_up(SigningUp::ByInvitation)
            .sign_up_by_invitation(TOKEN, "Ada", HER_PASSWORD)
            .await,
        Err(AccountsError::InvitationNotByAnOperator)
    ));
    assert_eq!(holder(&by_an_owner, ADA).await, None);
    assert_eq!(
        still_there(&by_an_owner).await,
        Some(into_a_project(false)),
        "she may still take it up once an operator lets her in"
    );
}

#[tokio::test]
async fn an_address_with_an_account_keeps_its_invitation_to_take_up_signed_in() {
    let wired = wiring::wired();
    wired
        .accounts
        .sign_up(ADA, "Ada", HER_PASSWORD)
        .await
        .expect("signing up should succeed");
    invited_into_a_project(&wired, false).await;

    assert!(matches!(
        signs_up(&wired, TOKEN).await,
        Err(AccountsError::EmailAddressHasAccount)
    ));
    assert_eq!(
        still_there(&wired).await,
        Some(into_a_project(false)),
        "signing in, she may take it up"
    );
}
