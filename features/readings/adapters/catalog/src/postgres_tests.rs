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
        let pool: PgPool = fixture.create_schema("readings").await;
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

    fn transactions(&self) -> &dyn Transactions {
        &self.transactions
    }

    async fn cleanup(self) {
        self.fixture.cleanup().await;
    }
}

mod texts {
    use super::*;
    use crate::postgres::PostgresSceneTextCatalog;

    #[async_trait]
    impl Workbench for OnPostgres<PostgresSceneTextCatalog> {
        type Catalog = PostgresSceneTextCatalog;

        async fn setup() -> Self {
            OnPostgres::over(PostgresSceneTextCatalog::new).await
        }

        fn catalog(&self) -> &Self::Catalog {
            &self.catalog
        }

        fn transactions(&self) -> &dyn Transactions {
            OnPostgres::transactions(self)
        }

        async fn cleanup(self) {
            OnPostgres::cleanup(self).await;
        }
    }

    crate::suite::conformance_tests!(OnPostgres<PostgresSceneTextCatalog>);
}

mod outlines {
    use super::*;
    use crate::outline_postgres::PostgresOutlineCatalog;

    #[async_trait]
    impl Workbench for OnPostgres<PostgresOutlineCatalog> {
        type Catalog = PostgresOutlineCatalog;

        async fn setup() -> Self {
            OnPostgres::over(PostgresOutlineCatalog::new).await
        }

        fn catalog(&self) -> &Self::Catalog {
            &self.catalog
        }

        fn transactions(&self) -> &dyn Transactions {
            OnPostgres::transactions(self)
        }

        async fn cleanup(self) {
            OnPostgres::cleanup(self).await;
        }
    }

    crate::outline_suite::outline_conformance_tests!(OnPostgres<PostgresOutlineCatalog>);
}

mod copies {
    use super::*;
    use crate::copies_postgres::PostgresReadingCopyCatalog;

    #[async_trait]
    impl Workbench for OnPostgres<PostgresReadingCopyCatalog> {
        type Catalog = PostgresReadingCopyCatalog;

        async fn setup() -> Self {
            OnPostgres::over(PostgresReadingCopyCatalog::new).await
        }

        fn catalog(&self) -> &Self::Catalog {
            &self.catalog
        }

        fn transactions(&self) -> &dyn Transactions {
            OnPostgres::transactions(self)
        }

        async fn cleanup(self) {
            OnPostgres::cleanup(self).await;
        }
    }

    crate::copies_suite::copies_conformance_tests!(OnPostgres<PostgresReadingCopyCatalog>);
}
