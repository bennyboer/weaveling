use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use clock::FixedClock;
use messaging::{Message, Publisher, RoutingKey, Undelivered};
use sqlx::{Row, SqlitePool};
use test_harness::SqliteFixture;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::outbox::{CLAIM_FOR, Delivered, KEPT_FOR, Origin, Outbox};
use clock::text;

use crate::sqlite::{SqliteOutbox, enqueue};

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
    fixture: SqliteFixture,
    pool: SqlitePool,
    clock: Arc<FixedClock>,
}

impl Wired {
    async fn setup() -> Self {
        let fixture = SqliteFixture::setup();
        let pool = fixture.create_database("outbox").await;
        crate::sqlite::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down");

        Self {
            fixture,
            pool,
            clock: Arc::new(FixedClock::new(at(2_000))),
        }
    }

    fn relay(&self, publisher: Arc<Overheard>) -> SqliteOutbox {
        SqliteOutbox::new(self.pool.clone(), publisher, self.clock.clone())
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

    async fn waiting(&self) -> Vec<(String, Option<String>)> {
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
        sqlx::query_scalar::<_, String>("SELECT message_id FROM outbox ORDER BY entry")
            .fetch_all(&self.pool)
            .await
            .expect("reading the outbox should succeed")
            .iter()
            .map(|id| Uuid::parse_str(id).expect("a stored id is a uuid"))
            .collect()
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
    sqlx::query("UPDATE outbox SET published_at = ?1")
        .bind(text::written(when))
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
async fn the_outbox_never_announces_so_the_relay_has_to_poll() {
    let wired = Wired::setup().await;
    let mut notifications = wired
        .relay(Overheard::listening())
        .notifications()
        .await
        .expect("asking for notifications is not a failure");
    wired.waiting_messages(1).await;

    let woken =
        tokio::time::timeout(std::time::Duration::from_millis(50), notifications.wait()).await;

    assert!(
        woken.is_err(),
        "nothing wakes the relay in local mode, so a message waits for the next poll"
    );

    wired.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_started_relay_delivers_by_polling_alone() {
    let wired = Wired::setup().await;
    let heard = Overheard::listening();
    let relay = crate::relaying::RelayTask::started(
        Arc::new(wired.relay(heard.clone())),
        crate::relaying::Cadence {
            deliver_every: std::time::Duration::from_millis(20),
            ..crate::relaying::Cadence::default()
        },
    );

    wired.waiting_messages(2).await;

    let mut delivered = false;
    for _ in 0..500 {
        if heard.ids_heard().len() == 2 {
            delivered = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    relay.stop().await;

    assert!(
        delivered,
        "with no notification, the poll alone has to carry every message"
    );

    wired.cleanup().await;
}

#[tokio::test]
async fn messages_are_published_in_the_order_they_were_written() {
    let wired = Wired::setup().await;
    let heard = Overheard::listening();
    wired.waiting_messages(5).await;
    let written = wired.message_ids().await;

    wired
        .relay(heard.clone())
        .deliver(10)
        .await
        .expect("delivering should succeed");

    assert_eq!(
        heard.ids_heard(),
        written,
        "a detach overtaking the attach it undoes would leave a read model wrong for good"
    );

    wired.cleanup().await;
}
