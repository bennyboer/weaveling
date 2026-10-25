use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use authorizing::{Act, Authorizer, AuthorizerError, Readable};
use clock::FixedClock;
use eventsourcing::{AccountId, Agent};
use projects_core::{ProjectServiceError, RawDetails};
use time::{Duration, OffsetDateTime};

use crate::wiring;
use crate::wiring::Wired;

#[derive(Default)]
struct Reading {
    readable: Mutex<Option<Readable>>,
}

impl Reading {
    fn lets_read(&self, readable: Option<Readable>) {
        *self.readable.lock().expect("never poisoned") = readable;
    }
}

#[async_trait]
impl Authorizer for Reading {
    async fn may(
        &self,
        _agent: &Agent,
        _act: Act,
        _project: &str,
    ) -> Result<bool, AuthorizerError> {
        Ok(true)
    }

    async fn readable_by(&self, _agent: &Agent) -> Result<Readable, AuthorizerError> {
        self.readable
            .lock()
            .expect("never poisoned")
            .clone()
            .ok_or_else(|| AuthorizerError::Unreachable("the list is down".into()))
    }
}

fn ada() -> Agent {
    Agent::Account(AccountId::from("account_ada"))
}

struct Guarded {
    wired: Wired,
    reading: Arc<Reading>,
    clock: Arc<FixedClock>,
}

fn guarded() -> Guarded {
    let reading = Arc::new(Reading::default());
    let clock = Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH));

    Guarded {
        wired: wiring::wired_guarded_by(clock.clone(), reading.clone()),
        reading,
        clock,
    }
}

async fn started(guarded: &Guarded, count: i64) -> Vec<String> {
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
                .to_string(),
        );
    }
    guarded.wired.settle().await;

    ids
}

async fn listed(wired: &Wired) -> Vec<String> {
    wired
        .projects
        .list(&ada())
        .await
        .expect("listing should succeed")
        .iter()
        .map(|summary| summary.id.to_string())
        .collect()
}

#[tokio::test]
async fn an_account_sees_only_the_projects_it_may_read_newest_first() {
    let guarded = guarded();
    let ids = started(&guarded, 3).await;

    guarded
        .reading
        .lets_read(Some(Readable::Only(BTreeSet::from([
            ids[0].clone(),
            ids[2].clone(),
            "not-a-project-id".to_owned(),
        ]))));

    assert_eq!(
        listed(&guarded.wired).await,
        vec![ids[2].clone(), ids[0].clone()]
    );
}

#[tokio::test]
async fn whoever_may_read_everything_sees_every_project() {
    let guarded = guarded();
    let ids = started(&guarded, 2).await;

    guarded.reading.lets_read(Some(Readable::Everything));

    assert_eq!(
        listed(&guarded.wired).await,
        vec![ids[1].clone(), ids[0].clone()]
    );
}

#[tokio::test]
async fn an_account_in_no_project_sees_none() {
    let guarded = guarded();
    started(&guarded, 2).await;

    guarded
        .reading
        .lets_read(Some(Readable::Only(BTreeSet::new())));

    assert!(listed(&guarded.wired).await.is_empty());
}

#[tokio::test]
async fn whom_to_show_what_not_being_known_is_a_failure_not_an_empty_list() {
    let guarded = guarded();
    started(&guarded, 1).await;

    guarded.reading.lets_read(None);

    assert!(matches!(
        guarded.wired.projects.list(&ada()).await,
        Err(ProjectServiceError::Authorizer(_))
    ));
}
