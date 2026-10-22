use std::error::Error;

use accounts_core::{
    AccountId, Attempt, EmailAddress, EmailAddresses, PasswordHash, Passwords, PendingVerification,
    SecretsError, Session, Sessions, Stage, TokenDigest, VerificationCode, Verifications,
};
use async_trait::async_trait;
use sqlx::migrate::Migrator;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use time::OffsetDateTime;

const CLAIM: &str = "
    INSERT INTO account_email_addresses (email, account)
    VALUES ($1, $2)
    ON CONFLICT (email) DO NOTHING
";

const HOLDER: &str = "SELECT account FROM account_email_addresses WHERE email = $1";

const RELEASE: &str = "DELETE FROM account_email_addresses WHERE email = $1 AND account = $2";

const KEEP: &str = "
    INSERT INTO account_passwords (account, password_hash)
    VALUES ($1, $2)
    ON CONFLICT (account) DO UPDATE
    SET password_hash = EXCLUDED.password_hash
";

const PASSWORD_OF: &str = "SELECT password_hash FROM account_passwords WHERE account = $1";

const ISSUE: &str = "
    INSERT INTO email_verifications (account, code, until, tried)
    VALUES ($1, $2, $3, $4)
    ON CONFLICT (account) DO UPDATE
    SET code   = EXCLUDED.code,
        until  = EXCLUDED.until,
        tried  = EXCLUDED.tried
";

const ATTEMPT: &str = "
    UPDATE email_verifications
    SET tried = tried + 1
    WHERE account = $1 AND tried < $2
    RETURNING code, until, tried
";

const ISSUED: &str = "SELECT count(*) FROM email_verifications WHERE account = $1";

const OPEN: &str = "
    INSERT INTO account_sessions (digest, account, stage, until)
    VALUES ($1, $2, $3, $4)
";

const FIND: &str = "SELECT account, stage, until FROM account_sessions WHERE digest = $1";

const END_SESSION: &str = "DELETE FROM account_sessions WHERE digest = $1";

const SIGNED_IN: &str = "signed-in";
const SECOND_FACTOR_OWED: &str = "second-factor-owed";

const CLOSE: &str = "DELETE FROM email_verifications WHERE account = $1";

pub fn migrations() -> Migrator {
    sqlx::migrate!("./migrations/postgres")
}

pub struct PostgresSecrets {
    pool: PgPool,
}

impl PostgresSecrets {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn unreachable(failure: impl Error + Send + Sync + 'static) -> SecretsError {
    SecretsError::Backend(Box::new(failure))
}

fn written(stage: Stage) -> &'static str {
    match stage {
        Stage::SignedIn => SIGNED_IN,
        Stage::SecondFactorOwed => SECOND_FACTOR_OWED,
    }
}

fn to_session(row: &PgRow) -> Result<Session, SecretsError> {
    let account: String = row.try_get("account").map_err(unreachable)?;
    let stage: String = row.try_get("stage").map_err(unreachable)?;
    let until: OffsetDateTime = row.try_get("until").map_err(unreachable)?;

    Ok(Session {
        account: account.parse().map_err(unreachable)?,
        stage: match stage.as_str() {
            SIGNED_IN => Stage::SignedIn,
            SECOND_FACTOR_OWED => Stage::SecondFactorOwed,
            _ => {
                return Err(SecretsError::Backend(
                    format!("stage holds {stage:?}, which is no stage").into(),
                ));
            }
        },
        until,
    })
}

fn to_pending(row: &PgRow) -> Result<PendingVerification, SecretsError> {
    let code: String = row.try_get("code").map_err(unreachable)?;
    let until: OffsetDateTime = row.try_get("until").map_err(unreachable)?;
    let tried: i32 = row.try_get("tried").map_err(unreachable)?;

    Ok(PendingVerification {
        code: VerificationCode::kept(code),
        until,
        tried: u32::try_from(tried).map_err(unreachable)?,
    })
}

#[async_trait]
impl EmailAddresses for PostgresSecrets {
    async fn claim(&self, email: &EmailAddress, account: &AccountId) -> Result<(), SecretsError> {
        let done = sqlx::query(CLAIM)
            .bind(email.as_str())
            .bind(account.to_string())
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        match done.rows_affected() {
            0 => Err(SecretsError::Taken),
            _ => Ok(()),
        }
    }

    async fn holder(&self, email: &EmailAddress) -> Result<Option<AccountId>, SecretsError> {
        let held: Option<String> = sqlx::query_scalar(HOLDER)
            .bind(email.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(unreachable)?;

        held.map(|account| account.parse().map_err(unreachable))
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
impl Passwords for PostgresSecrets {
    async fn keep(&self, account: &AccountId, password: &PasswordHash) -> Result<(), SecretsError> {
        sqlx::query(KEEP)
            .bind(account.to_string())
            .bind(password.as_str())
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn of(&self, account: &AccountId) -> Result<Option<PasswordHash>, SecretsError> {
        let kept: Option<String> = sqlx::query_scalar(PASSWORD_OF)
            .bind(account.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(kept.map(PasswordHash::kept))
    }
}

#[async_trait]
impl Sessions for PostgresSecrets {
    async fn open(&self, digest: &TokenDigest, session: &Session) -> Result<(), SecretsError> {
        sqlx::query(OPEN)
            .bind(digest.as_str())
            .bind(session.account.to_string())
            .bind(written(session.stage))
            .bind(session.until)
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn find(&self, digest: &TokenDigest) -> Result<Option<Session>, SecretsError> {
        sqlx::query(FIND)
            .bind(digest.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(unreachable)?
            .as_ref()
            .map(to_session)
            .transpose()
    }

    async fn end(&self, digest: &TokenDigest) -> Result<(), SecretsError> {
        sqlx::query(END_SESSION)
            .bind(digest.as_str())
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }
}

#[async_trait]
impl Verifications for PostgresSecrets {
    async fn issue(
        &self,
        account: &AccountId,
        pending: &PendingVerification,
    ) -> Result<(), SecretsError> {
        sqlx::query(ISSUE)
            .bind(account.to_string())
            .bind(pending.code.as_str())
            .bind(pending.until)
            .bind(i32::try_from(pending.tried).map_err(unreachable)?)
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn attempt(&self, account: &AccountId, within: u32) -> Result<Attempt, SecretsError> {
        let counted = sqlx::query(ATTEMPT)
            .bind(account.to_string())
            .bind(i32::try_from(within).map_err(unreachable)?)
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
