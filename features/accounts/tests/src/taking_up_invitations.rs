use accounts_core::{
    AccountEvent, AccountId, AccountsError, DeclineReason, EmailAddress, EmailAddresses,
    InvitationScope, InvitationTicket, InvitationTickets, InvitationToken, KIND,
};
use eventsourcing::{AggregateId, EventStore, Version};
use time::Duration;

use crate::wiring;
use crate::wiring::{ADA, HER_PASSWORD, Wired};

const TOKEN: &str = "a token drawn by invitations";
const BOB: &str = "bob@example.com";

fn into_a_project(to: &str) -> InvitationTicket {
    InvitationTicket {
        invitation: "invitation_1".to_owned(),
        to: EmailAddress::new(to).expect("a plain address"),
        until: wiring::at(1_700_000_000) + Duration::days(7),
        scope: InvitationScope::Project {
            project: "project_loom".to_owned(),
        },
        by_an_operator: false,
    }
}

async fn invited(wired: &Wired, ticket: &InvitationTicket) {
    wired
        .secrets
        .keep_ticket(&InvitationToken::presented(TOKEN).digest(), ticket)
        .await
        .expect("keeping should succeed");
}

async fn signed_up(wired: &Wired, address: &str) -> AccountId {
    wired
        .accounts
        .sign_up(address, "Someone", HER_PASSWORD)
        .await
        .expect("signing up should succeed");

    wired
        .secrets
        .holder(&EmailAddress::new(address).expect("a plain address"))
        .await
        .expect("reading should succeed")
        .expect("the address is held")
}

async fn still_there(wired: &Wired) -> Option<InvitationTicket> {
    wired
        .secrets
        .find_ticket(&InvitationToken::presented(TOKEN).digest())
        .await
        .expect("finding should succeed")
}

async fn taken_up(wired: &Wired, account: &AccountId) -> Vec<String> {
    wired
        .store
        .read_from(&AggregateId::from(account), KIND, Version::of(1))
        .await
        .expect("reading should succeed")
        .into_iter()
        .filter_map(|recorded| match recorded.event {
            AccountEvent::InvitationTakenUp { invitation } => {
                assert_eq!(
                    recorded.metadata.agent,
                    account.acting(),
                    "the account takes it up itself, so invitations know who accepted"
                );
                Some(invitation)
            }
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn an_account_takes_up_an_invitation_to_its_address_once() {
    let wired = wiring::wired();
    let ada = signed_up(&wired, ADA).await;
    invited(&wired, &into_a_project(ADA)).await;

    let ticket = wired
        .accounts
        .take_up_invitation(&ada, &format!(" {TOKEN}\n"))
        .await
        .expect("the invitation was sent to her address");

    assert_eq!(ticket, into_a_project(ADA));
    assert_eq!(
        taken_up(&wired, &ada).await,
        vec!["invitation_1".to_owned()]
    );
    assert_eq!(still_there(&wired).await, None);
    assert!(matches!(
        wired.accounts.take_up_invitation(&ada, TOKEN).await,
        Err(AccountsError::InvitationInvalid)
    ));
    assert_eq!(
        taken_up(&wired, &ada).await.len(),
        1,
        "a link lets one in once"
    );
}

#[tokio::test]
async fn an_invitation_to_another_address_is_refused_naming_it_and_left_for_it() {
    let wired = wiring::wired();
    let bob = signed_up(&wired, BOB).await;
    invited(&wired, &into_a_project(ADA)).await;

    let refused = wired.accounts.take_up_invitation(&bob, TOKEN).await;

    assert!(
        matches!(
            &refused,
            Err(AccountsError::InvitationForAnotherAddress { to }) if to.as_str() == ADA
        ),
        "whoever holds the link was sent it, so it may say whom it was for: {refused:?}"
    );
    assert!(taken_up(&wired, &bob).await.is_empty());
    assert_eq!(
        still_there(&wired).await,
        Some(into_a_project(ADA)),
        "the one invited may still sign in and take it up"
    );
}

#[tokio::test]
async fn an_invitation_to_the_instance_is_only_for_signing_up() {
    let wired = wiring::wired();
    let ada = signed_up(&wired, ADA).await;
    let to_the_instance = InvitationTicket {
        scope: InvitationScope::Instance,
        by_an_operator: true,
        ..into_a_project(ADA)
    };
    invited(&wired, &to_the_instance).await;

    assert!(matches!(
        wired.accounts.take_up_invitation(&ada, TOKEN).await,
        Err(AccountsError::InvitationInvalid)
    ));
    assert!(taken_up(&wired, &ada).await.is_empty());
    assert_eq!(
        still_there(&wired).await,
        Some(to_the_instance),
        "it stays open until it runs out or is withdrawn, as any link nobody used"
    );
}

#[tokio::test]
async fn a_token_nobody_was_given_or_one_withdrawn_takes_nothing_up() {
    let wired = wiring::wired();
    let ada = signed_up(&wired, ADA).await;
    invited(&wired, &into_a_project(ADA)).await;
    wired
        .secrets
        .forget_ticket("invitation_1")
        .await
        .expect("forgetting should succeed");

    for token in [TOKEN, "a token nobody was given"] {
        assert!(matches!(
            wired.accounts.take_up_invitation(&ada, token).await,
            Err(AccountsError::InvitationInvalid)
        ));
    }
    assert!(taken_up(&wired, &ada).await.is_empty());
}

#[tokio::test]
async fn an_invitation_is_taken_up_to_its_last_moment_and_no_longer() {
    let last = wiring::at(1_700_000_000) + Duration::days(7);
    let on_time = wiring::wired();
    let ada_on_time = signed_up(&on_time, ADA).await;
    invited(&on_time, &into_a_project(ADA)).await;
    on_time.clock.set(last);
    let late = wiring::wired();
    let ada_late = signed_up(&late, ADA).await;
    invited(&late, &into_a_project(ADA)).await;
    late.clock.set(last + Duration::seconds(1));

    on_time
        .accounts
        .take_up_invitation(&ada_on_time, TOKEN)
        .await
        .expect("an invitation still holds at its last moment");
    assert!(matches!(
        late.accounts.take_up_invitation(&ada_late, TOKEN).await,
        Err(AccountsError::InvitationExpired)
    ));
    assert!(taken_up(&late, &ada_late).await.is_empty());
}

#[tokio::test]
async fn an_account_takes_up_an_invitation_to_its_address_by_its_id_once() {
    let wired = wiring::wired();
    let ada = signed_up(&wired, ADA).await;
    invited(&wired, &into_a_project(ADA)).await;

    let ticket = wired
        .accounts
        .take_up_invitation_by_id(&ada, "invitation_1")
        .await
        .expect("the invitation was sent to her address");

    assert_eq!(ticket, into_a_project(ADA));
    assert_eq!(
        taken_up(&wired, &ada).await,
        vec!["invitation_1".to_owned()]
    );
    assert_eq!(
        still_there(&wired).await,
        None,
        "nor may the link be used after"
    );
    assert!(matches!(
        wired
            .accounts
            .take_up_invitation_by_id(&ada, "invitation_1")
            .await,
        Err(AccountsError::InvitationInvalid)
    ));
}

#[tokio::test]
async fn by_its_id_an_invitation_to_another_address_is_as_good_as_not_there_and_left_for_it() {
    let wired = wiring::wired();
    let bob = signed_up(&wired, BOB).await;
    invited(&wired, &into_a_project(ADA)).await;

    let refused = wired
        .accounts
        .take_up_invitation_by_id(&bob, "invitation_1")
        .await;

    assert!(
        matches!(refused, Err(AccountsError::InvitationInvalid)),
        "an id may be guessed, so whom it was sent to is not told: {refused:?}"
    );
    assert!(taken_up(&wired, &bob).await.is_empty());
    assert_eq!(still_there(&wired).await, Some(into_a_project(ADA)));
    assert!(matches!(
        wired
            .accounts
            .take_up_invitation_by_id(&bob, "invitation_nobody_issued")
            .await,
        Err(AccountsError::InvitationInvalid)
    ));
}

#[tokio::test]
async fn by_its_id_an_invitation_run_out_is_told_so() {
    let wired = wiring::wired();
    let ada = signed_up(&wired, ADA).await;
    invited(&wired, &into_a_project(ADA)).await;
    wired
        .clock
        .set(wiring::at(1_700_000_000) + Duration::days(7) + Duration::SECOND);

    assert!(matches!(
        wired
            .accounts
            .take_up_invitation_by_id(&ada, "invitation_1")
            .await,
        Err(AccountsError::InvitationExpired)
    ));
}

async fn turned_down(wired: &Wired, account: &AccountId) -> Vec<String> {
    wired
        .store
        .read_from(&AggregateId::from(account), KIND, Version::of(1))
        .await
        .expect("reading should succeed")
        .into_iter()
        .filter_map(|recorded| match recorded.event {
            AccountEvent::InvitationDeclined {
                invitation,
                reason: DeclineReason::TurnedDown,
            } => {
                assert_eq!(
                    recorded.metadata.agent,
                    account.acting(),
                    "the account turns it down itself"
                );
                Some(invitation)
            }
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn an_account_turns_down_an_invitation_to_its_address_once_and_may_take_it_up_no_more() {
    let wired = wiring::wired();
    let ada = signed_up(&wired, ADA).await;
    invited(&wired, &into_a_project(ADA)).await;

    wired
        .accounts
        .decline_invitation(&ada, &format!(" {TOKEN}\n"))
        .await
        .expect("the invitation was sent to her address");

    assert_eq!(
        turned_down(&wired, &ada).await,
        vec!["invitation_1".to_owned()]
    );
    assert_eq!(still_there(&wired).await, None);
    assert!(matches!(
        wired.accounts.decline_invitation(&ada, TOKEN).await,
        Err(AccountsError::InvitationInvalid)
    ));
    assert!(matches!(
        wired.accounts.take_up_invitation(&ada, TOKEN).await,
        Err(AccountsError::InvitationInvalid)
    ));
    assert_eq!(turned_down(&wired, &ada).await.len(), 1);
    assert!(taken_up(&wired, &ada).await.is_empty());
}

#[tokio::test]
async fn an_account_turns_down_an_invitation_to_its_address_by_its_id() {
    let wired = wiring::wired();
    let ada = signed_up(&wired, ADA).await;
    invited(&wired, &into_a_project(ADA)).await;

    wired
        .accounts
        .decline_invitation_by_id(&ada, "invitation_1")
        .await
        .expect("the invitation was sent to her address");

    assert_eq!(
        turned_down(&wired, &ada).await,
        vec!["invitation_1".to_owned()]
    );
    assert_eq!(still_there(&wired).await, None);
    assert!(matches!(
        wired
            .accounts
            .decline_invitation_by_id(&ada, "invitation_1")
            .await,
        Err(AccountsError::InvitationInvalid)
    ));
}

#[tokio::test]
async fn nobody_turns_down_an_invitation_to_another_address_and_it_is_left_for_it() {
    let wired = wiring::wired();
    let bob = signed_up(&wired, BOB).await;
    invited(&wired, &into_a_project(ADA)).await;

    let by_the_link = wired.accounts.decline_invitation(&bob, TOKEN).await;
    let by_its_id = wired
        .accounts
        .decline_invitation_by_id(&bob, "invitation_1")
        .await;

    assert!(
        matches!(
            &by_the_link,
            Err(AccountsError::InvitationForAnotherAddress { to }) if to.as_str() == ADA
        ),
        "whoever holds the link was sent it: {by_the_link:?}"
    );
    assert!(
        matches!(by_its_id, Err(AccountsError::InvitationInvalid)),
        "an id may be guessed: {by_its_id:?}"
    );
    assert!(turned_down(&wired, &bob).await.is_empty());
    assert_eq!(still_there(&wired).await, Some(into_a_project(ADA)));
    assert!(matches!(
        wired
            .accounts
            .decline_invitation_by_id(&bob, "invitation_nobody_issued")
            .await,
        Err(AccountsError::InvitationInvalid)
    ));
}

#[tokio::test]
async fn an_invitation_run_out_or_to_the_instance_is_not_turned_down() {
    let wired = wiring::wired();
    let ada = signed_up(&wired, ADA).await;
    invited(&wired, &into_a_project(ADA)).await;
    wired
        .clock
        .set(wiring::at(1_700_000_000) + Duration::days(7) + Duration::SECOND);
    let to_the_instance = wiring::wired();
    let ada_too = signed_up(&to_the_instance, ADA).await;
    invited(
        &to_the_instance,
        &InvitationTicket {
            scope: InvitationScope::Instance,
            by_an_operator: true,
            ..into_a_project(ADA)
        },
    )
    .await;

    assert!(matches!(
        wired.accounts.decline_invitation(&ada, TOKEN).await,
        Err(AccountsError::InvitationExpired)
    ));
    assert!(matches!(
        to_the_instance
            .accounts
            .decline_invitation(&ada_too, TOKEN)
            .await,
        Err(AccountsError::InvitationInvalid)
    ));
    assert!(turned_down(&wired, &ada).await.is_empty());
    assert!(turned_down(&to_the_instance, &ada_too).await.is_empty());
    assert!(still_there(&wired).await.is_some());
}
