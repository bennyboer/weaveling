use accounts_core::{
    AccountEvent, AccountId, AccountsError, EmailAddress, EmailAddresses, InvitationScope,
    InvitationTicket, InvitationTickets, InvitationToken, KIND, SigningUp,
};
use eventsourcing::{AggregateId, EventStore, Version};
use time::Duration;

use crate::wiring;
use crate::wiring::{ADA, HER_PASSWORD, Wired};

const TOKEN: &str = "a token drawn by invitations";

async fn an_invitation(wired: &Wired, address: &str, token: &str) {
    wired
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
    // given: an invitation
    let wired = wiring::wired();
    an_invitation(&wired, ADA, TOKEN).await;

    // when: its address signs up by it
    let (session, signed_in) = wired
        .accounts
        .sign_up_by_invitation(TOKEN, "Ada", HER_PASSWORD)
        .await
        .expect("an open invitation lets its address sign up");

    // then: the address holds the new account
    let id = holder(&wired, ADA).await.expect("the address is held");
    assert_eq!(signed_in.session.account, id);
    assert_eq!(signed_in.account.email().as_str(), ADA);
    // and: she is signed in
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
    // and: she may sign in again without a code
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
    // and: the account tells it signed up by the invitation and took it up itself
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
    // given: an invitation
    let wired = wiring::wired();
    an_invitation(&wired, ADA, TOKEN).await;

    // then: its token with space around it signs up
    signs_up(&wired, &format!("  {TOKEN}\n"))
        .await
        .expect("a token is read as it was meant");
}

#[tokio::test]
async fn an_invitation_lets_one_account_in() {
    // given: an invitation one account signed up by
    let wired = wiring::wired();
    an_invitation(&wired, ADA, TOKEN).await;
    signs_up(&wired, TOKEN)
        .await
        .expect("the first sign-up takes it");

    // then: it signs up nobody else
    assert!(matches!(
        signs_up(&wired, TOKEN).await,
        Err(AccountsError::InvitationInvalid)
    ));
}

#[tokio::test]
async fn a_token_nobody_was_given_or_one_withdrawn_signs_nobody_up() {
    // given: a withdrawn invitation
    let wired = wiring::wired();
    an_invitation(&wired, ADA, TOKEN).await;
    wired
        .forget_ticket("invitation_1")
        .await
        .expect("forgetting should succeed");

    // then: neither its token nor one nobody was given signs up
    for token in [TOKEN, "a token nobody was given"] {
        assert!(matches!(
            signs_up(&wired, token).await,
            Err(AccountsError::InvitationInvalid)
        ));
    }
    // and: the address is held by nobody
    assert_eq!(holder(&wired, ADA).await, None);
}

#[tokio::test]
async fn an_invitation_holds_to_its_last_moment_and_no_longer() {
    // given: an invitation at its last moment
    let last = wiring::at(1_700_000_000) + Duration::days(7);
    let on_time = wiring::wired();
    an_invitation(&on_time, ADA, TOKEN).await;
    on_time.clock.set(last);
    // and: one a second past it
    let late = wiring::wired();
    an_invitation(&late, ADA, TOKEN).await;
    late.clock.set(last + Duration::seconds(1));

    // then: the one at its last moment signs up
    signs_up(&on_time, TOKEN)
        .await
        .expect("an invitation still holds at its last moment");
    // and: the late one is refused as expired
    assert!(matches!(
        signs_up(&late, TOKEN).await,
        Err(AccountsError::InvitationExpired)
    ));
    // and: its address is held by nobody
    assert_eq!(holder(&late, ADA).await, None);
}

#[tokio::test]
async fn a_name_or_password_refused_leaves_the_invitation_to_be_used() {
    // given: an invitation
    let wired = wiring::wired();
    an_invitation(&wired, ADA, TOKEN).await;

    // then: a blank name is refused
    assert!(matches!(
        wired
            .accounts
            .sign_up_by_invitation(TOKEN, "  ", HER_PASSWORD)
            .await,
        Err(AccountsError::InvalidName(_))
    ));
    // and: so is a short password
    assert!(matches!(
        wired
            .accounts
            .sign_up_by_invitation(TOKEN, "Ada", "short")
            .await,
        Err(AccountsError::InvalidPassword(_))
    ));

    // and: the invitation still signs her up
    signs_up(&wired, TOKEN)
        .await
        .expect("what was typed wrong is checked before the invitation is taken");
}

#[tokio::test]
async fn an_address_with_an_account_already_is_told_to_sign_in() {
    // given: an address with an account
    let wired = wiring::wired();
    wired
        .accounts
        .sign_up(ADA, "Ada", HER_PASSWORD)
        .await
        .expect("signing up should succeed");
    let first = holder(&wired, ADA).await;
    // and: an invitation to it
    an_invitation(&wired, ADA, TOKEN).await;

    // then: signing up by the invitation is told the address has an account
    assert!(matches!(
        signs_up(&wired, TOKEN).await,
        Err(AccountsError::EmailAddressHasAccount)
    ));
    // and: the address keeps its account
    assert_eq!(
        holder(&wired, ADA).await,
        first,
        "the account there stays the one there"
    );
}

#[tokio::test]
async fn where_signing_up_is_by_invitation_only_an_invitation_lets_anyone_in() {
    // given: signing up by invitation only
    let wired = wiring::wired();
    let accounts = wired.accounts.clone().signing_up(SigningUp::ByInvitation);
    // and: an invitation
    an_invitation(&wired, ADA, TOKEN).await;

    // then: an address not invited may not sign up
    assert!(matches!(
        accounts
            .sign_up("bob@example.com", "Bob", HER_PASSWORD)
            .await,
        Err(AccountsError::SigningUpByInvitationOnly)
    ));
    assert_eq!(holder(&wired, "bob@example.com").await, None);
    // and: nothing is mailed to it
    assert!(
        wired.mailbox.sent_to("bob@example.com").is_empty(),
        "nothing is mailed to whoever was not asked"
    );
    // and: the invited address may not sign up without the invitation
    assert!(
        matches!(
            accounts.sign_up(ADA, "Ada", HER_PASSWORD).await,
            Err(AccountsError::SigningUpByInvitationOnly)
        ),
        "even an invited address comes in by its link, not around it"
    );
    // and: the invitation signs it up
    accounts
        .sign_up_by_invitation(TOKEN, "Ada", HER_PASSWORD)
        .await
        .expect("an invitation is the way in");
}

fn a_ticket_into_a_project(by_an_operator: bool) -> InvitationTicket {
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

async fn an_invitation_into_a_project(wired: &Wired, by_an_operator: bool) {
    wired
        .keep_ticket(
            &InvitationToken::presented(TOKEN).digest(),
            &a_ticket_into_a_project(by_an_operator),
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
    // given: an owner's invitation into a project
    let wired = wiring::wired();
    an_invitation_into_a_project(&wired, false).await;

    // when: its address signs up by it
    signs_up(&wired, TOKEN)
        .await
        .expect("an owner may bring in whom anyone could bring in");

    // then: the new account takes it up
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
    // and: its ticket is gone
    assert_eq!(still_there(&wired).await, None);
}

#[tokio::test]
async fn where_signing_up_is_by_invitation_only_an_operator_s_invitation_into_a_project_lets_in() {
    // given: an operator's invitation into a project
    let by_an_operator = wiring::wired();
    an_invitation_into_a_project(&by_an_operator, true).await;
    // and: an owner's
    let by_an_owner = wiring::wired();
    an_invitation_into_a_project(&by_an_owner, false).await;

    // then: where signing up is by invitation only the operator's signs its address up
    by_an_operator
        .accounts
        .clone()
        .signing_up(SigningUp::ByInvitation)
        .sign_up_by_invitation(TOKEN, "Ada", HER_PASSWORD)
        .await
        .expect("an operator may bring in anyone");
    // and: the owner's is refused as not by an operator
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
    // and: the owner's ticket is kept
    assert_eq!(
        still_there(&by_an_owner).await,
        Some(a_ticket_into_a_project(false)),
        "she may still take it up once an operator lets her in"
    );
}

#[tokio::test]
async fn an_address_with_an_account_keeps_its_invitation_to_take_up_signed_in() {
    // given: an address with an account
    let wired = wiring::wired();
    wired
        .accounts
        .sign_up(ADA, "Ada", HER_PASSWORD)
        .await
        .expect("signing up should succeed");
    // and: an invitation into a project to it
    an_invitation_into_a_project(&wired, false).await;

    // then: signing up by the invitation is told the address has an account
    assert!(matches!(
        signs_up(&wired, TOKEN).await,
        Err(AccountsError::EmailAddressHasAccount)
    ));
    // and: the ticket is kept
    assert_eq!(
        still_there(&wired).await,
        Some(a_ticket_into_a_project(false)),
        "signing in, she may take it up"
    );
}

#[tokio::test]
async fn a_sign_up_by_invitation_whose_account_cannot_be_kept_leaves_the_link_to_sign_up_by_again()
{
    // given: an invitation
    let wired = wiring::wired();
    an_invitation(&wired, ADA, TOKEN).await;

    // when: its address signs up by it while accounts cannot be kept
    let refused = wired
        .refusing_appends()
        .sign_up_by_invitation(TOKEN, "Ada", HER_PASSWORD)
        .await;

    // then: it is refused
    assert!(refused.is_err());
    // and: the address is held by nobody
    assert_eq!(
        holder(&wired, ADA).await,
        None,
        "an address claimed for an account that never came to be would be blocked for good"
    );
    // and: the ticket is kept
    assert!(
        wired
            .secrets
            .find_ticket(&InvitationToken::presented(TOKEN).digest())
            .await
            .expect("finding should succeed")
            .is_some(),
        "a ticket used up for an account that never came to be would leave the link dead"
    );

    // when: she signs up by it once accounts can be kept
    signs_up(&wired, TOKEN)
        .await
        .expect("the link signs its address up once the account can be kept");

    // then: the address holds an account
    assert!(holder(&wired, ADA).await.is_some());
}

#[tokio::test]
async fn a_sign_up_by_invitation_whose_session_cannot_be_opened_makes_no_account_and_leaves_the_link()
 {
    // given: an invitation
    let wired = wiring::wired();
    an_invitation(&wired, ADA, TOKEN).await;

    // when: its address signs up by it while sessions cannot be opened
    let refused = wired
        .refusing_to_open_sessions()
        .sign_up_by_invitation(TOKEN, "Ada", HER_PASSWORD)
        .await;

    // then: it is refused
    assert!(refused.is_err());
    // and: no account was made
    assert_eq!(
        holder(&wired, ADA).await,
        None,
        "told it failed, she would try again and find her address taken by an account she never got into"
    );

    // when: she signs up by it once sessions can be opened
    signs_up(&wired, TOKEN)
        .await
        .expect("the link signs its address up once sessions can be opened");

    // then: the address holds an account
    assert!(holder(&wired, ADA).await.is_some());
}
