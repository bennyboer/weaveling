use std::sync::Arc;

use access::{InMemoryAccess, KeptAccess};
use authorizing::AllowingEverything;
use eventsourcing::{
    AggregateId, EventStore, FailsAppending, RefusesAppending, ServiceError, StoreError, Version,
};
use invitations_core::{InvitationEvent, InvitationsCore, InvitationsError, KIND, ProjectLink};
use throttling::InMemoryThrottles;
use transacting::InMemoryTransactions;

use crate::wiring;
use crate::wiring::{ORIGIN, Wired};

fn core_over(wired: &Wired, store: Arc<dyn EventStore<InvitationEvent>>) -> InvitationsCore {
    core_committing_with(wired, store, InMemoryTransactions::new())
}

fn core_committing_with(
    wired: &Wired,
    store: Arc<dyn EventStore<InvitationEvent>>,
    transactions: InMemoryTransactions,
) -> InvitationsCore {
    InvitationsCore::new(
        store,
        Arc::new(transactions),
        wired.keys.clone(),
        Arc::new(InMemoryThrottles::default()),
        wired.catalog.clone(),
        wired.mailbox.clone(),
        KeptAccess::new(Arc::new(InMemoryAccess::new())),
        Arc::new(AllowingEverything),
        wired.clock.clone(),
    )
}

#[tokio::test]
async fn an_invitation_whose_issuing_cannot_be_kept_is_mailed_to_nobody() {
    // given: invitations whose events cannot be kept
    let wired = wiring::a_workbench();

    // when: one is issued
    let refused = core_over(&wired, Arc::new(RefusesAppending(wired.store.clone())))
        .invite_to_instance(&wiring::an_operator(), "ada@example.com", ORIGIN)
        .await;

    // then: it is refused
    assert!(refused.is_err());
    // and: nothing is mailed
    assert!(
        wired.sent_to("ada@example.com").await.is_empty(),
        "the mail goes out only once the invitation is kept"
    );
}

async fn withdrawing_while_losing_races(losses: u8) -> (Result<(), InvitationsError>, usize) {
    let wired = wiring::a_workbench();
    let issued = wired
        .invitations
        .invite_to_instance(&wiring::an_operator(), "ada@example.com", ORIGIN)
        .await
        .expect("an operator may invite");

    let withdrawn = core_over(
        &wired,
        Arc::new(FailsAppending::losing_races(wired.store.clone(), losses)),
    )
    .withdraw(&wiring::an_operator(), &issued.id, None)
    .await;
    let told = wired
        .store
        .read_from(&AggregateId::from(&issued.id), KIND, Version::of(1))
        .await
        .expect("reading should succeed")
        .len();

    (withdrawn, told)
}

#[tokio::test]
async fn a_withdrawal_that_lost_a_race_is_tried_again_and_lands() {
    // when: a withdrawal loses four races
    let (withdrawn, told) = withdrawing_while_losing_races(4).await;

    // then: it lands
    withdrawn.expect("the fifth try lands");
    // and: once
    assert_eq!(told, 2, "issued, then withdrawn once");
}

#[tokio::test]
async fn a_withdrawal_that_lost_every_race_it_may_try_is_a_conflict() {
    // when: a withdrawal loses every race it may try
    let (withdrawn, told) = withdrawing_while_losing_races(transacting::retrying::TRIES).await;

    // then: it is a conflict
    assert!(matches!(
        withdrawn,
        Err(InvitationsError::Events(ServiceError::Store(
            StoreError::Outdated { .. }
        )))
    ));
    // and: nothing is withdrawn
    assert_eq!(told, 1, "issued, never withdrawn");
}

#[tokio::test]
async fn an_invitation_whose_commit_is_refused_is_mailed_to_nobody() {
    // given: invitations whose commits are refused
    let wired = wiring::a_workbench();

    // when: one is issued
    let refused = core_committing_with(
        &wired,
        wired.store.clone(),
        InMemoryTransactions::refusing_commits(),
    )
    .invite_to_instance(&wiring::an_operator(), "ada@example.com", ORIGIN)
    .await;

    // then: it is refused
    assert!(refused.is_err());
    // and: nothing is mailed
    assert!(
        wired.sent_to("ada@example.com").await.is_empty(),
        "the mail goes out only once the invitation is committed"
    );
}

#[tokio::test]
async fn tombstones_forgotten_are_forgotten_for_good() {
    // given: a project's tombstone
    let wired = wiring::a_workbench();
    wired
        .invitations
        .lay_project_tombstone(None, &ProjectLink::from("project_gone"), clock::at(1_000))
        .await
        .expect("laying a tombstone should succeed");

    // when: what was laid before a later moment is forgotten twice
    let mut forgotten = Vec::new();
    for _ in 0..2 {
        forgotten.push(
            wired
                .invitations
                .forget_tombstones_laid_before(clock::at(2_000), 10)
                .await
                .expect("forgetting tombstones should succeed"),
        );
    }

    // then: it is forgotten once
    assert_eq!(forgotten, [1, 0]);
}
