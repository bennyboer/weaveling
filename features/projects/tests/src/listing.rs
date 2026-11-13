use std::sync::Arc;

use access::Role;
use clock::FixedClock;
use eventsourcing::{AccountId, Agent};
use projects_core::{ProjectsError, RawDetails};
use time::{Duration, OffsetDateTime};

use crate::wiring;
use crate::wiring::Wired;

const ADA: &str = "account_ada";

fn ada() -> Agent {
    Agent::Account(AccountId::from(ADA))
}

struct Guarded {
    wired: Wired,
    clock: Arc<FixedClock>,
}

fn guarded() -> Guarded {
    let clock = Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH));

    Guarded {
        wired: wiring::wired_with_accounts(clock.clone()),
        clock,
    }
}

async fn projects_started(guarded: &Guarded, count: i64) -> Vec<String> {
    let mut ids = Vec::new();
    for nth in 1..=count {
        guarded
            .clock
            .set(OffsetDateTime::UNIX_EPOCH + Duration::seconds(nth * 1_000));
        ids.push(
            guarded
                .wired
                .projects
                .start("A project", &RawDetails::default(), &ada())
                .await
                .expect("starting should succeed")
                .0
                .to_string(),
        );
    }
    guarded.wired.settle().await;

    ids
}

async fn listed(wired: &Wired, agent: &Agent) -> Vec<String> {
    wired
        .projects
        .list(agent)
        .await
        .expect("listing should succeed")
        .iter()
        .map(|summary| summary.id.to_string())
        .collect()
}

#[tokio::test]
async fn an_account_sees_only_the_projects_it_may_read_newest_first() {
    // given: three projects started one after another
    let guarded = guarded();
    let ids = projects_started(&guarded, 3).await;
    // and: an account that may read the first, the last and something that is no project
    for readable in [ids[0].as_str(), ids[2].as_str(), "not-a-project-id"] {
        guarded.wired.give(readable, ADA, Some(Role::Reader)).await;
    }

    // then: it sees the last and the first, in that order
    assert_eq!(
        listed(&guarded.wired, &ada()).await,
        vec![ids[2].clone(), ids[0].clone()]
    );
}

#[tokio::test]
async fn whoever_may_read_everything_sees_every_project() {
    // given: two projects started one after another
    let guarded = guarded();
    let ids = projects_started(&guarded, 2).await;

    // then: the system, which may read everything, sees both, newest first
    assert_eq!(
        listed(&guarded.wired, &Agent::System).await,
        vec![ids[1].clone(), ids[0].clone()]
    );
}

#[tokio::test]
async fn an_account_in_no_project_sees_none() {
    // given: two projects, ada a member of neither
    let guarded = guarded();
    projects_started(&guarded, 2).await;

    // then: she sees none
    assert!(listed(&guarded.wired, &ada()).await.is_empty());
    // and: nobody signed in sees none either
    assert!(listed(&guarded.wired, &Agent::Local).await.is_empty());
}

#[tokio::test]
async fn whom_to_show_what_not_being_known_is_a_failure_not_an_empty_list() {
    // given: a project
    let guarded = guarded();
    projects_started(&guarded, 1).await;
    // and: what the account may read cannot be learned
    guarded.wired.access.go_down();

    // then: listing fails
    assert!(matches!(
        guarded.wired.projects.list(&ada()).await,
        Err(ProjectsError::Authorizer(_))
    ));
}
