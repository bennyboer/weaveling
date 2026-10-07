use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use clock::FixedClock;
use messaging::{Message, Publisher, RoutingKey, Undelivered};
use sqlx::{PgPool, Row};
use test_harness::PostgresFixture;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::outbox::{CLAIM_FOR, Delivered, KEPT_FOR, Origin, Outbox};
use crate::postgres::{PostgresOutbox, enqueue};

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

fn a_link_message() -> Message {
    Message::opening(
        RoutingKey::parse("passage.idea.linked").expect("a plain key is fine"),
        serde_json::json!({ "passage": "passage_1", "idea": "idea_1" }),
        at(2_000),
    )
}

fn from_a_passage() -> Origin<'static> {
    Origin {
        aggregate: "passage_1",
        kind: "passage",
        version: 0,
    }
}

struct Wired {
    fixture: PostgresFixture,
    pool: PgPool,
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
            pool,
            clock: Arc::new(FixedClock::new(at(2_000))),
        }
    }

    fn relay(&self, publisher: Arc<Overheard>) -> PostgresOutbox {
        PostgresOutbox::new(self.pool.clone(), publisher, self.clock.clone())
    }

    async fn waiting_messages(&self, how_many: usize) {
        let mut transaction = self.pool.begin().await.expect("a transaction opens");
        for _ in 0..how_many {
            enqueue(&mut transaction, from_a_passage(), &a_link_message())
                .await
                .expect("enqueueing should succeed");
        }
        transaction.commit().await.expect("the transaction commits");
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
async fn the_relay_publishes_what_is_waiting_and_marks_it() {
    let wired = Wired::setup().await;
    let heard = Overheard::listening();
    wired.waiting_messages(1).await;
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
    let heard = Overheard::listening();
    wired.waiting_messages(1).await;

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
    wired.waiting_messages(1).await;

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
    let heard = Overheard::listening();
    wired.waiting_messages(1).await;
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
    let heard = Overheard::listening();
    wired.waiting_messages(1).await;

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
    let heard = Overheard::listening();

    wired.waiting_messages(3).await;

    let delivered = wired
        .relay(heard.clone())
        .deliver(2)
        .await
        .expect("delivering should succeed");

    assert_eq!(delivered.published, 2);
    assert_eq!(wired.unpublished().await, 1);

    wired.cleanup().await;
}

#[tokio::test]
async fn an_entry_published_longer_ago_than_we_keep_them_is_deleted() {
    let wired = Wired::setup().await;
    wired.waiting_messages(1).await;
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
    wired.waiting_messages(1).await;
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
    wired.waiting_messages(1).await;

    let gone = wired
        .relay(Overheard::listening())
        .delete_published(at(2_000) + Duration::days(3_650), 100)
        .await
        .expect("deleting should succeed");

    assert_eq!(gone, 0);
    assert_eq!(
        wired.unpublished().await,
        1,
        "an old unpublished entry is a stuck message, not rubbish — deleting it would lose it \
         silently"
    );

    wired.cleanup().await;
}

#[tokio::test]
async fn deleting_takes_no_more_than_it_was_asked_for() {
    let wired = Wired::setup().await;
    wired.waiting_messages(3).await;
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

#[tokio::test]
async fn a_message_enqueued_beside_any_write_is_relayed_once_that_write_commits() {
    let wired = Wired::setup().await;
    let heard = Overheard::listening();
    let message = a_link_message();
    let mut transaction = wired.pool.begin().await.expect("a transaction opens");

    enqueue(&mut transaction, from_a_passage(), &message)
        .await
        .expect("enqueueing should succeed");
    transaction.commit().await.expect("the transaction commits");
    wired
        .relay(heard.clone())
        .deliver(10)
        .await
        .expect("delivering should succeed");

    assert_eq!(
        heard.ids_heard(),
        vec![message.id.as_uuid()],
        "a writer that is not event-sourced gets the same relay, id and all"
    );

    wired.cleanup().await;
}

#[tokio::test]
async fn a_message_enqueued_beside_a_write_that_rolls_back_goes_with_it() {
    let wired = Wired::setup().await;
    let mut transaction = wired.pool.begin().await.expect("a transaction opens");

    enqueue(&mut transaction, from_a_passage(), &a_link_message())
        .await
        .expect("enqueueing should succeed");
    transaction
        .rollback()
        .await
        .expect("the transaction rolls back");

    assert_eq!(
        wired.unpublished().await,
        0,
        "the whole point of enqueueing in the writer's transaction: a change that never \
         happened is never announced"
    );

    wired.cleanup().await;
}

#[tokio::test]
async fn the_outbox_keeps_its_own_ledger_beside_whoever_owns_the_database() {
    let wired = Wired::setup().await;

    let found: Vec<String> = sqlx::query_scalar(
        "SELECT tablename::text FROM pg_tables WHERE schemaname = $1 ORDER BY tablename",
    )
    .bind(wired.fixture.schema_of("outbox"))
    .fetch_all(&wired.pool)
    .await
    .expect("reading the catalog should succeed");

    assert_eq!(
        found,
        vec!["_sqlx_migrations_outbox", "outbox"],
        "a writer that is not event-sourced gets the outbox without an events table"
    );

    wired.cleanup().await;
}

#[tokio::test]
async fn the_schema_carries_the_indexes_the_queries_rely_on() {
    let wired = Wired::setup().await;

    let found: Vec<String> = sqlx::query_scalar(
        "SELECT indexname::text FROM pg_indexes WHERE schemaname = $1 ORDER BY indexname",
    )
    .bind(wired.fixture.schema_of("outbox"))
    .fetch_all(&wired.pool)
    .await
    .expect("reading the catalog should succeed");

    assert_eq!(
        found,
        vec![
            "_sqlx_migrations_outbox_pkey",
            "outbox_pkey",
            "outbox_published",
            "outbox_waiting",
        ]
    );

    wired.cleanup().await;
}
