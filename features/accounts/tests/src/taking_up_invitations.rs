use accounts_core::{
    AccountEvent, AccountId, AccountsError, EmailAddress, EmailAddresses, InvitationScope,
    InvitationTicket, InvitationTickets, InvitationToken, KIND,
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
