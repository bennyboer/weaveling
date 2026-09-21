use std::sync::Arc;

use time::{Duration, OffsetDateTime};
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::postgres::outbox::{KEPT_FOR, PostgresOutbox};

#[derive(Debug, Clone, Copy)]
pub struct Cadence {
    pub deliver_every: std::time::Duration,
    pub deliver_at_most: i64,
    pub sweep_every: std::time::Duration,
    pub sweep_at_most: i64,
    pub kept_for: Duration,
}

impl Default for Cadence {
    fn default() -> Self {
        Self {
            deliver_every: std::time::Duration::from_millis(250),
            deliver_at_most: 128,
            sweep_every: std::time::Duration::from_secs(60 * 60),
            sweep_at_most: 1_000,
            kept_for: KEPT_FOR,
        }
    }
}

pub struct RelayTask {
    stopping: watch::Sender<bool>,
    delivering: JoinHandle<()>,
    sweeping: JoinHandle<()>,
}

impl RelayTask {
    pub fn started(outbox: Arc<PostgresOutbox>, cadence: Cadence) -> Self {
        let (stopping, stopped) = watch::channel(false);

        Self {
            delivering: tokio::spawn(delivering(outbox.clone(), cadence, stopped.clone())),
            sweeping: tokio::spawn(sweeping(outbox, cadence, stopped)),
            stopping,
        }
    }

    pub async fn stop(self) {
        let _ = self.stopping.send(true);
        let _ = self.delivering.await;
        let _ = self.sweeping.await;
    }
}

async fn delivering(
    outbox: Arc<PostgresOutbox>,
    cadence: Cadence,
    mut stopped: watch::Receiver<bool>,
) {
    while waiting(cadence.deliver_every, &mut stopped).await {
        match outbox.deliver(cadence.deliver_at_most).await {
            Ok(delivered) if delivered.refused > 0 => {
                tracing::warn!(
                    published = delivered.published,
                    refused = delivered.refused,
                    "the outbox could not hand everything over"
                );
            }
            Ok(_) => {}
            Err(why) => tracing::error!(error = %why, "the outbox could not be drained"),
        }
    }
}

async fn sweeping(
    outbox: Arc<PostgresOutbox>,
    cadence: Cadence,
    mut stopped: watch::Receiver<bool>,
) {
    while waiting(cadence.sweep_every, &mut stopped).await {
        swept(&outbox, cadence, &mut stopped).await;
    }
}

async fn swept(
    outbox: &PostgresOutbox,
    cadence: Cadence,
    stopped: &mut watch::Receiver<bool>,
) -> u64 {
    let before = OffsetDateTime::now_utc() - cadence.kept_for;
    let mut gone = 0;

    while !*stopped.borrow() {
        match outbox.delete_published(before, cadence.sweep_at_most).await {
            Ok(taken) => {
                gone += taken;

                if taken < cadence.sweep_at_most as u64 {
                    break;
                }
            }
            Err(why) => {
                tracing::error!(error = %why, "the outbox could not be swept");
                break;
            }
        }
    }

    if gone > 0 {
        tracing::info!(gone, "swept published outbox entries");
    }

    gone
}

async fn waiting(every: std::time::Duration, stopped: &mut watch::Receiver<bool>) -> bool {
    tokio::select! {
        _ = stopped.changed() => false,
        _ = tokio::time::sleep(every) => true,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use clock::SystemClock;
    use messaging::{Message, Publisher, Undelivered};
    use test_harness::PostgresFixture;

    use super::*;
    use crate::agent::Agent;
    use crate::aggregate::AggregateId;
    use crate::event::{Event, Recorded};
    use crate::metadata::EventMetadata;
    use crate::postgres::sample::{codec, message_for};
    use crate::postgres::{PostgresEventStore, PostgresOutbox};
    use crate::store::EventStore;
    use crate::testing::sample::{SAMPLE, SampleEvent, SampleKind};
    use crate::version::Version;

    #[derive(Default)]
    struct Overheard {
        published: Mutex<Vec<Message>>,
    }

    #[async_trait]
    impl Publisher for Overheard {
        async fn publish(&self, message: Message) -> Result<(), Undelivered> {
            self.published
                .lock()
                .expect("published lock poisoned")
                .push(message);

            Ok(())
        }
    }

    impl Overheard {
        fn how_many(&self) -> usize {
            self.published
                .lock()
                .expect("published lock poisoned")
                .len()
        }
    }

    fn briskly() -> Cadence {
        Cadence {
            deliver_every: std::time::Duration::from_millis(10),
            sweep_every: std::time::Duration::from_millis(10),
            deliver_at_most: 16,
            sweep_at_most: 16,
            kept_for: Duration::days(90),
        }
    }

    async fn a_captured_sample(store: &PostgresEventStore<SampleEvent>, aggregate: &AggregateId) {
        let event = SampleEvent::Created {
            title: "The Loom".to_owned(),
            description: "A silent machine.".to_owned(),
            kind: SampleKind::Ordinary,
        };

        store
            .append(
                aggregate,
                SAMPLE,
                Version::ZERO,
                &[Recorded {
                    metadata: EventMetadata {
                        aggregate: aggregate.clone(),
                        kind: SAMPLE,
                        version: Version::of(1),
                        agent: Agent::System,
                        occurred_at: OffsetDateTime::UNIX_EPOCH,
                        is_snapshot: event.is_snapshot(),
                    },
                    event,
                }],
            )
            .await
            .expect("appending should succeed");
    }

    async fn until(what: impl Fn() -> bool) -> bool {
        for _ in 0..200 {
            if what() {
                return true;
            }

            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        false
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_started_relay_delivers_without_being_asked() {
        let fixture = PostgresFixture::setup().await;
        let pool = fixture.create_schema("relaying").await;
        crate::postgres::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down");

        let store = PostgresEventStore::new(pool.clone(), codec(), message_for);
        let heard = Arc::new(Overheard::default());
        let relay = RelayTask::started(
            Arc::new(PostgresOutbox::new(
                pool.clone(),
                heard.clone(),
                Arc::new(SystemClock),
            )),
            briskly(),
        );

        a_captured_sample(&store, &AggregateId::from("sample_relayed")).await;

        assert!(
            until(|| heard.how_many() == 1).await,
            "nobody called deliver, so the task has to have done it itself"
        );

        relay.stop().await;
        fixture.cleanup().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_stopped_relay_delivers_nothing_more() {
        let fixture = PostgresFixture::setup().await;
        let pool = fixture.create_schema("relaying").await;
        crate::postgres::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down");

        let store = PostgresEventStore::new(pool.clone(), codec(), message_for);
        let heard = Arc::new(Overheard::default());
        let relay = RelayTask::started(
            Arc::new(PostgresOutbox::new(
                pool.clone(),
                heard.clone(),
                Arc::new(SystemClock),
            )),
            briskly(),
        );

        a_captured_sample(&store, &AggregateId::from("sample_first")).await;
        assert!(until(|| heard.how_many() == 1).await);

        relay.stop().await;
        a_captured_sample(&store, &AggregateId::from("sample_second")).await;
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        assert_eq!(
            heard.how_many(),
            1,
            "a stopped relay must let go of the database, or shutdown would never finish"
        );

        let waiting: i64 =
            sqlx::query_scalar("SELECT count(*) FROM outbox WHERE published_at IS NULL")
                .fetch_one(&pool)
                .await
                .expect("counting should succeed");
        assert_eq!(
            waiting, 1,
            "what it did not deliver is still there to deliver"
        );

        fixture.cleanup().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_sweep_clears_a_backlog_larger_than_one_batch() {
        let fixture = PostgresFixture::setup().await;
        let pool = fixture.create_schema("sweeping").await;
        crate::postgres::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down");

        let store = PostgresEventStore::new(pool.clone(), codec(), message_for);
        for nth in 0..40 {
            a_captured_sample(&store, &AggregateId::from(format!("sample_{nth}").as_str())).await;
        }
        sqlx::query("UPDATE outbox SET published_at = NOW() - INTERVAL '200 days'")
            .execute(&pool)
            .await
            .expect("ageing the entries should succeed");

        let outbox = PostgresOutbox::new(
            pool.clone(),
            Arc::new(Overheard::default()),
            Arc::new(SystemClock),
        );
        let (_stopping, mut stopped) = watch::channel(false);

        let gone = swept(
            &outbox,
            Cadence {
                sweep_at_most: 7,
                ..briskly()
            },
            &mut stopped,
        )
        .await;

        assert_eq!(gone, 40, "one pass has to clear the backlog, not 7 of it");

        let left: i64 = sqlx::query_scalar("SELECT count(*) FROM outbox")
            .fetch_one(&pool)
            .await
            .expect("counting should succeed");
        assert_eq!(
            left, 0,
            "a batch bounds each statement, never the work a pass is willing to do"
        );

        fixture.cleanup().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_sweep_asked_to_stop_stops_mid_backlog() {
        let fixture = PostgresFixture::setup().await;
        let pool = fixture.create_schema("sweeping").await;
        crate::postgres::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down");

        let store = PostgresEventStore::new(pool.clone(), codec(), message_for);
        for nth in 0..10 {
            a_captured_sample(&store, &AggregateId::from(format!("sample_{nth}").as_str())).await;
        }
        sqlx::query("UPDATE outbox SET published_at = NOW() - INTERVAL '200 days'")
            .execute(&pool)
            .await
            .expect("ageing the entries should succeed");

        let outbox = PostgresOutbox::new(
            pool.clone(),
            Arc::new(Overheard::default()),
            Arc::new(SystemClock),
        );
        let (stopping, mut stopped) = watch::channel(false);
        stopping.send(true).expect("the receiver is still here");

        let gone = swept(&outbox, briskly(), &mut stopped).await;

        assert_eq!(
            gone, 0,
            "a draining loop still has to let go when shutdown asks it to"
        );

        fixture.cleanup().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_sweep_takes_the_few_there_are_without_waiting_for_a_full_batch() {
        let fixture = PostgresFixture::setup().await;
        let pool = fixture.create_schema("sweeping").await;
        crate::postgres::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down");

        let store = PostgresEventStore::new(pool.clone(), codec(), message_for);
        for nth in 0..3 {
            a_captured_sample(&store, &AggregateId::from(format!("sample_{nth}").as_str())).await;
        }
        sqlx::query("UPDATE outbox SET published_at = NOW() - INTERVAL '200 days'")
            .execute(&pool)
            .await
            .expect("ageing the entries should succeed");

        let outbox = PostgresOutbox::new(
            pool.clone(),
            Arc::new(Overheard::default()),
            Arc::new(SystemClock),
        );
        let (_stopping, mut stopped) = watch::channel(false);

        let gone = swept(
            &outbox,
            Cadence {
                sweep_at_most: 1_000,
                ..briskly()
            },
            &mut stopped,
        )
        .await;

        assert_eq!(
            gone, 3,
            "a short batch means the backlog is drained, never that we are waiting for a full one"
        );

        let left: i64 = sqlx::query_scalar("SELECT count(*) FROM outbox")
            .fetch_one(&pool)
            .await
            .expect("counting should succeed");
        assert_eq!(left, 0);

        fixture.cleanup().await;
    }
}
