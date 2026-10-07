use std::sync::Arc;

use sqlx::{PgPool, Row};
use test_harness::PostgresFixture;
use time::{Duration, OffsetDateTime};

use crate::agent::Agent;
use crate::aggregate::AggregateId;
use crate::event::{Event, Recorded};
use crate::metadata::EventMetadata;
use crate::postgres::PostgresEventStore;
use crate::store::EventStore;
use crate::testing::sample::{SAMPLE, SampleEvent, SampleKind};
use crate::testing::stored::{codec, message_for};
use crate::version::Version;

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn recorded(aggregate: &AggregateId, version: u64, event: SampleEvent) -> Recorded<SampleEvent> {
    Recorded {
        metadata: EventMetadata {
            aggregate: aggregate.clone(),
            kind: SAMPLE,
            version: Version::of(version),
            agent: Agent::System,
            occurred_at: at(1_000),
            is_snapshot: event.is_snapshot(),
        },
        event,
    }
}

fn a_creation() -> SampleEvent {
    SampleEvent::Created {
        title: "The Loom".to_owned(),
        description: "A silent machine.".to_owned(),
        kind: SampleKind::Ordinary,
    }
}

fn a_snapshot() -> SampleEvent {
    SampleEvent::Snapshotted {
        title: "The Loom".to_owned(),
        description: "A silent machine.".to_owned(),
        deleted: false,
    }
}

fn retitled(what: &str) -> SampleEvent {
    SampleEvent::TitleUpdated(what.to_owned())
}

struct Wired {
    fixture: PostgresFixture,
    pool: PgPool,
    store: Arc<PostgresEventStore<SampleEvent>>,
}

impl Wired {
    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool = fixture.create_schema("announcing").await;
        crate::postgres::migrations()
            .run(&pool)
            .await
            .expect("the event store's schema should lay down");
        outbox::postgres::migrations()
            .run(&pool)
            .await
            .expect("the outbox's schema should lay down beside it");

        Self {
            fixture,
            store: Arc::new(PostgresEventStore::new(pool.clone(), codec(), message_for)),
            pool,
        }
    }

    async fn append(&self, aggregate: &AggregateId, expected: u64, events: Vec<SampleEvent>) {
        let stream: Vec<_> = events
            .into_iter()
            .enumerate()
            .map(|(nth, event)| recorded(aggregate, expected + nth as u64 + 1, event))
            .collect();

        self.store
            .append(aggregate, SAMPLE, Version::of(expected), &stream)
            .await
            .expect("appending should succeed");
    }

    async fn waiting(&self) -> Vec<(String, Option<OffsetDateTime>)> {
        sqlx::query("SELECT routing_key, published_at FROM outbox ORDER BY entry")
            .fetch_all(&self.pool)
            .await
            .expect("reading the outbox should succeed")
            .into_iter()
            .map(|row| (row.get::<String, _>(0), row.get(1)))
            .collect()
    }

    async fn unpublished(&self) -> usize {
        self.waiting()
            .await
            .iter()
            .filter(|(_, published)| published.is_none())
            .count()
    }

    async fn cleanup(self) {
        self.fixture.cleanup().await;
    }
}

#[tokio::test]
async fn an_append_leaves_a_message_waiting_for_every_publishable_event() {
    let wired = Wired::setup().await;
    let aggregate = AggregateId::from("sample_announced");

    wired
        .append(&aggregate, 0, vec![a_creation(), retitled("Second")])
        .await;

    let found = wired.waiting().await;

    assert_eq!(
        found
            .iter()
            .map(|(routing, _)| routing.as_str())
            .collect::<Vec<_>>(),
        vec!["sample.created", "sample.title_updated"]
    );
    assert_eq!(
        wired.unpublished().await,
        2,
        "an append enqueues, it does not publish"
    );

    wired.cleanup().await;
}

#[tokio::test]
async fn an_event_that_publishes_nothing_leaves_nothing_waiting() {
    let wired = Wired::setup().await;
    let aggregate = AggregateId::from("sample_quiet");

    wired
        .append(
            &aggregate,
            0,
            vec![
                a_creation(),
                SampleEvent::Corrected("Quietly".to_owned()),
                a_snapshot(),
            ],
        )
        .await;

    let found = wired.waiting().await;

    assert_eq!(
        found.len(),
        1,
        "a correction and a snapshot are both unpublishable, so only the creation waits: {found:?}"
    );

    wired.cleanup().await;
}

#[tokio::test]
async fn a_refused_append_leaves_no_message_waiting() {
    let wired = Wired::setup().await;
    let aggregate = AggregateId::from("sample_rolled_back");

    let refused = wired
        .store
        .append(
            &aggregate,
            SAMPLE,
            Version::of(7),
            &[recorded(&aggregate, 8, a_creation())],
        )
        .await;

    assert!(refused.is_err());
    assert!(
        wired.waiting().await.is_empty(),
        "the event and its message are written in one transaction or not at all"
    );

    wired.cleanup().await;
}

#[tokio::test]
async fn a_message_beside_an_event_that_rolls_back_goes_with_it() {
    let wired = Wired::setup().await;
    let aggregate = AggregateId::from("sample_half_announced");
    wired.append(&aggregate, 0, vec![a_creation()]).await;

    let refused = wired
        .store
        .append(
            &aggregate,
            SAMPLE,
            Version::of(1),
            &[
                recorded(&aggregate, 2, retitled("Second")),
                recorded(&aggregate, 1, retitled("Colliding")),
            ],
        )
        .await;

    assert!(refused.is_err());
    assert_eq!(
        wired.waiting().await.len(),
        1,
        "the surviving message is the first append's, not half of the second's"
    );

    wired.cleanup().await;
}
