use std::error::Error;

use accounts_core::{
    AccountId, Attempt, Credentials, EmailAddress, PasswordHash, PendingVerification, Registered,
    SecretsError, VerificationCode, Verifications,
};
use async_trait::async_trait;
use clock::text;
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

const REGISTER: &str = "
    INSERT INTO account_credentials (email, account, password_hash)
    VALUES (?1, ?2, ?3)
    ON CONFLICT (email) DO NOTHING
";

const REGISTERED: &str = "SELECT account, password_hash FROM account_credentials WHERE email = ?1";

const RELEASE: &str = "DELETE FROM account_credentials WHERE email = ?1 AND account = ?2";

const ISSUE: &str = "
    INSERT INTO email_verifications (account, code, until, tried)
    VALUES (?1, ?2, ?3, ?4)
    ON CONFLICT (account) DO UPDATE
    SET code  = excluded.code,
        until = excluded.until,
        tried = excluded.tried
";

const ATTEMPT: &str = "
    UPDATE email_verifications
    SET tried = tried + 1
    WHERE account = ?1 AND tried < ?2
    RETURNING code, until, tried
";

const ISSUED: &str = "SELECT count(*) FROM email_verifications WHERE account = ?1";

const CLOSE: &str = "DELETE FROM email_verifications WHERE account = ?1";

pub fn migrations() -> Migrator {
    sqlx::migrate!("./migrations/sqlite")
}

pub struct SqliteSecrets {
    pool: SqlitePool,
}

impl SqliteSecrets {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn unreachable(failure: impl Error + Send + Sync + 'static) -> SecretsError {
    SecretsError::Backend(Box::new(failure))
}

fn to_registered(row: &SqliteRow) -> Result<Registered, SecretsError> {
    let account: String = row.try_get("account").map_err(unreachable)?;
    let password: String = row.try_get("password_hash").map_err(unreachable)?;

    Ok(Registered {
        account: account.parse().map_err(unreachable)?,
        password: PasswordHash::kept(password),
    })
}

fn to_pending(row: &SqliteRow) -> Result<PendingVerification, SecretsError> {
    let code: String = row.try_get("code").map_err(unreachable)?;
    let until: String = row.try_get("until").map_err(unreachable)?;
    let tried: i64 = row.try_get("tried").map_err(unreachable)?;

    Ok(PendingVerification {
        code: VerificationCode::kept(code),
        until: text::read(&until).ok_or_else(|| {
            SecretsError::Backend(format!("until holds {until:?}, which is no time").into())
        })?,
        tried: u32::try_from(tried).map_err(unreachable)?,
    })
}

#[async_trait]
impl Credentials for SqliteSecrets {
    async fn register(
        &self,
        email: &EmailAddress,
        account: &AccountId,
        password: &PasswordHash,
    ) -> Result<(), SecretsError> {
        let done = sqlx::query(REGISTER)
            .bind(email.as_str())
            .bind(account.to_string())
            .bind(password.as_str())
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        match done.rows_affected() {
            0 => Err(SecretsError::Taken),
            _ => Ok(()),
        }
    }

    async fn registered(&self, email: &EmailAddress) -> Result<Option<Registered>, SecretsError> {
        sqlx::query(REGISTERED)
            .bind(email.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(unreachable)?
            .as_ref()
            .map(to_registered)
            .transpose()
    }

    async fn release(&self, email: &EmailAddress, account: &AccountId) -> Result<(), SecretsError> {
        sqlx::query(RELEASE)
            .bind(email.as_str())
            .bind(account.to_string())
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }
}

#[async_trait]
impl Verifications for SqliteSecrets {
    async fn issue(
        &self,
        account: &AccountId,
        pending: &PendingVerification,
    ) -> Result<(), SecretsError> {
        sqlx::query(ISSUE)
            .bind(account.to_string())
            .bind(pending.code.as_str())
            .bind(text::written(pending.until))
            .bind(i64::from(pending.tried))
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn attempt(&self, account: &AccountId, within: u32) -> Result<Attempt, SecretsError> {
        let counted = sqlx::query(ATTEMPT)
            .bind(account.to_string())
            .bind(i64::from(within))
            .fetch_optional(&self.pool)
            .await
            .map_err(unreachable)?;
        if let Some(counted) = counted {
            return Ok(Attempt::Counted(to_pending(&counted)?));
        }

        let issued: i64 = sqlx::query_scalar(ISSUED)
            .bind(account.to_string())
            .fetch_one(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(match issued {
            0 => Attempt::Nothing,
            _ => Attempt::Exhausted,
        })
    }

    async fn close(&self, account: &AccountId) -> Result<(), SecretsError> {
        sqlx::query(CLOSE)
            .bind(account.to_string())
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }
}
