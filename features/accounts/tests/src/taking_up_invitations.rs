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

fn a_ticket_into_a_project(to: &str) -> InvitationTicket {
    InvitationTicket {
        invitation: "invitation_1".to_owned(),
        to: EmailAddress::new(to).expect("a plain address"),
        until: clock::at(1_700_000_000) + Duration::days(7),
        scope: InvitationScope::Project {
            project: "project_loom".to_owned(),
        },
        by_an_operator: false,
    }
}

async fn an_invitation(wired: &Wired, ticket: &InvitationTicket) {
    wired
        .keep_ticket(&InvitationToken::presented(TOKEN).digest(), ticket)
        .await
        .expect("keeping should succeed");
}

async fn an_account(wired: &Wired, address: &str) -> AccountId {
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

async fn invitations_taken_up_by(wired: &Wired, account: &AccountId) -> Vec<String> {
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
    // given: an account
    let wired = wiring::wired();
    let ada = an_account(&wired, ADA).await;
    // and: an invitation into a project to its address
    an_invitation(&wired, &a_ticket_into_a_project(ADA)).await;

    // when: it takes the invitation up by its link
    let ticket = wired
        .accounts
        .take_up_invitation(&ada, &format!(" {TOKEN}\n"))
        .await
        .expect("the invitation was sent to her address");

    // then: the invitation is answered
    assert_eq!(ticket, a_ticket_into_a_project(ADA));
    // and: the account took it up
    assert_eq!(
        invitations_taken_up_by(&wired, &ada).await,
        vec!["invitation_1".to_owned()]
    );
    // and: its ticket is gone
    assert_eq!(still_there(&wired).await, None);
    // and: taking it up again is refused
    assert!(matches!(
        wired.accounts.take_up_invitation(&ada, TOKEN).await,
        Err(AccountsError::InvitationInvalid)
    ));
    // and: the account took it up once
    assert_eq!(
        invitations_taken_up_by(&wired, &ada).await.len(),
        1,
        "a link lets one in once"
    );
}

#[tokio::test]
async fn an_invitation_to_another_address_is_refused_naming_it_and_left_for_it() {
    // given: an account
    let wired = wiring::wired();
    let bob = an_account(&wired, BOB).await;
    // and: an invitation to another address
    an_invitation(&wired, &a_ticket_into_a_project(ADA)).await;

    // when: the account takes it up by its link
    let refused = wired.accounts.take_up_invitation(&bob, TOKEN).await;

    // then: it is refused as for another address, not naming it
    assert!(
        matches!(&refused, Err(AccountsError::InvitationForAnotherAddress)),
        "a link may be forwarded or found, so it does not tell whom it was for: {refused:?}"
    );
    // and: the account took nothing up
    assert!(invitations_taken_up_by(&wired, &bob).await.is_empty());
    // and: the ticket is kept
    assert_eq!(
        still_there(&wired).await,
        Some(a_ticket_into_a_project(ADA)),
        "the one invited may still sign in and take it up"
    );
}

#[tokio::test]
async fn an_invitation_to_the_instance_is_only_for_signing_up() {
    // given: an account
    let wired = wiring::wired();
    let ada = an_account(&wired, ADA).await;
    // and: an invitation to the instance to its address
    let to_the_instance = InvitationTicket {
        scope: InvitationScope::Instance,
        by_an_operator: true,
        ..a_ticket_into_a_project(ADA)
    };
    an_invitation(&wired, &to_the_instance).await;

    // then: taking it up is refused
    assert!(matches!(
        wired.accounts.take_up_invitation(&ada, TOKEN).await,
        Err(AccountsError::InvitationInvalid)
    ));
    // and: the account took nothing up
    assert!(invitations_taken_up_by(&wired, &ada).await.is_empty());
    // and: the ticket is kept
    assert_eq!(
        still_there(&wired).await,
        Some(to_the_instance),
        "it stays open until it runs out or is withdrawn, as any link nobody used"
    );
}

#[tokio::test]
async fn a_token_nobody_was_given_or_one_withdrawn_takes_nothing_up() {
    // given: an account
    let wired = wiring::wired();
    let ada = an_account(&wired, ADA).await;
    // and: a withdrawn invitation to its address
    an_invitation(&wired, &a_ticket_into_a_project(ADA)).await;
    wired
        .forget_ticket("invitation_1")
        .await
        .expect("forgetting should succeed");

    // then: neither its token nor one nobody was given is taken up
    for token in [TOKEN, "a token nobody was given"] {
        assert!(matches!(
            wired.accounts.take_up_invitation(&ada, token).await,
            Err(AccountsError::InvitationInvalid)
        ));
    }
    // and: the account took nothing up
    assert!(invitations_taken_up_by(&wired, &ada).await.is_empty());
}

#[tokio::test]
async fn an_invitation_is_taken_up_to_its_last_moment_and_no_longer() {
    // given: an account invited at the invitation's last moment
    let last = clock::at(1_700_000_000) + Duration::days(7);
    let on_time = wiring::wired();
    let ada_on_time = an_account(&on_time, ADA).await;
    an_invitation(&on_time, &a_ticket_into_a_project(ADA)).await;
    on_time.clock.set(last);
    // and: one a second past it
    let late = wiring::wired();
    let ada_late = an_account(&late, ADA).await;
    an_invitation(&late, &a_ticket_into_a_project(ADA)).await;
    late.clock.set(last + Duration::seconds(1));

    // then: the one on time takes it up
    on_time
        .accounts
        .take_up_invitation(&ada_on_time, TOKEN)
        .await
        .expect("an invitation still holds at its last moment");
    // and: the late one is refused as expired
    assert!(matches!(
        late.accounts.take_up_invitation(&ada_late, TOKEN).await,
        Err(AccountsError::InvitationExpired)
    ));
    // and: takes nothing up
    assert!(invitations_taken_up_by(&late, &ada_late).await.is_empty());
}

#[tokio::test]
async fn an_account_takes_up_an_invitation_to_its_address_by_its_id_once() {
    // given: an account
    let wired = wiring::wired();
    let ada = an_account(&wired, ADA).await;
    // and: an invitation into a project to its address
    an_invitation(&wired, &a_ticket_into_a_project(ADA)).await;

    // when: it takes the invitation up by its id
    let ticket = wired
        .accounts
        .take_up_invitation_by_id(&ada, "invitation_1")
        .await
        .expect("the invitation was sent to her address");

    // then: the invitation is answered
    assert_eq!(ticket, a_ticket_into_a_project(ADA));
    // and: the account took it up
    assert_eq!(
        invitations_taken_up_by(&wired, &ada).await,
        vec!["invitation_1".to_owned()]
    );
    // and: its ticket is gone
    assert_eq!(
        still_there(&wired).await,
        None,
        "nor may the link be used after"
    );
    // and: taking it up again is refused
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
    // given: an account
    let wired = wiring::wired();
    let bob = an_account(&wired, BOB).await;
    // and: an invitation to another address
    an_invitation(&wired, &a_ticket_into_a_project(ADA)).await;

    // when: the account takes it up by its id
    let refused = wired
        .accounts
        .take_up_invitation_by_id(&bob, "invitation_1")
        .await;

    // then: it is refused as though it were not there
    assert!(
        matches!(refused, Err(AccountsError::InvitationInvalid)),
        "an id may be guessed, so whom it was sent to is not told: {refused:?}"
    );
    // and: the account took nothing up
    assert!(invitations_taken_up_by(&wired, &bob).await.is_empty());
    // and: the ticket is kept
    assert_eq!(
        still_there(&wired).await,
        Some(a_ticket_into_a_project(ADA))
    );
    // and: an id nobody issued is refused alike
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
    // given: an account
    let wired = wiring::wired();
    let ada = an_account(&wired, ADA).await;
    // and: an invitation to its address run out
    an_invitation(&wired, &a_ticket_into_a_project(ADA)).await;
    wired
        .clock
        .set(clock::at(1_700_000_000) + Duration::days(7) + Duration::SECOND);

    // then: taking it up by its id is refused as expired
    assert!(matches!(
        wired
            .accounts
            .take_up_invitation_by_id(&ada, "invitation_1")
            .await,
        Err(AccountsError::InvitationExpired)
    ));
}

async fn invitations_turned_down_by(wired: &Wired, account: &AccountId) -> Vec<String> {
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
    // given: an account
    let wired = wiring::wired();
    let ada = an_account(&wired, ADA).await;
    // and: an invitation into a project to its address
    an_invitation(&wired, &a_ticket_into_a_project(ADA)).await;

    // when: it turns the invitation down by its link
    wired
        .accounts
        .decline_invitation(&ada, &format!(" {TOKEN}\n"))
        .await
        .expect("the invitation was sent to her address");

    // then: the account turned it down
    assert_eq!(
        invitations_turned_down_by(&wired, &ada).await,
        vec!["invitation_1".to_owned()]
    );
    // and: its ticket is gone
    assert_eq!(still_there(&wired).await, None);
    // and: turning it down again is refused
    assert!(matches!(
        wired.accounts.decline_invitation(&ada, TOKEN).await,
        Err(AccountsError::InvitationInvalid)
    ));
    // and: so is taking it up
    assert!(matches!(
        wired.accounts.take_up_invitation(&ada, TOKEN).await,
        Err(AccountsError::InvitationInvalid)
    ));
    // and: it was turned down once and never taken up
    assert_eq!(invitations_turned_down_by(&wired, &ada).await.len(), 1);
    assert!(invitations_taken_up_by(&wired, &ada).await.is_empty());
}

#[tokio::test]
async fn an_account_turns_down_an_invitation_to_its_address_by_its_id() {
    // given: an account
    let wired = wiring::wired();
    let ada = an_account(&wired, ADA).await;
    // and: an invitation into a project to its address
    an_invitation(&wired, &a_ticket_into_a_project(ADA)).await;

    // when: it turns the invitation down by its id
    wired
        .accounts
        .decline_invitation_by_id(&ada, "invitation_1")
        .await
        .expect("the invitation was sent to her address");

    // then: the account turned it down
    assert_eq!(
        invitations_turned_down_by(&wired, &ada).await,
        vec!["invitation_1".to_owned()]
    );
    // and: its ticket is gone
    assert_eq!(still_there(&wired).await, None);
    // and: turning it down again is refused
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
    // given: an account
    let wired = wiring::wired();
    let bob = an_account(&wired, BOB).await;
    // and: an invitation to another address
    an_invitation(&wired, &a_ticket_into_a_project(ADA)).await;

    // when: the account turns it down by its link and by its id
    let by_the_link = wired.accounts.decline_invitation(&bob, TOKEN).await;
    let by_its_id = wired
        .accounts
        .decline_invitation_by_id(&bob, "invitation_1")
        .await;

    // then: by the link it is refused as for another address
    assert!(
        matches!(
            &by_the_link,
            Err(AccountsError::InvitationForAnotherAddress)
        ),
        "{by_the_link:?}"
    );
    // and: by its id as though it were not there
    assert!(
        matches!(by_its_id, Err(AccountsError::InvitationInvalid)),
        "an id may be guessed: {by_its_id:?}"
    );
    // and: the account turned nothing down
    assert!(invitations_turned_down_by(&wired, &bob).await.is_empty());
    // and: the ticket is kept
    assert_eq!(
        still_there(&wired).await,
        Some(a_ticket_into_a_project(ADA))
    );
    // and: an id nobody issued is refused alike
    assert!(matches!(
        wired
            .accounts
            .decline_invitation_by_id(&bob, "invitation_nobody_issued")
            .await,
        Err(AccountsError::InvitationInvalid)
    ));
}

#[tokio::test]
async fn an_invitation_run_out_is_not_turned_down() {
    // given: an account
    let wired = wiring::wired();
    let ada = an_account(&wired, ADA).await;
    // and: an invitation to its address run out
    an_invitation(&wired, &a_ticket_into_a_project(ADA)).await;
    wired
        .clock
        .set(clock::at(1_700_000_000) + Duration::days(7) + Duration::SECOND);

    // then: turning it down is refused as expired
    assert!(matches!(
        wired.accounts.decline_invitation(&ada, TOKEN).await,
        Err(AccountsError::InvitationExpired)
    ));
    // and: the account turned nothing down
    assert!(invitations_turned_down_by(&wired, &ada).await.is_empty());
    // and: the ticket is kept
    assert!(still_there(&wired).await.is_some());
}

#[tokio::test]
async fn an_invitation_to_the_instance_is_turned_down_by_the_link_or_by_its_id() {
    // given: an account invited to the instance
    let to_the_instance = InvitationTicket {
        scope: InvitationScope::Instance,
        by_an_operator: true,
        ..a_ticket_into_a_project(ADA)
    };
    let by_the_link = wiring::wired();
    let ada = an_account(&by_the_link, ADA).await;
    an_invitation(&by_the_link, &to_the_instance).await;
    // and: another one so invited
    let by_its_id = wiring::wired();
    let ada_too = an_account(&by_its_id, ADA).await;
    an_invitation(&by_its_id, &to_the_instance).await;

    // when: one turns it down by the link and the other by its id
    by_the_link
        .accounts
        .decline_invitation(&ada, TOKEN)
        .await
        .expect("whoever was invited may say no, whatever to");
    by_its_id
        .accounts
        .decline_invitation_by_id(&ada_too, "invitation_1")
        .await
        .expect("whoever was invited may say no, whatever to");

    // then: each turned it down and its ticket is gone
    for (wired, ada) in [(&by_the_link, &ada), (&by_its_id, &ada_too)] {
        assert_eq!(
            invitations_turned_down_by(wired, ada).await,
            vec!["invitation_1".to_owned()]
        );
        assert_eq!(still_there(wired).await, None);
    }
}

#[tokio::test]
async fn a_take_up_whose_account_cannot_be_kept_leaves_the_invitation_to_be_taken_up_again() {
    // given: an account
    let wired = wiring::wired();
    let ada = an_account(&wired, ADA).await;
    // and: an invitation into a project to its address
    an_invitation(&wired, &a_ticket_into_a_project(ADA)).await;

    // when: it takes the invitation up while the account cannot be kept
    let refused = wired
        .refusing_appends()
        .take_up_invitation(&ada, TOKEN)
        .await;

    // then: it is refused
    assert!(refused.is_err());
    // and: the ticket is kept
    assert_eq!(
        still_there(&wired).await,
        Some(a_ticket_into_a_project(ADA)),
        "a ticket used up for a take-up that never landed would leave the invitation open for good"
    );

    // when: it takes the invitation up once the account can be kept
    wired
        .accounts
        .take_up_invitation(&ada, TOKEN)
        .await
        .expect("the invitation is there to take up once the account can be kept");

    // then: the account took it up
    assert_eq!(
        invitations_taken_up_by(&wired, &ada).await,
        vec!["invitation_1".to_owned()]
    );
}

#[tokio::test]
async fn a_turning_down_whose_account_cannot_be_kept_leaves_the_invitation_to_be_turned_down_again()
{
    // given: an account
    let wired = wiring::wired();
    let ada = an_account(&wired, ADA).await;
    // and: an invitation into a project to its address
    an_invitation(&wired, &a_ticket_into_a_project(ADA)).await;

    // when: it turns the invitation down while the account cannot be kept
    let refused = wired
        .refusing_appends()
        .decline_invitation(&ada, TOKEN)
        .await;

    // then: it is refused
    assert!(refused.is_err());
    // and: the ticket is kept
    assert_eq!(
        still_there(&wired).await,
        Some(a_ticket_into_a_project(ADA))
    );

    // when: it turns the invitation down once the account can be kept
    wired
        .accounts
        .decline_invitation(&ada, TOKEN)
        .await
        .expect("the invitation is there to turn down once the account can be kept");

    // then: the account turned it down
    assert_eq!(
        invitations_turned_down_by(&wired, &ada).await,
        vec!["invitation_1".to_owned()]
    );
}

#[tokio::test]
async fn a_take_up_that_lost_a_race_takes_the_ticket_again_in_a_fresh_transaction_and_lands() {
    // given: an account
    let wired = wiring::wired();
    let ada = an_account(&wired, ADA).await;
    // and: an invitation into a project to its address
    an_invitation(&wired, &a_ticket_into_a_project(ADA)).await;

    // when: it takes the invitation up and loses a race once
    wired
        .losing_races(1)
        .take_up_invitation(&ada, TOKEN)
        .await
        .expect(
            "the ticket taken in the lost try was given back with it, so the second try takes it",
        );

    // then: the account took it up
    assert_eq!(
        invitations_taken_up_by(&wired, &ada).await,
        vec!["invitation_1".to_owned()]
    );
    // and: its ticket is gone
    assert_eq!(still_there(&wired).await, None);
}
