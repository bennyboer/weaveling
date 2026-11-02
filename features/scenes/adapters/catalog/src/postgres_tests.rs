use std::sync::Arc;

use async_trait::async_trait;
use scenes_core::{CatalogError, ProjectLink, SceneCatalog, SceneId};
use sqlx::PgPool;
use test_harness::PostgresFixture;
use time::{Duration, OffsetDateTime};
use transacting::{PostgresTransactions, Transactions};

use crate::postgres;
use crate::postgres::{PostgresQuotationRestylingTasks, PostgresSceneCatalog};
use crate::suite::Workbench;

struct OnPostgres {
    fixture: PostgresFixture,
    pool: PgPool,
    transactions: PostgresTransactions,
    catalog: PostgresSceneCatalog,
    tasks: PostgresQuotationRestylingTasks,
}

#[async_trait]
impl Workbench for OnPostgres {
    type Catalog = PostgresSceneCatalog;
    type Tasks = PostgresQuotationRestylingTasks;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool: PgPool = fixture.create_schema("scenes").await;
        postgres::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty namespace");

        Self {
            fixture,
            transactions: PostgresTransactions::new(pool.clone()),
            catalog: PostgresSceneCatalog::new(pool.clone()),
            tasks: PostgresQuotationRestylingTasks::new(pool.clone()),
            pool,
        }
    }

    fn catalog(&self) -> &Self::Catalog {
        &self.catalog
    }

    fn tasks(&self) -> &Self::Tasks {
        &self.tasks
    }

    fn transactions(&self) -> &dyn Transactions {
        &self.transactions
    }

    async fn cleanup(self) {
        self.fixture.cleanup().await;
    }
}

crate::suite::conformance_tests!(OnPostgres);

#[tokio::test(flavor = "multi_thread")]
async fn remembering_and_forgetting_at_once_never_leaves_a_forgotten_scene_standing() {
    let bench = OnPostgres::setup().await;
    let catalog = Arc::new(PostgresSceneCatalog::new(bench.pool.clone()));
    let transactions = Arc::new(PostgresTransactions::new(bench.pool.clone()));
    let project = ProjectLink::from("project_1");

    for nth in 0..40 {
        let id = SceneId::generate(OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_000 + nth));
        let (remembering, forgetting) = (catalog.clone(), catalog.clone());
        let (remembered_in, forgotten_in) = (transactions.clone(), transactions.clone());
        let in_project = project.clone();
        let (remembered, forgotten) = tokio::join!(
            tokio::spawn(async move {
                let mut transaction = remembered_in
                    .begin()
                    .await
                    .expect("beginning should succeed");
                remembering
                    .remember(&mut transaction, id, &in_project)
                    .await?;
                transaction
                    .finish()
                    .await
                    .expect("finishing should succeed");
                Ok::<_, CatalogError>(())
            }),
            tokio::spawn(async move {
                let mut transaction = forgotten_in
                    .begin()
                    .await
                    .expect("beginning should succeed");
                forgetting
                    .forget(
                        &mut transaction,
                        id,
                        OffsetDateTime::UNIX_EPOCH + Duration::seconds(5_000),
                    )
                    .await?;
                transaction
                    .finish()
                    .await
                    .expect("finishing should succeed");
                Ok::<_, CatalogError>(())
            }),
        );
        remembered
            .expect("remembering should not panic")
            .expect("remembering should succeed");
        forgotten
            .expect("forgetting should not panic")
            .expect("forgetting should succeed");
    }

    assert!(
        catalog
            .in_project_after(&project, None, 100)
            .await
            .expect("listing should succeed")
            .is_empty(),
        "a remember racing a forget must never bring the forgotten scene back"
    );
    bench.cleanup().await;
}
