use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use clock::FixedClock;
use messaging::{Message, Publisher, Undelivered};
use sqlx::{PgPool, Row};
use test_harness::PostgresFixture;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::agent::Agent;
use crate::aggregate::AggregateId;
use crate::event::{Event, Recorded};
use crate::metadata::EventMetadata;
use crate::postgres::sample::{codec, message_for};
use crate::postgres::{CLAIM_FOR, Delivered, KEPT_FOR, PostgresEventStore, PostgresOutbox};
use crate::store::EventStore;
use crate::testing::sample::{SAMPLE, SampleEvent, SampleKind};
use crate::version::Version;

struct Overheard {
    published: Mutex<Vec<Message>>,
    refusing: bool,
}

#[async_trait]
impl Publisher for Overheard {
    async fn publish(&self, message: Message) -> Result<(), Undelivered> {
        if self.refusing {
            return Err(Undelivered {
                routing: message.routing,
                because: "the broker is not listening".into(),
            });
        }

        self.published
            .lock()
            .expect("published lock poisoned")
            .push(message);

        Ok(())
    }
}

impl Overheard {
    fn listening() -> Arc<Self> {
        Arc::new(Self {
            published: Mutex::new(Vec::new()),
            refusing: false,
        })
    }

    fn refusing() -> Arc<Self> {
        Arc::new(Self {
            published: Mutex::new(Vec::new()),
            refusing: true,
        })
    }

    fn ids_heard(&self) -> Vec<Uuid> {
        self.published
            .lock()
            .expect("published lock poisoned")
            .iter()
            .map(|message| message.id.as_uuid())
            .collect()
    }
}

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
    store: PostgresEventStore<SampleEvent>,
    clock: Arc<FixedClock>,
}

impl Wired {
    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool = fixture.create_schema("outbox").await;
        crate::postgres::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down");

        Self {
            fixture,
            store: PostgresEventStore::new(pool.clone(), codec(), message_for),
            pool,
            clock: Arc::new(FixedClock::new(at(2_000))),
        }
    }

    fn relay(&self, publisher: Arc<Overheard>) -> PostgresOutbox {
        PostgresOutbox::new(self.pool.clone(), publisher, self.clock.clone())
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

    async fn message_ids(&self) -> Vec<Uuid> {
        sqlx::query_scalar("SELECT message_id FROM outbox ORDER BY entry")
            .fetch_all(&self.pool)
            .await
            .expect("reading the outbox should succeed")
    }

    async fn cleanup(self) {
        self.fixture.cleanup().await;
    }
}

fn nothing() -> Delivered {
    Delivered {
        published: 0,
        refused: 0,
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

#[tokio::test]
async fn the_relay_publishes_what_is_waiting_and_marks_it() {
    let wired = Wired::setup().await;
    let aggregate = AggregateId::from("sample_relayed");
    let heard = Overheard::listening();
    wired.append(&aggregate, 0, vec![a_creation()]).await;
    let written = wired.message_ids().await;

    let delivered = wired
        .relay(heard.clone())
        .deliver(10)
        .await
        .expect("delivering should succeed");

    assert_eq!(
        delivered,
        Delivered {
            published: 1,
            refused: 0
        }
    );
    assert_eq!(
        heard.ids_heard(),
        written,
        "the relay publishes the message it was handed, id and all"
    );
    assert_eq!(
        wired.unpublished().await,
        0,
        "a published entry is marked so it is never sent twice"
    );

    wired.cleanup().await;
}

#[tokio::test]
async fn the_relay_leaves_nothing_to_do_the_second_time() {
    let wired = Wired::setup().await;
    let aggregate = AggregateId::from("sample_once");
    let heard = Overheard::listening();
    wired.append(&aggregate, 0, vec![a_creation()]).await;

    let relay = wired.relay(heard.clone());
    relay.deliver(10).await.expect("the first pass publishes");
    let again = relay
        .deliver(10)
        .await
        .expect("the second pass finds nothing");

    assert_eq!(again, nothing());
    assert_eq!(heard.ids_heard().len(), 1);

    wired.cleanup().await;
}

#[tokio::test]
async fn a_message_the_broker_refuses_stays_waiting() {
    let wired = Wired::setup().await;
    let aggregate = AggregateId::from("sample_refused");
    wired.append(&aggregate, 0, vec![a_creation()]).await;

    let delivered = wired
        .relay(Overheard::refusing())
        .deliver(10)
        .await
        .expect("a refused publish is not a store failure");

    assert_eq!(
        delivered,
        Delivered {
            published: 0,
            refused: 1
        }
    );
    assert_eq!(
        wired.unpublished().await,
        1,
        "what the broker would not take must still be waiting"
    );

    wired.cleanup().await;
}

#[tokio::test]
async fn a_retried_message_keeps_the_id_it_was_written_with() {
    let wired = Wired::setup().await;
    let aggregate = AggregateId::from("sample_retried");
    let heard = Overheard::listening();
    wired.append(&aggregate, 0, vec![a_creation()]).await;
    let written = wired.message_ids().await;

    wired
        .relay(Overheard::refusing())
        .deliver(10)
        .await
        .expect("refusing is not a failure");
    wired
        .clock
        .set(at(2_000) + CLAIM_FOR + Duration::seconds(1));
    wired
        .relay(heard.clone())
        .deliver(10)
        .await
        .expect("delivering should succeed");

    assert_eq!(
        heard.ids_heard(),
        written,
        "a retry must not mint a new id, or the far end cannot recognise the redelivery"
    );

    wired.cleanup().await;
}

#[tokio::test]
async fn a_claimed_message_is_left_alone_until_the_claim_runs_out() {
    let wired = Wired::setup().await;
    let aggregate = AggregateId::from("sample_claimed");
    let heard = Overheard::listening();
    wired.append(&aggregate, 0, vec![a_creation()]).await;

    wired
        .relay(Overheard::refusing())
        .deliver(10)
        .await
        .expect("claiming and failing is not a failure");

    let too_soon = wired
        .relay(heard.clone())
        .deliver(10)
        .await
        .expect("delivering should succeed");

    assert_eq!(
        too_soon,
        nothing(),
        "another relay must not pick up an entry that is still claimed"
    );

    wired
        .clock
        .set(at(2_000) + CLAIM_FOR + Duration::seconds(1));
    let later = wired
        .relay(heard.clone())
        .deliver(10)
        .await
        .expect("delivering should succeed");

    assert_eq!(
        later,
        Delivered {
            published: 1,
            refused: 0
        },
        "an expired claim is what stops a crashed relay from stranding a message"
    );

    wired.cleanup().await;
}

#[tokio::test]
async fn the_relay_takes_no_more_than_it_was_asked_for() {
    let wired = Wired::setup().await;
    let aggregate = AggregateId::from("sample_batched");
    let heard = Overheard::listening();

    wired
        .append(
            &aggregate,
            0,
            vec![a_creation(), retitled("Second"), retitled("Third")],
        )
        .await;

    let delivered = wired
        .relay(heard.clone())
        .deliver(2)
        .await
        .expect("delivering should succeed");

    assert_eq!(delivered.published, 2);
    assert_eq!(wired.unpublished().await, 1);

    wired.cleanup().await;
}

async fn published_long_ago(wired: &Wired, when: OffsetDateTime) {
    sqlx::query("UPDATE outbox SET published_at = $1")
        .bind(when)
        .execute(&wired.pool)
        .await
        .expect("ageing an entry should succeed");
}

async fn entries(wired: &Wired) -> usize {
    wired.waiting().await.len()
}

#[tokio::test]
async fn an_entry_published_longer_ago_than_we_keep_them_is_deleted() {
    let wired = Wired::setup().await;
    let aggregate = AggregateId::from("sample_aged");
    wired.append(&aggregate, 0, vec![a_creation()]).await;
    published_long_ago(&wired, at(2_000) - KEPT_FOR - Duration::days(1)).await;

    let gone = wired
        .relay(Overheard::listening())
        .delete_published(at(2_000) - KEPT_FOR, 100)
        .await
        .expect("deleting should succeed");

    assert_eq!(gone, 1);
    assert_eq!(entries(&wired).await, 0);

    wired.cleanup().await;
}

#[tokio::test]
async fn an_entry_published_recently_is_kept() {
    let wired = Wired::setup().await;
    let aggregate = AggregateId::from("sample_recent");
    wired.append(&aggregate, 0, vec![a_creation()]).await;
    published_long_ago(&wired, at(2_000) - Duration::days(1)).await;

    let gone = wired
        .relay(Overheard::listening())
        .delete_published(at(2_000) - KEPT_FOR, 100)
        .await
        .expect("deleting should succeed");

    assert_eq!(gone, 0);
    assert_eq!(entries(&wired).await, 1);

    wired.cleanup().await;
}

#[tokio::test]
async fn an_entry_never_published_is_kept_however_old_it_is() {
    let wired = Wired::setup().await;
    let aggregate = AggregateId::from("sample_stuck");
    wired.append(&aggregate, 0, vec![a_creation()]).await;

    let gone = wired
        .relay(Overheard::listening())
        .delete_published(at(2_000) + Duration::days(3_650), 100)
        .await
        .expect("deleting should succeed");

    assert_eq!(gone, 0);
    assert_eq!(
        wired.unpublished().await,
        1,
        "an old unpublished entry is a stuck message, not rubbish — deleting it would lose it silently"
    );

    wired.cleanup().await;
}

#[tokio::test]
async fn deleting_takes_no_more_than_it_was_asked_for() {
    let wired = Wired::setup().await;
    let aggregate = AggregateId::from("sample_sweeping");
    wired
        .append(
            &aggregate,
            0,
            vec![a_creation(), retitled("Second"), retitled("Third")],
        )
        .await;
    published_long_ago(&wired, at(2_000) - KEPT_FOR - Duration::days(1)).await;

    let relay = wired.relay(Overheard::listening());
    let first = relay
        .delete_published(at(2_000) - KEPT_FOR, 2)
        .await
        .expect("deleting should succeed");

    assert_eq!(first, 2);
    assert_eq!(entries(&wired).await, 1);

    wired.cleanup().await;
}
