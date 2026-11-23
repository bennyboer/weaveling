use async_trait::async_trait;
use sqlx::PgPool;
use test_harness::PostgresFixture;
use transacting::{PostgresTransactions, Transactions};

use crate::postgres;
use crate::suite::Workbench;

/// The feature's database, laid out afresh, with a catalog over it.
struct OnPostgres<C> {
    fixture: PostgresFixture,
    transactions: PostgresTransactions,
    catalog: C,
}

impl<C> OnPostgres<C> {
    async fn over(catalog: fn(PgPool) -> C) -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool: PgPool = fixture.create_schema("feedback").await;
        postgres::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty namespace");

        Self {
            fixture,
            transactions: PostgresTransactions::new(pool.clone()),
            catalog: catalog(pool),
        }
    }
}

macro_rules! on_postgres {
    ($module:ident, $catalog:ty, $suite:path) => {
        mod $module {
            use super::*;

            #[async_trait]
            impl Workbench for OnPostgres<$catalog> {
                type Catalog = $catalog;

                async fn setup() -> Self {
                    OnPostgres::over(<$catalog>::new).await
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

            $suite!(OnPostgres<$catalog>);
        }
    };
}

on_postgres!(
    feedback,
    crate::PostgresFeedbackCatalog,
    crate::feedback_suite::feedback_conformance_tests
);
on_postgres!(
    reading_copies,
    crate::PostgresReadingCopyCatalog,
    crate::reading_copies_suite::reading_copies_conformance_tests
);
on_postgres!(
    names,
    crate::PostgresAccountNameCatalog,
    crate::names_suite::names_conformance_tests
);
