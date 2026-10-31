use std::sync::Arc;

use async_trait::async_trait;
use clock::Clock;
use messaging::{ListenerName, MessageId};
use sqlx::migrate::Migrator;
use time::OffsetDateTime;
use transacting::Transaction;

use crate::{Inbox, InboxError};

const LEDGER: &str = "_sqlx_migrations_inbox";

const HEAR: &str = "
    INSERT INTO inbox (listener, message_id, heard_at)
    VALUES ($1, $2, $3)
    ON CONFLICT (listener, message_id) DO NOTHING
";

const FORGET: &str = "
    DELETE FROM inbox
    WHERE (listener, message_id) IN (
        SELECT listener, message_id
        FROM inbox
        WHERE heard_at < $1
        ORDER BY heard_at
        LIMIT $2
    )
";

pub fn migrations() -> Migrator {
    let mut laying = sqlx::migrate!("./migrations/postgres");
    laying.dangerous_set_table_name(LEDGER);

    laying
}

pub struct PostgresInbox {
    clock: Arc<dyn Clock>,
}

impl PostgresInbox {
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self { clock }
    }
}

#[async_trait]
impl Inbox for PostgresInbox {
    async fn first_time(
        &self,
        transaction: &mut Transaction,
        listener: &ListenerName,
        message: MessageId,
    ) -> Result<bool, InboxError> {
        let heard = sqlx::query(HEAR)
            .bind(listener.as_str())
            .bind(message.as_uuid())
            .bind(self.clock.now())
            .execute(transacting::postgres::connection(transaction)?)
            .await
            .map_err(unreachable)?;

        Ok(heard.rows_affected() == 1)
    }

    async fn forget_heard_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, InboxError> {
        let forgotten = sqlx::query(FORGET)
            .bind(before)
            .bind(i64::try_from(at_most).unwrap_or(i64::MAX))
            .execute(transacting::postgres::connection(transaction)?)
            .await
            .map_err(unreachable)?;

        Ok(forgotten.rows_affected())
    }
}

fn unreachable(failure: sqlx::Error) -> InboxError {
    InboxError::Unreachable(failure.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use clock::FixedClock;
    use test_harness::PostgresFixture;
    use transacting::{PostgresTransactions, Transactions};

    use super::PostgresInbox;
    use crate::suite::Workbench;

    struct OnPostgres {
        fixture: PostgresFixture,
        inbox: PostgresInbox,
        transactions: PostgresTransactions,
    }

    #[async_trait]
    impl Workbench for OnPostgres {
        type Inbox = PostgresInbox;

        async fn setup(clock: Arc<FixedClock>) -> Self {
            let fixture = PostgresFixture::setup().await;
            let pool = fixture.create_schema("inbox").await;
            super::migrations()
                .run(&pool)
                .await
                .expect("the inbox should lay down in an empty namespace");

            Self {
                fixture,
                inbox: PostgresInbox::new(clock),
                transactions: PostgresTransactions::new(pool),
            }
        }

        fn inbox(&self) -> &PostgresInbox {
            &self.inbox
        }

        fn transactions(&self) -> &dyn Transactions {
            &self.transactions
        }

        async fn cleanup(self) {
            self.fixture.cleanup().await;
        }
    }

    crate::conformance_tests!(OnPostgres);
}
