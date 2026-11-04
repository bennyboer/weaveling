use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use authorizing::{Act, Authorizer, AuthorizerError, Readable};
use clock::FixedClock;
use eventsourcing::{Agent, InMemoryEventStore};
use ideas_catalog::InMemoryIdeaCatalog;
use ideas_contract::{MORE_TO_SWEEP, MoreToSweepDTO};
use ideas_core::{IdeaCatalog, IdeaId, IdeasCore, IdeasError, ProjectLink};
use ideas_messaging::{AT_MOST, DiscardOnProjectDeletedListener};
use messaging::{FakePublisher, Listener, Message, RoutingKey};
use projects_contract::DELETED;
use serde_json::json;
use time::{Duration, OffsetDateTime};
use transacting::InMemoryTransactions;

use crate::wiring;

const A_PROJECT: &str = "project_1";

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

#[derive(Default)]
struct TheSystemOnly {
    down: AtomicBool,
}

#[async_trait]
impl Authorizer for TheSystemOnly {
    async fn may(&self, agent: &Agent, _act: Act, _project: &str) -> Result<bool, AuthorizerError> {
        if self.down.load(Ordering::Acquire) {
            return Err(AuthorizerError::Unreachable("the list is down".into()));
        }

        Ok(*agent == Agent::System)
    }

    async fn readable_by(&self, _agent: &Agent) -> Result<Readable, AuthorizerError> {
        Ok(Readable::Everything)
    }
}

struct Wired {
    sweep: DiscardOnProjectDeletedListener,
    authorizer: Arc<TheSystemOnly>,
    ideas: IdeasCore,
    catalog: Arc<InMemoryIdeaCatalog>,
    overheard: Arc<FakePublisher>,
}

fn a_workbench(at_most: usize) -> Wired {
    let clock = Arc::new(FixedClock::new(at(1_000)));
    let catalog = Arc::new(InMemoryIdeaCatalog::new());
    let overheard = Arc::new(FakePublisher::default());
    let authorizer = Arc::new(TheSystemOnly::default());
    let ideas = IdeasCore::new(
        Arc::new(InMemoryEventStore::new()),
        Arc::new(InMemoryTransactions::new()),
        catalog.clone(),
        authorizer.clone(),
        clock.clone(),
    );

    Wired {
        sweep: DiscardOnProjectDeletedListener::new(ideas.clone(), overheard.clone(), clock)
            .taking_at_most(at_most),
        authorizer,
        ideas,
        catalog,
        overheard,
    }
}

impl Wired {
    async fn holding(&self, ideas: usize) -> Vec<IdeaId> {
        let mut captured = Vec::new();

        for nth in 0..ideas {
            let id = self
                .ideas
                .capture(A_PROJECT, &format!("Idea {nth}"), &Agent::System)
                .await
                .expect("capturing should succeed")
                .0;

            self.ideas
                .catalogue(None, &id)
                .await
                .expect("cataloguing should succeed");
            captured.push(id);
        }
        captured.sort();

        captured
    }

    async fn still_listed(&self) -> usize {
        self.catalog
            .in_project(&ProjectLink::from(A_PROJECT))
            .await
            .expect("listing should succeed")
            .len()
    }

    fn carried_on(&self) -> Vec<Message> {
        self.overheard
            .heard()
            .into_iter()
            .filter(|message| message.routing.to_string() == MORE_TO_SWEEP)
            .collect()
    }

    async fn sweep_through(&self, asked: Message) {
        let mut asked = asked;

        loop {
            let before = self.carried_on().len();
            self.sweep.handle(&asked).await.expect("sweeping succeeds");
            self.forget_what_was_discarded().await;

            let carried = self.carried_on();
            if carried.len() == before {
                break;
            }

            asked = carried.last().expect("a continuation was sent").clone();
        }
    }

    async fn forget_what_was_discarded(&self) {
        for summary in self
            .catalog
            .in_project(&ProjectLink::from(A_PROJECT))
            .await
            .expect("listing should succeed")
        {
            let discarded = self
                .ideas
                .read(&summary.id.to_string(), &Agent::System)
                .await
                .expect("a listed idea can be read")
                .state
                .is_discarded();
            if discarded {
                self.ideas
                    .catalogue(None, &summary.id)
                    .await
                    .expect("cataloguing should succeed");
            }
        }
    }
}

fn a_deletion() -> Message {
    Message::opening(
        RoutingKey::parse(DELETED).expect("a declared routing key is fine"),
        json!({
            "event": { "version": 0, "name": "DELETED" },
            "aggregate": { "id": A_PROJECT, "kind": "project", "version": 2 },
            "agent": { "kind": "system", "id": null },
            "occurred_at": "1970-01-01T00:16:40Z",
        }),
        at(1_000),
    )
}

#[tokio::test]
async fn an_idea_discarded_before_the_sweep_reaches_it_is_passed_over() {
    // given: a project holding two ideas
    let wired = a_workbench(4);
    let held = wired.holding(2).await;
    // and: one of them discarded
    wired
        .ideas
        .discard(&held[0].to_string(), None, &Agent::System)
        .await
        .expect("discarding should succeed");

    // when: the project's deletion is heard
    wired
        .sweep
        .handle(&a_deletion())
        .await
        .expect("an idea discarded meanwhile is no reason to fail");
    wired.forget_what_was_discarded().await;

    // then: none of its ideas is listed
    assert_eq!(wired.still_listed().await, 0);
}

#[tokio::test]
async fn whom_to_let_in_not_being_known_fails_the_sweep_so_it_is_tried_again() {
    // given: a project holding ideas
    let wired = a_workbench(4);
    wired.holding(2).await;
    // and: whom to let in not known
    wired.authorizer.down.store(true, Ordering::Release);

    // then: hearing the project's deletion fails
    assert!(
        wired.sweep.handle(&a_deletion()).await.is_err(),
        "a sweep that quietly skipped what it could not discard would leave the project's ideas behind"
    );
}

#[tokio::test]
async fn a_project_smaller_than_one_batch_is_swept_in_one_go() {
    // given: a project holding fewer ideas than a batch
    let wired = a_workbench(4);
    wired.holding(3).await;

    // when: its deletion is heard
    wired
        .sweep
        .handle(&a_deletion())
        .await
        .expect("sweeping succeeds");
    wired.forget_what_was_discarded().await;

    // then: none of its ideas is listed
    assert_eq!(wired.still_listed().await, 0);
    // and: the sweep does not carry on
    assert!(
        wired.carried_on().is_empty(),
        "a batch that did not fill is the last one, and carrying on would never end"
    );
}

#[tokio::test]
async fn a_project_larger_than_one_batch_is_swept_batch_by_batch() {
    // given: a project holding more ideas than a batch
    let wired = a_workbench(2);
    wired.holding(7).await;

    // when: its deletion is swept through
    wired.sweep_through(a_deletion()).await;

    // then: none of its ideas is listed
    assert_eq!(
        wired.still_listed().await,
        0,
        "a sweep bounded at two ideas must carry itself on until the project is empty, \
         or a book of thousands is swept only as far as its first batch"
    );
    // and: the sweep carried on once per full batch
    assert_eq!(
        wired.carried_on().len(),
        3,
        "seven ideas in batches of two is three continuations and a short fourth batch"
    );
}

#[tokio::test]
async fn a_continuation_stays_in_the_conversation_the_deletion_opened() {
    // given: a project holding two batches of ideas
    let wired = a_workbench(2);
    wired.holding(4).await;
    // and: its deletion
    let asked = a_deletion();

    // when: the deletion is heard
    wired.sweep.handle(&asked).await.expect("sweeping succeeds");

    // then: the sweep carries on in the deletion's conversation
    let carried = wired.carried_on();
    assert_eq!(carried.len(), 1);
    assert_eq!(
        carried[0].conversation, asked.conversation,
        "the whole sweep has to be followable as one thing, however many batches it takes"
    );
    // and: as caused by the deletion
    assert_eq!(carried[0].caused_by, Some(asked.id));
}

#[tokio::test]
async fn a_continuation_starts_after_the_idea_it_names() {
    // given: a project holding two batches of ideas
    let wired = a_workbench(2);
    let captured = wired.holding(4).await;

    // when: its deletion is heard
    wired
        .sweep
        .handle(&a_deletion())
        .await
        .expect("sweeping succeeds");

    // then: the sweep carries on after the last idea of the batch
    let asked: MoreToSweepDTO =
        serde_json::from_value(wired.carried_on()[0].payload.clone()).expect("readable");

    assert_eq!(
        asked.after,
        captured[1].to_string(),
        "the cursor is the last idea of the batch just done, so progress cannot stall on \
         a projection that has not caught up"
    );
    assert_eq!(asked.project, A_PROJECT);
}

#[tokio::test]
async fn a_sweep_of_a_project_holding_nothing_carries_nothing_on() {
    // given: a project holding nothing
    let wired = a_workbench(2);

    // when: its deletion is heard
    wired
        .sweep
        .handle(&a_deletion())
        .await
        .expect("sweeping succeeds");

    // then: the sweep does not carry on
    assert!(wired.carried_on().is_empty());
}

#[tokio::test]
async fn a_continuation_nobody_can_read_is_refused() {
    // given: a continuation nobody can read
    let wired = a_workbench(2);
    let nonsense = Message::opening(
        RoutingKey::parse(MORE_TO_SWEEP).expect("a declared routing key is fine"),
        json!({ "nothing": "useful" }),
        at(1_000),
    );

    // then: it is refused
    assert!(
        wired.sweep.handle(&nonsense).await.is_err(),
        "a continuation the sweep cannot act on belongs in dead letters, not dropped"
    );
}

#[tokio::test]
async fn a_project_larger_than_one_batch_is_swept_to_the_end_by_the_wired_sweep() {
    // given: a project holding more ideas than a batch
    let wired = wiring::wired(Arc::new(FixedClock::new(at(1_000))));
    for nth in 0..=AT_MOST {
        wired
            .ideas
            .capture(A_PROJECT, &format!("Idea {nth}"), &Agent::System)
            .await
            .expect("capturing should succeed");
    }
    wired.settle().await;

    // when: its deletion is published
    wired.outbox.enqueue(a_deletion());
    wired.settle().await;

    // then: none of its ideas is listed
    assert!(
        wired
            .catalog
            .in_project(&ProjectLink::from(A_PROJECT))
            .await
            .expect("listing should succeed")
            .is_empty(),
        "every batch after the first is asked for by the sweep itself"
    );
}

#[tokio::test]
async fn a_deleted_project_takes_no_new_idea() {
    // given: a deleted project
    let wired = a_workbench(4);

    wired
        .sweep
        .handle(&a_deletion())
        .await
        .expect("sweeping succeeds");
    // when: an idea is captured in it
    let refused = wired
        .ideas
        .capture(A_PROJECT, "Too late", &Agent::System)
        .await;

    // then: it is refused
    assert!(matches!(refused, Err(IdeasError::ProjectNotThere)));
}

#[tokio::test]
async fn an_idea_catalogued_only_after_its_project_was_swept_is_discarded_then() {
    // given: an idea not yet catalogued
    let wired = a_workbench(4);
    let late = wired
        .ideas
        .capture(A_PROJECT, "Not yet listed", &Agent::System)
        .await
        .expect("capturing should succeed")
        .0;
    // and: its project swept
    wired
        .sweep
        .handle(&a_deletion())
        .await
        .expect("sweeping succeeds");

    // when: the idea is catalogued
    wired
        .ideas
        .catalogue(None, &late)
        .await
        .expect("cataloguing should succeed");
    wired
        .ideas
        .catalogue(None, &late)
        .await
        .expect("cataloguing the discard should succeed");

    // then: it is discarded
    assert!(
        wired
            .ideas
            .read(&late.to_string(), &Agent::System)
            .await
            .expect("the idea is still there to read")
            .state
            .is_discarded(),
        "the sweep reads the catalog, which had not heard of the idea yet, so cataloguing it discards it"
    );
    // and: it is not listed
    assert_eq!(wired.still_listed().await, 0);
}

#[tokio::test]
async fn a_continuation_of_the_sweep_lays_no_tombstone() {
    // given: a project never deleted
    let wired = a_workbench(4);

    // when: a continuation of its sweep is heard
    wired
        .sweep
        .handle(&Message::opening(
            RoutingKey::parse(MORE_TO_SWEEP).expect("a declared routing key is fine"),
            json!({ "project": A_PROJECT, "after": IdeaId::generate(at(500)).to_string() }),
            at(1_000),
        ))
        .await
        .expect("sweeping succeeds");

    // then: the project still takes new ideas
    assert!(
        wired
            .ideas
            .capture(A_PROJECT, "Still welcome", &Agent::System)
            .await
            .is_ok(),
        "only the deletion itself buries the project"
    );
}
