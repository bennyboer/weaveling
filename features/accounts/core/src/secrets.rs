use std::error::Error;
use std::sync::Arc;
use transacting::Transaction;

use async_trait::async_trait;
use thiserror::Error;
use time::OffsetDateTime;

use crate::code::{CodeDigest, CodePurpose, PendingVerification};
use crate::id::AccountId;
use crate::key::KeyId;
use crate::password::PasswordHash;
use crate::recovery_code::RecoveryCodeHash;
use crate::second_factor::SecondStepWay;
use crate::session::{Session, TokenDigest};
use crate::ticket::InvitationTicket;
use crate::totp::TotpStep;
use crate::totp_key::SealedTotpSecret;
use email::EmailAddress;
use throttling::Throttles;

#[derive(Debug, Error)]
pub enum SecretsError {
    #[error("this address already belongs to an account")]
    Taken,
    #[error(transparent)]
    Backend(Box<dyn Error + Send + Sync>),
}

#[derive(Clone)]
pub struct SecretStores {
    pub email_addresses: Arc<dyn EmailAddresses>,
    pub passwords: Arc<dyn Passwords>,
    pub verifications: Arc<dyn Verifications>,
    pub sessions: Arc<dyn Sessions>,
    pub throttles: Arc<dyn Throttles>,
    pub operators: Arc<dyn Operators>,
    pub invitation_tickets: Arc<dyn InvitationTickets>,
    pub second_factors: Arc<dyn SecondFactors>,
    pub key_credentials: Arc<dyn KeyCredentials>,
}

impl SecretStores {
    pub fn kept_by<S>(secrets: Arc<S>, throttles: Arc<dyn Throttles>) -> Self
    where
        S: EmailAddresses
            + Passwords
            + Verifications
            + Sessions
            + Operators
            + InvitationTickets
            + SecondFactors
            + KeyCredentials
            + 'static,
    {
        Self {
            email_addresses: secrets.clone(),
            passwords: secrets.clone(),
            verifications: secrets.clone(),
            sessions: secrets.clone(),
            throttles,
            operators: secrets.clone(),
            invitation_tickets: secrets.clone(),
            second_factors: secrets.clone(),
            key_credentials: secrets,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attempt {
    Nothing,
    Exhausted,
    Counted(PendingVerification),
}

#[async_trait]
pub trait EmailAddresses: Send + Sync {
    async fn claim(
        &self,
        transaction: &mut Transaction,
        email: &EmailAddress,
        account: &AccountId,
        at: OffsetDateTime,
    ) -> Result<(), SecretsError>;

    async fn take_over(
        &self,
        transaction: &mut Transaction,
        email: &EmailAddress,
        from: &AccountId,
        to: &AccountId,
        claimed_before: OffsetDateTime,
        at: OffsetDateTime,
    ) -> Result<bool, SecretsError>;

    async fn holder(&self, email: &EmailAddress) -> Result<Option<AccountId>, SecretsError>;

    async fn release(
        &self,
        transaction: &mut Transaction,
        email: &EmailAddress,
        account: &AccountId,
    ) -> Result<(), SecretsError>;
}

#[async_trait]
pub trait Passwords: Send + Sync {
    async fn keep(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        password: &PasswordHash,
    ) -> Result<(), SecretsError>;

    async fn of(&self, account: &AccountId) -> Result<Option<PasswordHash>, SecretsError>;
    /// Forgets the account's password, for good.
    async fn forget_password_of(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), SecretsError>;
}

#[async_trait]
pub trait Sessions: Send + Sync {
    async fn open(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
        session: &Session,
    ) -> Result<(), SecretsError>;

    async fn find(&self, digest: &TokenDigest) -> Result<Option<Session>, SecretsError>;

    async fn end(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
    ) -> Result<(), SecretsError>;
    /// Ends every session of the account, at whatever stage.
    async fn end_every_session_of(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), SecretsError>;
    /// Forgets at most `at_most` sessions that expired before the given time, oldest first.
    /// Returns how many were forgotten.
    async fn forget_sessions_run_out_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, SecretsError>;
}

#[async_trait]
pub trait Verifications: Send + Sync {
    async fn issue(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        purpose: CodePurpose,
        pending: &PendingVerification,
    ) -> Result<(), SecretsError>;

    async fn attempt(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        purpose: CodePurpose,
        within: u32,
    ) -> Result<Attempt, SecretsError>;

    async fn close(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        purpose: CodePurpose,
        digest: &CodeDigest,
    ) -> Result<bool, SecretsError>;
    /// Forgets every code issued to the account, for whatever purpose.
    async fn forget_codes_of(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), SecretsError>;
    /// Forgets at most `at_most` codes that expired before the given time, oldest first.
    /// Returns how many were forgotten.
    async fn forget_codes_run_out_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, SecretsError>;
}

#[async_trait]
pub trait Operators: Send + Sync {
    async fn keep_code(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
    ) -> Result<bool, SecretsError>;

    async fn holds_code(&self, digest: &TokenDigest) -> Result<bool, SecretsError>;

    /// Reserves the code for the first account that signs up with it, so nobody else can sign up with it.
    /// Returns false if the code is not stored, or if it is already reserved for another account.
    async fn hold_code(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
        account: &AccountId,
    ) -> Result<bool, SecretsError>;

    /// Takes the code, once, for the account that becomes the operator.
    /// This only works if the code is not reserved for another account.
    async fn take_code(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
        account: &AccountId,
    ) -> Result<bool, SecretsError>;

    async fn appoint(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), SecretsError>;

    async fn dismiss(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<Dismissal, SecretsError>;

    async fn is_appointed(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<bool, SecretsError>;

    async fn any_appointed(&self) -> Result<bool, SecretsError>;
}

#[async_trait]
pub trait InvitationTickets: Send + Sync {
    async fn keep_ticket(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
        ticket: &InvitationTicket,
    ) -> Result<bool, SecretsError>;

    async fn find_ticket(
        &self,
        digest: &TokenDigest,
    ) -> Result<Option<InvitationTicket>, SecretsError>;

    async fn take_ticket(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
    ) -> Result<Option<InvitationTicket>, SecretsError>;

    async fn digest_of_ticket(&self, invitation: &str)
    -> Result<Option<TokenDigest>, SecretsError>;

    async fn forget_ticket(
        &self,
        transaction: &mut Transaction,
        invitation: &str,
    ) -> Result<(), SecretsError>;

    async fn drop_ticket(
        &self,
        transaction: &mut Transaction,
        invitation: &str,
    ) -> Result<(), SecretsError>;
}

/// TOTP as it is kept: its sealed secret, and the step of the last code it accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeptTotp {
    pub secret: SealedTotpSecret,
    pub last_accepted: TotpStep,
}

/// An account's second-factor secrets.
///
/// A TOTP secret being set up is kept until a code for it is confirmed.
/// Then it becomes the secret in use for signing in.
/// Setting up TOTP again leaves the old secret in use until the new setup is finished.
/// The account's recovery codes are stored too, as hashes.
#[async_trait]
pub trait SecondFactors: Send + Sync {
    async fn begin_setting_up(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        secret: &SealedTotpSecret,
    ) -> Result<(), SecretsError>;

    async fn totp_being_set_up(
        &self,
        account: &AccountId,
    ) -> Result<Option<SealedTotpSecret>, SecretsError>;

    async fn finish_setting_up(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        being_set_up: &SealedTotpSecret,
        accepted: TotpStep,
    ) -> Result<bool, SecretsError>;

    async fn totp_of(&self, account: &AccountId) -> Result<Option<KeptTotp>, SecretsError>;

    async fn accept_step(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        step: TotpStep,
    ) -> Result<bool, SecretsError>;

    async fn keep_recovery_codes(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        codes: &[RecoveryCodeHash],
    ) -> Result<(), SecretsError>;

    async fn recovery_codes_of(
        &self,
        account: &AccountId,
    ) -> Result<Vec<RecoveryCodeHash>, SecretsError>;

    async fn use_recovery_code(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        code: &RecoveryCodeHash,
    ) -> Result<bool, SecretsError>;

    /// Forgets the account's TOTP: the one in use and any being set up.
    /// Its recovery codes stay, because they belong to the second step as a whole.
    async fn forget_totp(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), SecretsError>;

    async fn forget_recovery_codes(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), SecretsError>;

    /// Records the way that answered the account's second step, replacing the one recorded before.
    async fn note_answered_by(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        way: SecondStepWay,
    ) -> Result<(), SecretsError>;

    async fn answered_last_by(
        &self,
        account: &AccountId,
    ) -> Result<Option<SecondStepWay>, SecretsError>;
    /// Forgets the account's whole second step.
    /// That is the TOTP in use and any being set up, the recovery codes, and the way that answered last.
    async fn forget_second_factors_of(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), SecretsError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dismissal {
    Dismissed,
    LastOne,
    NotAppointed,
}

/// A key's credential as webauthn-rs wrote it, stored whole and not read here.
/// It contains the public key, the counter, and what the key can do.
/// Its id comes from the browser, so all accounts share one space of ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyCredential {
    pub id: Vec<u8>,
    pub written: String,
}

/// The credentials of every account's keys, one per key.
/// Each credential belongs to exactly one key of one account.
#[async_trait]
pub trait KeyCredentials: Send + Sync {
    /// Stores a new key's credential.
    /// Returns false if a key of any account already holds a credential with that id.
    async fn keep_credential(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        key: &KeyId,
        credential: &KeyCredential,
    ) -> Result<bool, SecretsError>;

    /// An account's credentials, the oldest key first.
    async fn credentials_of(
        &self,
        account: &AccountId,
    ) -> Result<Vec<(KeyId, KeyCredential)>, SecretsError>;

    /// Overwrites a key's credential with its state after a use.
    /// Returns false if the key has no credential.
    async fn renew_credential(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        key: &KeyId,
        written: &str,
    ) -> Result<bool, SecretsError>;

    async fn forget_credential(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        key: &KeyId,
    ) -> Result<(), SecretsError>;
    /// Forgets the credentials of every key of the account.
    async fn forget_credentials_of(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), SecretsError>;
}
