use std::error::Error;

use accounts_core::{
    AccountId, Attempt, CodePurpose, Dismissal, EmailAddress, EmailAddresses, InvitationTicket,
    InvitationTickets, Operators, PasswordHash, Passwords, PendingVerification, SecretsError,
    Session, Sessions, Stage, TokenDigest, VerificationCode, Verifications,
};
use async_trait::async_trait;
use clock::text;
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use time::OffsetDateTime;

const KEEP_CODE: &str = "
    INSERT INTO account_operator_code (digest)
    VALUES (?1)
    ON CONFLICT (single) DO UPDATE
    SET digest = EXCLUDED.digest
";

const HOLDS_CODE: &str = "SELECT EXISTS (SELECT 1 FROM account_operator_code WHERE digest = ?1)";

const TAKE_CODE: &str = "DELETE FROM account_operator_code WHERE digest = ?1";

const APPOINT: &str = "
    INSERT INTO account_operators (account)
    VALUES (?1)
    ON CONFLICT (account) DO NOTHING
";

const DISMISS_UNLESS_LAST: &str = "
    DELETE FROM account_operators
    WHERE account = ?1 AND (SELECT count(*) FROM account_operators) > 1
";

const APPOINTED: &str = "SELECT EXISTS (SELECT 1 FROM account_operators WHERE account = ?1)";

const ANY_APPOINTED: &str = "SELECT EXISTS (SELECT 1 FROM account_operators)";

const CLAIM: &str = "
    INSERT INTO account_email_addresses (email, account, claimed_at)
    VALUES (?1, ?2, ?3)
    ON CONFLICT (email) DO NOTHING
";

const TAKE_OVER: &str = "
    UPDATE account_email_addresses
    SET account = ?3, claimed_at = ?5
    WHERE email = ?1 AND account = ?2 AND claimed_at < ?4
";

const HOLDER: &str = "SELECT account FROM account_email_addresses WHERE email = ?1";

const RELEASE: &str = "DELETE FROM account_email_addresses WHERE email = ?1 AND account = ?2";

const KEEP: &str = "
    INSERT INTO account_passwords (account, password_hash)
    VALUES (?1, ?2)
    ON CONFLICT (account) DO UPDATE
    SET password_hash = excluded.password_hash
";

const PASSWORD_OF: &str = "SELECT password_hash FROM account_passwords WHERE account = ?1";

const ISSUE: &str = "
    INSERT INTO account_codes (account, purpose, code, until, tried)
    VALUES (?1, ?2, ?3, ?4, ?5)
    ON CONFLICT (account, purpose) DO UPDATE
    SET code  = excluded.code,
        until = excluded.until,
        tried = excluded.tried
";

const ATTEMPT: &str = "
    UPDATE account_codes
    SET tried = tried + 1
    WHERE account = ?1 AND purpose = ?2 AND tried < ?3
    RETURNING code, until, tried
";

const ISSUED: &str = "SELECT count(*) FROM account_codes WHERE account = ?1 AND purpose = ?2";

const OPEN: &str = "
    INSERT INTO account_sessions (digest, account, stage, begun, until)
    VALUES (?1, ?2, ?3, ?4, ?5)
";

const FIND: &str = "SELECT account, stage, begun, until FROM account_sessions WHERE digest = ?1";

const END_SESSION: &str = "DELETE FROM account_sessions WHERE digest = ?1";

const SIGNED_IN: &str = "signed-in";
const SECOND_FACTOR_OWED: &str = "second-factor-owed";

const CLOSE: &str = "DELETE FROM account_codes WHERE account = ?1 AND purpose = ?2";

const KEEP_TICKET: &str = "
    INSERT INTO account_invitation_tickets (digest, invitation, email, until)
    VALUES (?1, ?2, ?3, ?4)
    ON CONFLICT DO NOTHING
";

const TAKE_TICKET: &str = "
    UPDATE account_invitation_tickets
    SET ended = TRUE
    WHERE digest = ?1 AND NOT ended
    RETURNING invitation, email, until
";

const FORGET_TICKET: &str = "
    INSERT INTO account_invitation_tickets (invitation, ended)
    VALUES (?1, TRUE)
    ON CONFLICT (invitation) DO UPDATE
    SET ended = TRUE
";

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

fn written(stage: Stage) -> &'static str {
    match stage {
        Stage::SignedIn => SIGNED_IN,
        Stage::SecondFactorOwed => SECOND_FACTOR_OWED,
    }
}

fn to_ticket(row: &SqliteRow) -> Result<InvitationTicket, SecretsError> {
    let email: String = row.try_get("email").map_err(unreachable)?;
    let until: String = row.try_get("until").map_err(unreachable)?;
    let until = text::read(&until).ok_or_else(|| {
        SecretsError::Backend(format!("until holds {until:?}, which is no time").into())
    })?;

    Ok(InvitationTicket {
        invitation: row.try_get("invitation").map_err(unreachable)?,
        to: EmailAddress::new(&email).map_err(unreachable)?,
        until,
    })
}

fn to_session(row: &SqliteRow) -> Result<Session, SecretsError> {
    let account: String = row.try_get("account").map_err(unreachable)?;
    let stage: String = row.try_get("stage").map_err(unreachable)?;
    let begun: String = row.try_get("begun").map_err(unreachable)?;
    let begun = text::read(&begun).ok_or_else(|| {
        SecretsError::Backend(format!("begun holds {begun:?}, which is no time").into())
    })?;
    let until: String = row.try_get("until").map_err(unreachable)?;
    let until = text::read(&until).ok_or_else(|| {
        SecretsError::Backend(format!("until holds {until:?}, which is no time").into())
    })?;

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
        begun,
        until,
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
impl EmailAddresses for SqliteSecrets {
    async fn claim(
        &self,
        email: &EmailAddress,
        account: &AccountId,
        at: OffsetDateTime,
    ) -> Result<(), SecretsError> {
        let done = sqlx::query(CLAIM)
            .bind(email.as_str())
            .bind(account.to_string())
            .bind(text::written(at))
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        match done.rows_affected() {
            0 => Err(SecretsError::Taken),
            _ => Ok(()),
        }
    }

    async fn take_over(
        &self,
        email: &EmailAddress,
        from: &AccountId,
        to: &AccountId,
        claimed_before: OffsetDateTime,
        at: OffsetDateTime,
    ) -> Result<bool, SecretsError> {
        let done = sqlx::query(TAKE_OVER)
            .bind(email.as_str())
            .bind(from.to_string())
            .bind(to.to_string())
            .bind(text::written(claimed_before))
            .bind(text::written(at))
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(done.rows_affected() == 1)
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
impl Passwords for SqliteSecrets {
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
impl Sessions for SqliteSecrets {
    async fn open(&self, digest: &TokenDigest, session: &Session) -> Result<(), SecretsError> {
        sqlx::query(OPEN)
            .bind(digest.as_str())
            .bind(session.account.to_string())
            .bind(written(session.stage))
            .bind(text::written(session.begun))
            .bind(text::written(session.until))
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
impl Verifications for SqliteSecrets {
    async fn issue(
        &self,
        account: &AccountId,
        purpose: CodePurpose,
        pending: &PendingVerification,
    ) -> Result<(), SecretsError> {
        sqlx::query(ISSUE)
            .bind(account.to_string())
            .bind(purpose.as_str())
            .bind(pending.code.as_str())
            .bind(text::written(pending.until))
            .bind(i64::from(pending.tried))
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn attempt(
        &self,
        account: &AccountId,
        purpose: CodePurpose,
        within: u32,
    ) -> Result<Attempt, SecretsError> {
        let counted = sqlx::query(ATTEMPT)
            .bind(account.to_string())
            .bind(purpose.as_str())
            .bind(i64::from(within))
            .fetch_optional(&self.pool)
            .await
            .map_err(unreachable)?;
        if let Some(counted) = counted {
            return Ok(Attempt::Counted(to_pending(&counted)?));
        }

        let issued: i64 = sqlx::query_scalar(ISSUED)
            .bind(account.to_string())
            .bind(purpose.as_str())
            .fetch_one(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(match issued {
            0 => Attempt::Nothing,
            _ => Attempt::Exhausted,
        })
    }

    async fn close(&self, account: &AccountId, purpose: CodePurpose) -> Result<(), SecretsError> {
        sqlx::query(CLOSE)
            .bind(account.to_string())
            .bind(purpose.as_str())
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }
}

#[async_trait]
impl Operators for SqliteSecrets {
    async fn keep_code(&self, digest: &TokenDigest) -> Result<(), SecretsError> {
        sqlx::query(KEEP_CODE)
            .bind(digest.as_str())
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn holds_code(&self, digest: &TokenDigest) -> Result<bool, SecretsError> {
        sqlx::query_scalar(HOLDS_CODE)
            .bind(digest.as_str())
            .fetch_one(&self.pool)
            .await
            .map_err(unreachable)
    }

    async fn take_code(&self, digest: &TokenDigest) -> Result<bool, SecretsError> {
        let done = sqlx::query(TAKE_CODE)
            .bind(digest.as_str())
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(done.rows_affected() == 1)
    }

    async fn appoint(&self, account: &AccountId) -> Result<(), SecretsError> {
        sqlx::query(APPOINT)
            .bind(account.to_string())
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn dismiss(&self, account: &AccountId) -> Result<Dismissal, SecretsError> {
        let done = sqlx::query(DISMISS_UNLESS_LAST)
            .bind(account.to_string())
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;
        if done.rows_affected() == 1 {
            return Ok(Dismissal::Dismissed);
        }
        let appointed: bool = sqlx::query_scalar(APPOINTED)
            .bind(account.to_string())
            .fetch_one(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(if appointed {
            Dismissal::LastOne
        } else {
            Dismissal::NotAppointed
        })
    }

    async fn any_appointed(&self) -> Result<bool, SecretsError> {
        sqlx::query_scalar(ANY_APPOINTED)
            .fetch_one(&self.pool)
            .await
            .map_err(unreachable)
    }
}

#[async_trait]
impl InvitationTickets for SqliteSecrets {
    async fn keep_ticket(
        &self,
        digest: &TokenDigest,
        ticket: &InvitationTicket,
    ) -> Result<bool, SecretsError> {
        let done = sqlx::query(KEEP_TICKET)
            .bind(digest.as_str())
            .bind(&ticket.invitation)
            .bind(ticket.to.as_str())
            .bind(text::written(ticket.until))
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(done.rows_affected() == 1)
    }

    async fn take_ticket(
        &self,
        digest: &TokenDigest,
    ) -> Result<Option<InvitationTicket>, SecretsError> {
        sqlx::query(TAKE_TICKET)
            .bind(digest.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(unreachable)?
            .as_ref()
            .map(to_ticket)
            .transpose()
    }

    async fn forget_ticket(&self, invitation: &str) -> Result<(), SecretsError> {
        sqlx::query(FORGET_TICKET)
            .bind(invitation)
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }
}
