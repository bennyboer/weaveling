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
        let pool: SqlitePool = fixture.create_database("readings").await;
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

    fn transactions(&self) -> &dyn Transactions {
        &self.transactions
    }

    async fn cleanup(self) {
        self.fixture.cleanup().await;
    }
}

mod texts {
    use super::*;
    use crate::sqlite::SqliteSceneTextCatalog;

    #[async_trait]
    impl Workbench for OnSqlite<SqliteSceneTextCatalog> {
        type Catalog = SqliteSceneTextCatalog;

        async fn setup() -> Self {
            OnSqlite::over(SqliteSceneTextCatalog::new).await
        }

        fn catalog(&self) -> &Self::Catalog {
            &self.catalog
        }

        fn transactions(&self) -> &dyn Transactions {
            OnSqlite::transactions(self)
        }

        async fn cleanup(self) {
            OnSqlite::cleanup(self).await;
        }
    }

    crate::suite::conformance_tests!(OnSqlite<SqliteSceneTextCatalog>);
}

mod outlines {
    use super::*;
    use crate::outline_sqlite::SqliteOutlineCatalog;

    #[async_trait]
    impl Workbench for OnSqlite<SqliteOutlineCatalog> {
        type Catalog = SqliteOutlineCatalog;

        async fn setup() -> Self {
            OnSqlite::over(SqliteOutlineCatalog::new).await
        }

        fn catalog(&self) -> &Self::Catalog {
            &self.catalog
        }

        fn transactions(&self) -> &dyn Transactions {
            OnSqlite::transactions(self)
        }

        async fn cleanup(self) {
            OnSqlite::cleanup(self).await;
        }
    }

    crate::outline_suite::outline_conformance_tests!(OnSqlite<SqliteOutlineCatalog>);
}

mod copies {
    use super::*;
    use crate::copies_sqlite::SqliteReadingCopyCatalog;

    #[async_trait]
    impl Workbench for OnSqlite<SqliteReadingCopyCatalog> {
        type Catalog = SqliteReadingCopyCatalog;

        async fn setup() -> Self {
            OnSqlite::over(SqliteReadingCopyCatalog::new).await
        }

        fn catalog(&self) -> &Self::Catalog {
            &self.catalog
        }

        fn transactions(&self) -> &dyn Transactions {
            OnSqlite::transactions(self)
        }

        async fn cleanup(self) {
            OnSqlite::cleanup(self).await;
        }
    }

    crate::copies_suite::copies_conformance_tests!(OnSqlite<SqliteReadingCopyCatalog>);
}
