use std::error::Error;
use transacting::Transaction;

use accounts_core::{
    AccountId, Attempt, CodePurpose, Dismissal, EmailAddress, EmailAddresses, InvitationScope,
    InvitationTicket, InvitationTickets, KeptSecondFactor, Operators, PasswordHash, Passwords,
    PendingVerification, RecoveryCodeHash, SealedTotpSecret, SecondFactors, SecretsError, Session,
    Sessions, Stage, TokenDigest, TotpStep, VerificationCode, Verifications,
};
use async_trait::async_trait;
use sqlx::migrate::Migrator;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use time::OffsetDateTime;

const KEEP_CODE: &str = "
    INSERT INTO account_operator_code (digest)
    SELECT $1
    WHERE NOT EXISTS (SELECT 1 FROM account_operators)
    ON CONFLICT (single) DO UPDATE
    SET digest = EXCLUDED.digest
";

const HOLDS_CODE: &str = "SELECT EXISTS (SELECT 1 FROM account_operator_code WHERE digest = $1)";

const TAKE_CODE: &str = "
    DELETE FROM account_operator_code
    WHERE digest = $1 AND NOT EXISTS (SELECT 1 FROM account_operators)
";

const APPOINT: &str = "
    INSERT INTO account_operators (account)
    VALUES ($1)
    ON CONFLICT (account) DO NOTHING
";

const EVERY_OPERATOR: &str = "SELECT account FROM account_operators";

const DISMISS: &str = "DELETE FROM account_operators WHERE account = $1";

const ANY_APPOINTED: &str = "SELECT EXISTS (SELECT 1 FROM account_operators)";

const APPOINTED: &str = "SELECT EXISTS (SELECT 1 FROM account_operators WHERE account = $1)";

const CLAIM: &str = "
    INSERT INTO account_email_addresses (email, account, claimed_at)
    VALUES ($1, $2, $3)
    ON CONFLICT (email) DO NOTHING
";

const TAKE_OVER: &str = "
    UPDATE account_email_addresses
    SET account = $3, claimed_at = $5
    WHERE email = $1 AND account = $2 AND claimed_at < $4
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
    INSERT INTO account_codes (account, purpose, code, until, tried)
    VALUES ($1, $2, $3, $4, $5)
    ON CONFLICT (account, purpose) DO UPDATE
    SET code  = EXCLUDED.code,
        until = EXCLUDED.until,
        tried = EXCLUDED.tried
";

const ATTEMPT: &str = "
    UPDATE account_codes
    SET tried = tried + 1
    WHERE account = $1 AND purpose = $2 AND tried < $3
    RETURNING code, until, tried
";

const ISSUED: &str = "SELECT count(*) FROM account_codes WHERE account = $1 AND purpose = $2";

const OPEN: &str = "
    INSERT INTO account_sessions (digest, account, stage, begun, until)
    VALUES ($1, $2, $3, $4, $5)
";

const FIND: &str = "SELECT account, stage, begun, until FROM account_sessions WHERE digest = $1";

const END_SESSION: &str = "DELETE FROM account_sessions WHERE digest = $1";

const SIGNED_IN: &str = "signed-in";
const SECOND_FACTOR_OWED: &str = "second-factor-owed";

const CLOSE: &str = "DELETE FROM account_codes WHERE account = $1 AND purpose = $2 AND code = $3";

const KEEP_TICKET: &str = "
    INSERT INTO account_invitation_tickets (digest, invitation, email, until, project, by_an_operator)
    VALUES ($1, $2, $3, $4, $5, $6)
    ON CONFLICT DO NOTHING
";

const FIND_TICKET: &str = "
    SELECT invitation, email, until, project, by_an_operator
    FROM account_invitation_tickets
    WHERE digest = $1 AND NOT ended
";

const TAKE_TICKET: &str = "
    UPDATE account_invitation_tickets
    SET ended = TRUE
    WHERE digest = $1 AND NOT ended
    RETURNING invitation, email, until, project, by_an_operator
";

const DIGEST_OF_TICKET: &str =
    "SELECT digest FROM account_invitation_tickets WHERE invitation = $1 AND NOT ended";

const DROP_TICKET: &str = "DELETE FROM account_invitation_tickets WHERE invitation = $1";

const REMEMBER_TRASH: &str = "
    INSERT INTO account_trashed_projects (project, version, in_the_trash)
    VALUES ($1, $2, $3)
    ON CONFLICT (project) DO UPDATE
    SET version = excluded.version, in_the_trash = excluded.in_the_trash
    WHERE account_trashed_projects.version < excluded.version
";

const FORGET_TRASH: &str = "DELETE FROM account_trashed_projects WHERE project = $1";

const IN_THE_TRASH: &str = "SELECT in_the_trash FROM account_trashed_projects WHERE project = $1";

const FORGET_TICKET: &str = "
    INSERT INTO account_invitation_tickets (invitation, ended)
    VALUES ($1, TRUE)
    ON CONFLICT (invitation) DO UPDATE
    SET ended = TRUE
";

const BEGIN_SETTING_UP: &str = "
    INSERT INTO account_second_factors (account, secret_being_set_up)
    VALUES ($1, $2)
    ON CONFLICT (account) DO UPDATE
    SET secret_being_set_up = EXCLUDED.secret_being_set_up
";

const SECOND_FACTOR_BEING_SET_UP: &str = "SELECT secret_being_set_up FROM account_second_factors WHERE account = $1 AND secret_being_set_up IS NOT NULL";

const FINISH_SETTING_UP: &str = "
    UPDATE account_second_factors
    SET secret_in_use = secret_being_set_up, secret_being_set_up = NULL, last_step = $3
    WHERE account = $1 AND secret_being_set_up = $2
";

const SECOND_FACTOR_OF: &str = "SELECT secret_in_use, last_step FROM account_second_factors WHERE account = $1 AND secret_in_use IS NOT NULL";

const ACCEPT_STEP: &str = "
    UPDATE account_second_factors
    SET last_step = $2
    WHERE account = $1 AND secret_in_use IS NOT NULL AND last_step < $2
";

const FORGET_RECOVERY_CODES: &str = "DELETE FROM account_recovery_codes WHERE account = $1";

const KEEP_RECOVERY_CODE: &str =
    "INSERT INTO account_recovery_codes (account, code_hash) VALUES ($1, $2)";

const RECOVERY_CODES_OF: &str =
    "SELECT code_hash FROM account_recovery_codes WHERE account = $1 ORDER BY code_hash";

const USE_RECOVERY_CODE: &str =
    "DELETE FROM account_recovery_codes WHERE account = $1 AND code_hash = $2";

const FORGET_SECOND_FACTOR: &str = "DELETE FROM account_second_factors WHERE account = $1";

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
    let begun: OffsetDateTime = row.try_get("begun").map_err(unreachable)?;
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
        begun,
        until,
    })
}

fn to_ticket(row: &PgRow) -> Result<InvitationTicket, SecretsError> {
    let email: String = row.try_get("email").map_err(unreachable)?;

    Ok(InvitationTicket {
        invitation: row.try_get("invitation").map_err(unreachable)?,
        to: EmailAddress::new(&email).map_err(unreachable)?,
        until: row.try_get("until").map_err(unreachable)?,
        scope: match row.try_get("project").map_err(unreachable)? {
            None => InvitationScope::Instance,
            Some(project) => InvitationScope::Project { project },
        },
        by_an_operator: row.try_get("by_an_operator").map_err(unreachable)?,
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
    async fn claim(
        &self,
        transaction: &mut Transaction,
        email: &EmailAddress,
        account: &AccountId,
        at: OffsetDateTime,
    ) -> Result<(), SecretsError> {
        let done = sqlx::query(CLAIM)
            .bind(email.as_str())
            .bind(account.to_string())
            .bind(at)
            .execute(transacting::postgres::connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        match done.rows_affected() {
            0 => Err(SecretsError::Taken),
            _ => Ok(()),
        }
    }

    async fn take_over(
        &self,
        transaction: &mut Transaction,
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
            .bind(claimed_before)
            .bind(at)
            .execute(transacting::postgres::connection(transaction).map_err(unreachable)?)
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

    async fn release(
        &self,
        transaction: &mut Transaction,
        email: &EmailAddress,
        account: &AccountId,
    ) -> Result<(), SecretsError> {
        sqlx::query(RELEASE)
            .bind(email.as_str())
            .bind(account.to_string())
            .execute(transacting::postgres::connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }
}

#[async_trait]
impl Passwords for PostgresSecrets {
    async fn keep(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        password: &PasswordHash,
    ) -> Result<(), SecretsError> {
        sqlx::query(KEEP)
            .bind(account.to_string())
            .bind(password.as_str())
            .execute(transacting::postgres::connection(transaction).map_err(unreachable)?)
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
    async fn open(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
        session: &Session,
    ) -> Result<(), SecretsError> {
        sqlx::query(OPEN)
            .bind(digest.as_str())
            .bind(session.account.to_string())
            .bind(written(session.stage))
            .bind(session.begun)
            .bind(session.until)
            .execute(transacting::postgres::connection(transaction).map_err(unreachable)?)
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

    async fn end(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
    ) -> Result<(), SecretsError> {
        sqlx::query(END_SESSION)
            .bind(digest.as_str())
            .execute(transacting::postgres::connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }
}

#[async_trait]
impl Verifications for PostgresSecrets {
    async fn issue(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        purpose: CodePurpose,
        pending: &PendingVerification,
    ) -> Result<(), SecretsError> {
        sqlx::query(ISSUE)
            .bind(account.to_string())
            .bind(purpose.as_str())
            .bind(pending.code.as_str())
            .bind(pending.until)
            .bind(i32::try_from(pending.tried).map_err(unreachable)?)
            .execute(transacting::postgres::connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn attempt(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        purpose: CodePurpose,
        within: u32,
    ) -> Result<Attempt, SecretsError> {
        let counted = sqlx::query(ATTEMPT)
            .bind(account.to_string())
            .bind(purpose.as_str())
            .bind(i32::try_from(within).map_err(unreachable)?)
            .fetch_optional(transacting::postgres::connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;
        if let Some(counted) = counted {
            return Ok(Attempt::Counted(to_pending(&counted)?));
        }

        let issued: i64 = sqlx::query_scalar(ISSUED)
            .bind(account.to_string())
            .bind(purpose.as_str())
            .fetch_one(transacting::postgres::connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(match issued {
            0 => Attempt::Nothing,
            _ => Attempt::Exhausted,
        })
    }

    async fn close(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        purpose: CodePurpose,
        code: &VerificationCode,
    ) -> Result<bool, SecretsError> {
        let closed = sqlx::query(CLOSE)
            .bind(account.to_string())
            .bind(purpose.as_str())
            .bind(code.as_str())
            .execute(transacting::postgres::connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(closed.rows_affected() > 0)
    }
}

#[async_trait]
impl Operators for PostgresSecrets {
    async fn keep_code(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
    ) -> Result<bool, SecretsError> {
        let kept = sqlx::query(KEEP_CODE)
            .bind(digest.as_str())
            .execute(transacting::postgres::connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(kept.rows_affected() == 1)
    }

    async fn holds_code(&self, digest: &TokenDigest) -> Result<bool, SecretsError> {
        sqlx::query_scalar(HOLDS_CODE)
            .bind(digest.as_str())
            .fetch_one(&self.pool)
            .await
            .map_err(unreachable)
    }

    async fn take_code(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
    ) -> Result<bool, SecretsError> {
        let done = sqlx::query(TAKE_CODE)
            .bind(digest.as_str())
            .execute(transacting::postgres::connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(done.rows_affected() == 1)
    }

    async fn appoint(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), SecretsError> {
        sqlx::query(APPOINT)
            .bind(account.to_string())
            .execute(transacting::postgres::connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn dismiss(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<Dismissal, SecretsError> {
        let appointed: Vec<String> = sqlx::query_scalar(EVERY_OPERATOR)
            .fetch_all(transacting::postgres::connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;
        let dismissal = if !appointed.contains(&account.to_string()) {
            Dismissal::NotAppointed
        } else if appointed.len() == 1 {
            Dismissal::LastOne
        } else {
            sqlx::query(DISMISS)
                .bind(account.to_string())
                .execute(transacting::postgres::connection(transaction).map_err(unreachable)?)
                .await
                .map_err(unreachable)?;
            Dismissal::Dismissed
        };

        Ok(dismissal)
    }

    async fn is_appointed(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<bool, SecretsError> {
        sqlx::query_scalar(APPOINTED)
            .bind(account.to_string())
            .fetch_one(transacting::postgres::connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)
    }

    async fn any_appointed(&self) -> Result<bool, SecretsError> {
        sqlx::query_scalar(ANY_APPOINTED)
            .fetch_one(&self.pool)
            .await
            .map_err(unreachable)
    }
}

#[async_trait]
impl InvitationTickets for PostgresSecrets {
    async fn keep_ticket(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
        ticket: &InvitationTicket,
    ) -> Result<bool, SecretsError> {
        let done = sqlx::query(KEEP_TICKET)
            .bind(digest.as_str())
            .bind(&ticket.invitation)
            .bind(ticket.to.as_str())
            .bind(ticket.until)
            .bind(match &ticket.scope {
                InvitationScope::Instance => None,
                InvitationScope::Project { project } => Some(project.as_str()),
            })
            .bind(ticket.by_an_operator)
            .execute(transacting::postgres::connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(done.rows_affected() == 1)
    }

    async fn find_ticket(
        &self,
        digest: &TokenDigest,
    ) -> Result<Option<InvitationTicket>, SecretsError> {
        sqlx::query(FIND_TICKET)
            .bind(digest.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(unreachable)?
            .as_ref()
            .map(to_ticket)
            .transpose()
    }

    async fn take_ticket(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
    ) -> Result<Option<InvitationTicket>, SecretsError> {
        sqlx::query(TAKE_TICKET)
            .bind(digest.as_str())
            .fetch_optional(transacting::postgres::connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?
            .as_ref()
            .map(to_ticket)
            .transpose()
    }

    async fn digest_of_ticket(
        &self,
        invitation: &str,
    ) -> Result<Option<TokenDigest>, SecretsError> {
        let found: Option<String> = sqlx::query_scalar(DIGEST_OF_TICKET)
            .bind(invitation)
            .fetch_optional(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(found.map(TokenDigest::kept))
    }

    async fn forget_ticket(
        &self,
        transaction: &mut Transaction,
        invitation: &str,
    ) -> Result<(), SecretsError> {
        sqlx::query(FORGET_TICKET)
            .bind(invitation)
            .execute(transacting::postgres::connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn drop_ticket(
        &self,
        transaction: &mut Transaction,
        invitation: &str,
    ) -> Result<(), SecretsError> {
        sqlx::query(DROP_TICKET)
            .bind(invitation)
            .execute(transacting::postgres::connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn remember_trash(
        &self,
        transaction: &mut Transaction,
        project: &str,
        version: u64,
        in_the_trash: bool,
    ) -> Result<(), SecretsError> {
        sqlx::query(REMEMBER_TRASH)
            .bind(project)
            .bind(i64::try_from(version).map_err(unreachable)?)
            .bind(in_the_trash)
            .execute(transacting::postgres::connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget_trash(
        &self,
        transaction: &mut Transaction,
        project: &str,
    ) -> Result<(), SecretsError> {
        sqlx::query(FORGET_TRASH)
            .bind(project)
            .execute(transacting::postgres::connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn is_in_the_trash(
        &self,
        transaction: &mut Transaction,
        project: &str,
    ) -> Result<bool, SecretsError> {
        let kept: Option<bool> = sqlx::query_scalar(IN_THE_TRASH)
            .bind(project)
            .fetch_optional(transacting::postgres::connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(kept.unwrap_or(false))
    }
}

fn to_step(step: i64) -> Result<TotpStep, SecretsError> {
    u64::try_from(step).map(TotpStep::kept).map_err(unreachable)
}

fn written_step(step: TotpStep) -> Result<i64, SecretsError> {
    i64::try_from(step.get()).map_err(unreachable)
}

fn to_second_factor(row: &PgRow) -> Result<KeptSecondFactor, SecretsError> {
    let secret: Vec<u8> = row.try_get("secret_in_use").map_err(unreachable)?;
    let last_step: i64 = row.try_get("last_step").map_err(unreachable)?;

    Ok(KeptSecondFactor {
        secret: SealedTotpSecret::kept(secret),
        last_accepted: to_step(last_step)?,
    })
}

#[async_trait]
impl SecondFactors for PostgresSecrets {
    async fn begin_setting_up(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        secret: &SealedTotpSecret,
    ) -> Result<(), SecretsError> {
        sqlx::query(BEGIN_SETTING_UP)
            .bind(account.to_string())
            .bind(secret.as_bytes())
            .execute(transacting::postgres::connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn second_factor_being_set_up(
        &self,
        account: &AccountId,
    ) -> Result<Option<SealedTotpSecret>, SecretsError> {
        let being_set_up: Option<Vec<u8>> = sqlx::query_scalar(SECOND_FACTOR_BEING_SET_UP)
            .bind(account.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(being_set_up.map(SealedTotpSecret::kept))
    }

    async fn finish_setting_up(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        being_set_up: &SealedTotpSecret,
        accepted: TotpStep,
    ) -> Result<bool, SecretsError> {
        let done = sqlx::query(FINISH_SETTING_UP)
            .bind(account.to_string())
            .bind(being_set_up.as_bytes())
            .bind(written_step(accepted)?)
            .execute(transacting::postgres::connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(done.rows_affected() == 1)
    }

    async fn second_factor_of(
        &self,
        account: &AccountId,
    ) -> Result<Option<KeptSecondFactor>, SecretsError> {
        sqlx::query(SECOND_FACTOR_OF)
            .bind(account.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(unreachable)?
            .map(|row| to_second_factor(&row))
            .transpose()
    }

    async fn accept_step(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        step: TotpStep,
    ) -> Result<bool, SecretsError> {
        let done = sqlx::query(ACCEPT_STEP)
            .bind(account.to_string())
            .bind(written_step(step)?)
            .execute(transacting::postgres::connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(done.rows_affected() == 1)
    }

    async fn keep_recovery_codes(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        codes: &[RecoveryCodeHash],
    ) -> Result<(), SecretsError> {
        sqlx::query(FORGET_RECOVERY_CODES)
            .bind(account.to_string())
            .execute(transacting::postgres::connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;
        for code in codes {
            sqlx::query(KEEP_RECOVERY_CODE)
                .bind(account.to_string())
                .bind(code.as_str())
                .execute(transacting::postgres::connection(transaction).map_err(unreachable)?)
                .await
                .map_err(unreachable)?;
        }

        Ok(())
    }

    async fn recovery_codes_of(
        &self,
        account: &AccountId,
    ) -> Result<Vec<RecoveryCodeHash>, SecretsError> {
        let kept: Vec<String> = sqlx::query_scalar(RECOVERY_CODES_OF)
            .bind(account.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(kept.into_iter().map(RecoveryCodeHash::kept).collect())
    }

    async fn use_recovery_code(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        code: &RecoveryCodeHash,
    ) -> Result<bool, SecretsError> {
        let done = sqlx::query(USE_RECOVERY_CODE)
            .bind(account.to_string())
            .bind(code.as_str())
            .execute(transacting::postgres::connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(done.rows_affected() == 1)
    }

    async fn forget_second_factor(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), SecretsError> {
        sqlx::query(FORGET_SECOND_FACTOR)
            .bind(account.to_string())
            .execute(transacting::postgres::connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;
        sqlx::query(FORGET_RECOVERY_CODES)
            .bind(account.to_string())
            .execute(transacting::postgres::connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }
}
