use std::sync::{Arc, Mutex};

use clock::FixedClock;
use eventsourcing::{Agent, InMemoryEventStore, PublishingEventStore, Version};
use ideas_catalog::InMemoryIdeaCatalog;
use ideas_contract::{MORE_TO_SWEEP, MoreToSweepDTO};
use ideas_core::{IdeaCatalog, IdeaId, IdeaService, IdeaSummary, IdeaTitle, ProjectLink};
use ideas_messaging::{DiscardOnProjectDeleted, IdeaEventPublisher};
use messaging::{Listener, Message, Publisher, RoutingKey, Undelivered};
use projects_contract::DELETED;
use serde_json::json;
use time::{Duration, OffsetDateTime};

const A_PROJECT: &str = "project_1";

#[derive(Default)]
struct Overheard {
    heard: Mutex<Vec<Message>>,
}

#[async_trait::async_trait]
impl Publisher for Overheard {
    async fn publish(&self, message: Message) -> Result<(), Undelivered> {
        self.heard.lock().expect("lock poisoned").push(message);

        Ok(())
    }
}

impl Overheard {
    fn what_it_heard(&self) -> Vec<Message> {
        self.heard.lock().expect("lock poisoned").clone()
    }
}

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

struct Wired {
    sweep: DiscardOnProjectDeleted,
    ideas: IdeaService,
    catalog: Arc<InMemoryIdeaCatalog>,
    overheard: Arc<Overheard>,
}

fn a_workbench(at_most: usize) -> Wired {
    let clock = Arc::new(FixedClock::new(at(1_000)));
    let catalog = Arc::new(InMemoryIdeaCatalog::new());
    let overheard = Arc::new(Overheard::default());
    let ideas = IdeaService::new(
        PublishingEventStore::wrapping(
            Arc::new(InMemoryEventStore::new()),
            Arc::new(IdeaEventPublisher::new(overheard.clone())),
        ),
        catalog.clone(),
        clock.clone(),
    );

    Wired {
        sweep: DiscardOnProjectDeleted::new(
            ideas.clone(),
            catalog.clone(),
            overheard.clone(),
            clock,
        )
        .taking_at_most(at_most),
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
                .expect("capturing should succeed");

            self.catalog
                .remember(&IdeaSummary {
                    id,
                    version: Version::of(1),
                    project: ProjectLink::from(A_PROJECT),
                    title: IdeaTitle::new(&format!("Idea {nth}")).expect("a plain title is fine"),
                })
                .await
                .expect("remembering should succeed");
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
            .what_it_heard()
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
            let standing = self
                .ideas
                .get(&summary.id.to_string())
                .await
                .expect("reading should succeed");

            if standing.state.is_discarded() {
                self.catalog
                    .forget(&summary.id)
                    .await
                    .expect("forgetting should succeed");
            }
        }
    }
}

fn deleted() -> Message {
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
async fn a_project_smaller_than_one_batch_is_swept_in_one_go() {
    let wired = a_workbench(4);
    wired.holding(3).await;

    wired
        .sweep
        .handle(&deleted())
        .await
        .expect("sweeping succeeds");
    wired.forget_what_was_discarded().await;

    assert_eq!(wired.still_listed().await, 0);
    assert!(
        wired.carried_on().is_empty(),
        "a batch that did not fill is the last one, and carrying on would never end"
    );
}

#[tokio::test]
async fn a_project_larger_than_one_batch_is_swept_batch_by_batch() {
    let wired = a_workbench(2);
    wired.holding(7).await;

    wired.sweep_through(deleted()).await;

    assert_eq!(
        wired.still_listed().await,
        0,
        "a sweep bounded at two ideas must carry itself on until the project is empty, \
         or a book of thousands is swept only as far as its first batch"
    );
    assert_eq!(
        wired.carried_on().len(),
        3,
        "seven ideas in batches of two is three continuations and a short fourth batch"
    );
}

#[tokio::test]
async fn a_continuation_stays_in_the_conversation_the_deletion_opened() {
    let wired = a_workbench(2);
    wired.holding(4).await;
    let asked = deleted();

    wired.sweep.handle(&asked).await.expect("sweeping succeeds");

    let carried = wired.carried_on();
    assert_eq!(carried.len(), 1);
    assert_eq!(
        carried[0].conversation, asked.conversation,
        "the whole sweep has to be followable as one thing, however many batches it takes"
    );
    assert_eq!(carried[0].caused_by, Some(asked.id));
}

#[tokio::test]
async fn a_continuation_starts_after_the_idea_it_names() {
    let wired = a_workbench(2);
    let captured = wired.holding(4).await;

    wired
        .sweep
        .handle(&deleted())
        .await
        .expect("sweeping succeeds");

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
    let wired = a_workbench(2);

    wired
        .sweep
        .handle(&deleted())
        .await
        .expect("sweeping succeeds");

    assert!(wired.carried_on().is_empty());
}

#[tokio::test]
async fn a_continuation_nobody_can_read_is_refused() {
    let wired = a_workbench(2);
    let nonsense = Message::opening(
        RoutingKey::parse(MORE_TO_SWEEP).expect("a declared routing key is fine"),
        json!({ "nothing": "useful" }),
        at(1_000),
    );

    assert!(
        wired.sweep.handle(&nonsense).await.is_err(),
        "a continuation the sweep cannot act on belongs in dead letters, not dropped"
    );
}
