use assets_core::{CatalogError, Membership, MembershipCatalog, ProjectLink};
use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use time::OffsetDateTime;
use transacting::Transaction;
use transacting::postgres::connection;

use crate::written::unreachable;

const TOLD: &str = "
    INSERT INTO asset_memberships (project, account, version, gone_at)
    VALUES ($1, $2, $3, $4)
    ON CONFLICT (project, account) DO UPDATE
    SET version = excluded.version, gone_at = excluded.gone_at
    WHERE asset_memberships.version < excluded.version
";

const DISBANDED: &str = "
    INSERT INTO asset_disbanded_projects (project, version, laid_at)
    VALUES ($1, $2, $3)
    ON CONFLICT (project) DO NOTHING
";

const SHARED: &str = "
    SELECT EXISTS (
        SELECT 1 FROM asset_memberships one
        JOIN asset_memberships other ON other.project = one.project
        WHERE one.account = $1 AND other.account = $2
          AND one.gone_at IS NULL AND other.gone_at IS NULL
          AND NOT EXISTS (
              SELECT 1 FROM asset_disbanded_projects disbanded
              WHERE disbanded.project = one.project
          )
    )
";

const OLD_DISBANDED: &str = "
    SELECT project FROM asset_disbanded_projects
    WHERE laid_at < $1
    ORDER BY laid_at, project
    LIMIT $2
";

const FORGET_MEMBERSHIPS_OF_OLD_DISBANDED: &str = "
    DELETE FROM asset_memberships WHERE project IN (
        SELECT project FROM asset_disbanded_projects
        WHERE laid_at < $1
        ORDER BY laid_at, project
        LIMIT $2
    )
";

const FORGET_OLD_DISBANDED: &str = "
    DELETE FROM asset_disbanded_projects WHERE project IN (
        SELECT project FROM asset_disbanded_projects
        WHERE laid_at < $1
        ORDER BY laid_at, project
        LIMIT $2
    )
";

const FORGET_OLD_GONE: &str = "
    DELETE FROM asset_memberships WHERE (project, account) IN (
        SELECT project, account FROM asset_memberships
        WHERE gone_at < $1
        ORDER BY gone_at, project, account
        LIMIT $2
    )
";

/// Who is a member of which project on Postgres, beside the assets in the catalog's own schema, each membership with the version it was last told at and when it ended, and each project disbanded with when.
#[derive(Debug, Default)]
pub struct PostgresMembershipCatalog;

impl PostgresMembershipCatalog {
    pub fn new() -> Self {
        Self
    }
}

fn version_written(version: Version) -> Result<i64, CatalogError> {
    i64::try_from(version.count()).map_err(unreachable)
}

async fn told(
    transaction: &mut Transaction,
    membership: &Membership,
    version: Version,
    gone_at: Option<OffsetDateTime>,
) -> Result<(), CatalogError> {
    sqlx::query(TOLD)
        .bind(membership.project.to_string())
        .bind(membership.account.to_string())
        .bind(version_written(version)?)
        .bind(gone_at)
        .execute(connection(transaction).map_err(unreachable)?)
        .await
        .map_err(unreachable)?;

    Ok(())
}

#[async_trait]
impl MembershipCatalog for PostgresMembershipCatalog {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        membership: &Membership,
        version: Version,
    ) -> Result<(), CatalogError> {
        told(transaction, membership, version, None).await
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        membership: &Membership,
        version: Version,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        told(transaction, membership, version, Some(at)).await
    }

    async fn forget_project(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        version: Version,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        sqlx::query(DISBANDED)
            .bind(project.to_string())
            .bind(version_written(version)?)
            .bind(at)
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn share_a_project(
        &self,
        transaction: &mut Transaction,
        one: &AccountId,
        other: &AccountId,
    ) -> Result<bool, CatalogError> {
        sqlx::query_scalar(SHARED)
            .bind(one.to_string())
            .bind(other.to_string())
            .fetch_one(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)
    }

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        let at_most = i64::try_from(at_most).map_err(unreachable)?;
        let disbanded = sqlx::query_scalar::<_, String>(OLD_DISBANDED)
            .bind(before)
            .bind(at_most)
            .fetch_all(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?
            .len();
        for forgetting in [FORGET_MEMBERSHIPS_OF_OLD_DISBANDED, FORGET_OLD_DISBANDED] {
            sqlx::query(forgetting)
                .bind(before)
                .bind(at_most)
                .execute(connection(transaction).map_err(unreachable)?)
                .await
                .map_err(unreachable)?;
        }
        let gone = sqlx::query(FORGET_OLD_GONE)
            .bind(before)
            .bind(at_most)
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?
            .rows_affected();

        Ok(u64::try_from(disbanded).map_err(unreachable)? + gone)
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use sqlx::PgPool;
    use test_harness::PostgresFixture;
    use transacting::{PostgresTransactions, Transactions};

    use super::*;
    use crate::memberships_suite::Workbench;
    use crate::postgres;

    struct OnPostgres {
        fixture: PostgresFixture,
        store: PostgresMembershipCatalog,
        transactions: PostgresTransactions,
    }

    #[async_trait]
    impl Workbench for OnPostgres {
        type Store = PostgresMembershipCatalog;

        async fn setup() -> Self {
            let fixture = PostgresFixture::setup().await;
            let pool: PgPool = fixture.create_schema("assets").await;
            postgres::migrations()
                .run(&pool)
                .await
                .expect("the schema should lay down in an empty namespace");

            Self {
                fixture,
                store: PostgresMembershipCatalog::new(),
                transactions: PostgresTransactions::new(pool),
            }
        }

        fn store(&self) -> &Self::Store {
            &self.store
        }

        fn transactions(&self) -> &dyn Transactions {
            &self.transactions
        }

        async fn cleanup(self) {
            self.fixture.cleanup().await;
        }
    }

    crate::membership_conformance_tests!(OnPostgres);
}
