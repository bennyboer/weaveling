use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use time::OffsetDateTime;
use transacting::Transaction;
use transacting::sqlite::connection;

use crate::{AccessCatalog, AccessError, Role};

const ROLE: &str = "
    INSERT INTO access_roles (project, account, version, role, gone_at)
    VALUES (?1, ?2, ?3, ?4, ?5)
    ON CONFLICT (project, account) DO UPDATE
    SET version = excluded.version, role = excluded.role, gone_at = excluded.gone_at
    WHERE access_roles.version < excluded.version
";

const DISBANDED: &str = "
    INSERT INTO access_disbanded (project, version, laid_at) VALUES (?1, ?2, ?3)
    ON CONFLICT (project) DO NOTHING
";

const TRASH: &str = "
    INSERT INTO access_trash (project, version, in_the_trash) VALUES (?1, ?2, ?3)
    ON CONFLICT (project) DO UPDATE
    SET version = excluded.version, in_the_trash = excluded.in_the_trash
    WHERE access_trash.version < excluded.version
";

const OPERATOR: &str = "
    INSERT INTO access_operators (account, version, operator, gone_at)
    VALUES (?1, ?2, ?3, ?4)
    ON CONFLICT (account) DO UPDATE
    SET version = excluded.version, operator = excluded.operator, gone_at = excluded.gone_at
    WHERE access_operators.version < excluded.version
";

const ROLE_OF: &str = "
    SELECT role FROM access_roles
    WHERE project = ?1 AND account = ?2 AND role IS NOT NULL
      AND NOT EXISTS (SELECT 1 FROM access_disbanded WHERE access_disbanded.project = access_roles.project)
";

const IN_THE_TRASH: &str = "SELECT in_the_trash FROM access_trash WHERE project = ?1";

const PROJECTS_OF: &str = "
    SELECT project FROM access_roles
    WHERE account = ?1 AND role IS NOT NULL
      AND NOT EXISTS (SELECT 1 FROM access_disbanded WHERE access_disbanded.project = access_roles.project)
      AND NOT EXISTS (
          SELECT 1 FROM access_trash
          WHERE access_trash.project = access_roles.project AND access_trash.in_the_trash
      )
    ORDER BY project
";

const SHARED: &str = "
    SELECT EXISTS (
        SELECT 1 FROM access_roles one
        JOIN access_roles other ON other.project = one.project
        WHERE one.account = ?1 AND other.account = ?2
          AND one.role IS NOT NULL AND other.role IS NOT NULL
          AND NOT EXISTS (SELECT 1 FROM access_disbanded WHERE access_disbanded.project = one.project)
          AND NOT EXISTS (
              SELECT 1 FROM access_trash
              WHERE access_trash.project = one.project AND access_trash.in_the_trash
          )
    )
";

const OPERATES: &str = "SELECT operator FROM access_operators WHERE account = ?1";

const OWNERS_OF: &str = "
    SELECT account FROM access_roles
    WHERE project = ?1 AND role = 'owner'
      AND NOT EXISTS (SELECT 1 FROM access_disbanded WHERE access_disbanded.project = access_roles.project)
    ORDER BY account
";

const OLD_DISBANDED: &str = "
    SELECT project FROM access_disbanded
    WHERE laid_at < ?1
    ORDER BY laid_at, project
    LIMIT ?2
";

const FORGET_ROLES_OF_OLD_DISBANDED: &str = "
    DELETE FROM access_roles WHERE project IN (
        SELECT project FROM access_disbanded WHERE laid_at < ?1 ORDER BY laid_at, project LIMIT ?2
    )
";

const FORGET_TRASH_OF_OLD_DISBANDED: &str = "
    DELETE FROM access_trash WHERE project IN (
        SELECT project FROM access_disbanded WHERE laid_at < ?1 ORDER BY laid_at, project LIMIT ?2
    )
";

const FORGET_OLD_DISBANDED: &str = "
    DELETE FROM access_disbanded WHERE project IN (
        SELECT project FROM access_disbanded WHERE laid_at < ?1 ORDER BY laid_at, project LIMIT ?2
    )
";

const FORGET_OLD_GONE: &str = "
    DELETE FROM access_roles WHERE (project, account) IN (
        SELECT project, account FROM access_roles
        WHERE gone_at < ?1
        ORDER BY gone_at, project, account
        LIMIT ?2
    )
";

const FORGET_OLD_STOPPED: &str = "
    DELETE FROM access_operators WHERE account IN (
        SELECT account FROM access_operators
        WHERE gone_at < ?1
        ORDER BY gone_at, account
        LIMIT ?2
    )
";

const LEDGER: &str = "_sqlx_migrations_access";

const OWNER: &str = "owner";
const AUTHOR: &str = "author";
const READER: &str = "reader";

pub fn migrations() -> Migrator {
    let mut laying = sqlx::migrate!("./migrations/sqlite");
    laying.dangerous_set_table_name(LEDGER);

    laying
}

/// Who may do what on Sqlite, laid down in the feature's own database: written in the listener's transaction, read on the feature's own pool.
pub struct SqliteAccess {
    pool: SqlitePool,
}

impl SqliteAccess {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn unreachable(failure: impl std::error::Error + Send + Sync + 'static) -> AccessError {
    AccessError::Backend(Box::new(failure))
}

fn version_written(version: Version) -> Result<i64, AccessError> {
    i64::try_from(version.count()).map_err(unreachable)
}

fn role_written(role: Role) -> &'static str {
    match role {
        Role::Owner => OWNER,
        Role::Author => AUTHOR,
        Role::Reader => READER,
    }
}

fn role_read(written: &str) -> Result<Role, AccessError> {
    match written {
        OWNER => Ok(Role::Owner),
        AUTHOR => Ok(Role::Author),
        READER => Ok(Role::Reader),
        other => Err(AccessError::Backend(
            format!("access_roles holds {other:?}, which is no role").into(),
        )),
    }
}

#[async_trait]
impl AccessCatalog for SqliteAccess {
    async fn remember_role(
        &self,
        transaction: &mut Transaction,
        project: &str,
        account: &AccountId,
        version: Version,
        role: Option<Role>,
        at: OffsetDateTime,
    ) -> Result<(), AccessError> {
        sqlx::query(ROLE)
            .bind(project)
            .bind(account.to_string())
            .bind(version_written(version)?)
            .bind(role.map(role_written))
            .bind(role.is_none().then_some(at))
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn remember_disbanded(
        &self,
        transaction: &mut Transaction,
        project: &str,
        version: Version,
        at: OffsetDateTime,
    ) -> Result<(), AccessError> {
        sqlx::query(DISBANDED)
            .bind(project)
            .bind(version_written(version)?)
            .bind(at)
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn remember_trash(
        &self,
        transaction: &mut Transaction,
        project: &str,
        version: Version,
        in_the_trash: bool,
    ) -> Result<(), AccessError> {
        sqlx::query(TRASH)
            .bind(project)
            .bind(version_written(version)?)
            .bind(in_the_trash)
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn remember_operator(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        version: Version,
        operator: bool,
        at: OffsetDateTime,
    ) -> Result<(), AccessError> {
        sqlx::query(OPERATOR)
            .bind(account.to_string())
            .bind(version_written(version)?)
            .bind(operator)
            .bind((!operator).then_some(at))
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, AccessError> {
        let at_most = i64::try_from(at_most).map_err(unreachable)?;
        let disbanded = sqlx::query_scalar::<_, String>(OLD_DISBANDED)
            .bind(before)
            .bind(at_most)
            .fetch_all(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?
            .len();
        for forgetting in [
            FORGET_ROLES_OF_OLD_DISBANDED,
            FORGET_TRASH_OF_OLD_DISBANDED,
            FORGET_OLD_DISBANDED,
        ] {
            sqlx::query(forgetting)
                .bind(before)
                .bind(at_most)
                .execute(connection(transaction).map_err(unreachable)?)
                .await
                .map_err(unreachable)?;
        }
        let mut ended = u64::try_from(disbanded).map_err(unreachable)?;
        for forgetting in [FORGET_OLD_GONE, FORGET_OLD_STOPPED] {
            ended += sqlx::query(forgetting)
                .bind(before)
                .bind(at_most)
                .execute(connection(transaction).map_err(unreachable)?)
                .await
                .map_err(unreachable)?
                .rows_affected();
        }

        Ok(ended)
    }

    async fn role_of(
        &self,
        project: &str,
        account: &AccountId,
    ) -> Result<Option<Role>, AccessError> {
        sqlx::query_scalar::<_, String>(ROLE_OF)
            .bind(project)
            .bind(account.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(unreachable)?
            .map(|written| role_read(&written))
            .transpose()
    }

    async fn is_in_the_trash(&self, project: &str) -> Result<bool, AccessError> {
        Ok(sqlx::query_scalar::<_, bool>(IN_THE_TRASH)
            .bind(project)
            .fetch_optional(&self.pool)
            .await
            .map_err(unreachable)?
            .unwrap_or(false))
    }

    async fn projects_of(&self, account: &AccountId) -> Result<Vec<String>, AccessError> {
        sqlx::query_scalar(PROJECTS_OF)
            .bind(account.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)
    }

    async fn share_a_project(
        &self,
        one: &AccountId,
        other: &AccountId,
    ) -> Result<bool, AccessError> {
        sqlx::query_scalar(SHARED)
            .bind(one.to_string())
            .bind(other.to_string())
            .fetch_one(&self.pool)
            .await
            .map_err(unreachable)
    }

    async fn operates(&self, account: &AccountId) -> Result<bool, AccessError> {
        Ok(sqlx::query_scalar::<_, bool>(OPERATES)
            .bind(account.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(unreachable)?
            .unwrap_or(false))
    }

    async fn owners_of(&self, project: &str) -> Result<Vec<AccountId>, AccessError> {
        Ok(sqlx::query_scalar::<_, String>(OWNERS_OF)
            .bind(project)
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?
            .into_iter()
            .map(AccountId::from)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use test_harness::SqliteFixture;
    use transacting::{SqliteTransactions, Transactions};

    use super::*;
    use crate::suite::Workbench;

    struct OnSqlite {
        fixture: SqliteFixture,
        store: SqliteAccess,
        transactions: SqliteTransactions,
    }

    #[async_trait]
    impl Workbench for OnSqlite {
        type Store = SqliteAccess;

        async fn setup() -> Self {
            let fixture = SqliteFixture::setup();
            let pool: SqlitePool = fixture.create_database("access").await;
            migrations()
                .run(&pool)
                .await
                .expect("the schema should lay down in an empty file");

            Self {
                fixture,
                store: SqliteAccess::new(pool.clone()),
                transactions: SqliteTransactions::new(pool),
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

    crate::conformance_tests!(OnSqlite);
}
