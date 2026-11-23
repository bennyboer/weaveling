use async_trait::async_trait;
use sqlx::SqlitePool;
use test_harness::SqliteFixture;
use transacting::{SqliteTransactions, Transactions};

use crate::sqlite;
use crate::suite::Workbench;

/// The feature's database, laid out afresh, with a catalog over it.
struct OnSqlite<C> {
    fixture: SqliteFixture,
    transactions: SqliteTransactions,
    catalog: C,
}

impl<C> OnSqlite<C> {
    async fn over(catalog: fn(SqlitePool) -> C) -> Self {
        let fixture = SqliteFixture::setup();
        let pool: SqlitePool = fixture.create_database("feedback").await;
        sqlite::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty file");

        Self {
            fixture,
            transactions: SqliteTransactions::new(pool.clone()),
            catalog: catalog(pool),
        }
    }
}

macro_rules! on_sqlite {
    ($module:ident, $catalog:ty, $suite:path) => {
        mod $module {
            use super::*;

            #[async_trait]
            impl Workbench for OnSqlite<$catalog> {
                type Catalog = $catalog;

                async fn setup() -> Self {
                    OnSqlite::over(<$catalog>::new).await
                }

                fn catalog(&self) -> &Self::Catalog {
                    &self.catalog
                }

                fn transactions(&self) -> &dyn Transactions {
                    &self.transactions
                }

                async fn cleanup(self) {
                    self.fixture.cleanup().await;
                }
            }

            $suite!(OnSqlite<$catalog>);
        }
    };
}

on_sqlite!(
    feedback,
    crate::SqliteFeedbackCatalog,
    crate::feedback_suite::feedback_conformance_tests
);
on_sqlite!(
    reading_copies,
    crate::SqliteReadingCopyCatalog,
    crate::reading_copies_suite::reading_copies_conformance_tests
);
on_sqlite!(
    names,
    crate::SqliteAccountNameCatalog,
    crate::names_suite::names_conformance_tests
);
