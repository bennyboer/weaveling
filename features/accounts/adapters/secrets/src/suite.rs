use accounts_core::{
    AccountId, Attempt, CodeDigest, CodePurpose, Dismissal, EmailAddress, EmailAddresses,
    InvitationScope, InvitationTicket, InvitationTickets, KeyCredentials, Operators, PasswordHash,
    Passwords, PendingVerification, SecondFactors, SecretsError, Session, SessionToken, Sessions,
    Stage, TokenDigest, Verifications,
};
use std::ops::Deref;

use futures_util::future;
use time::OffsetDateTime;
use transacting::{Transaction, Transactions, Tried};

mod forgetting;
mod key_credentials;
mod second_factors;

pub use forgetting::*;
pub use key_credentials::*;
pub use second_factors::*;

pub trait Secrets:
    EmailAddresses
    + InvitationTickets
    + Operators
    + Passwords
    + SecondFactors
    + KeyCredentials
    + Sessions
    + Verifications
{
}

impl<
    S: EmailAddresses
        + InvitationTickets
        + Operators
        + Passwords
        + SecondFactors
        + KeyCredentials
        + Sessions
        + Verifications,
> Secrets for S
{
}

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: Secrets;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

    fn transactions(&self) -> &dyn Transactions;

    fn committing(&self) -> Committing<'_, Self::Store> {
        Committing {
            store: self.store(),
            transactions: self.transactions(),
        }
    }

    async fn cleanup(self);
}

pub struct Committing<'a, S> {
    store: &'a S,
    transactions: &'a dyn Transactions,
}

impl<S: Secrets> Committing<'_, S> {
    pub async fn claim(
        &self,
        email: &EmailAddress,
        account: &AccountId,
        at: OffsetDateTime,
    ) -> Result<(), SecretsError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.store
                .claim(transaction, email, account, at)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn take_over(
        &self,
        email: &EmailAddress,
        from: &AccountId,
        to: &AccountId,
        claimed_before: OffsetDateTime,
        at: OffsetDateTime,
    ) -> Result<bool, SecretsError> {
        transacting::retrying!(self.transactions, |transaction| {
            let taken = self
                .store
                .take_over(transaction, email, from, to, claimed_before, at)
                .await
                .map_err(Tried::Refused)?;

            Ok(taken)
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn release(
        &self,
        email: &EmailAddress,
        account: &AccountId,
    ) -> Result<(), SecretsError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.store
                .release(transaction, email, account)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn keep(
        &self,
        account: &AccountId,
        password: &PasswordHash,
    ) -> Result<(), SecretsError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.store
                .keep(transaction, account, password)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn keep_ticket(
        &self,
        digest: &TokenDigest,
        ticket: &InvitationTicket,
    ) -> Result<bool, SecretsError> {
        transacting::retrying!(self.transactions, |transaction| {
            let kept = self
                .store
                .keep_ticket(transaction, digest, ticket)
                .await
                .map_err(Tried::Refused)?;

            Ok(kept)
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn take_ticket(
        &self,
        digest: &TokenDigest,
    ) -> Result<Option<InvitationTicket>, SecretsError> {
        transacting::retrying!(self.transactions, |transaction| {
            let taken = self
                .store
                .take_ticket(transaction, digest)
                .await
                .map_err(Tried::Refused)?;

            Ok(taken)
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn forget_ticket(&self, invitation: &str) -> Result<(), SecretsError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.store
                .forget_ticket(transaction, invitation)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn drop_ticket(&self, invitation: &str) -> Result<(), SecretsError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.store
                .drop_ticket(transaction, invitation)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn open(&self, digest: &TokenDigest, session: &Session) -> Result<(), SecretsError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.store
                .open(transaction, digest, session)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn end(&self, digest: &TokenDigest) -> Result<(), SecretsError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.store
                .end(transaction, digest)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn issue(
        &self,
        account: &AccountId,
        purpose: CodePurpose,
        pending: &PendingVerification,
    ) -> Result<(), SecretsError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.store
                .issue(transaction, account, purpose, pending)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn attempt(
        &self,
        account: &AccountId,
        purpose: CodePurpose,
        within: u32,
    ) -> Result<Attempt, SecretsError> {
        transacting::retrying!(self.transactions, |transaction| {
            let done = self
                .store
                .attempt(transaction, account, purpose, within)
                .await
                .map_err(Tried::Refused)?;

            Ok(done)
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn close(
        &self,
        account: &AccountId,
        purpose: CodePurpose,
        code: &str,
    ) -> Result<bool, SecretsError> {
        transacting::retrying!(self.transactions, |transaction| {
            let closed = self
                .store
                .close(
                    transaction,
                    account,
                    purpose,
                    &CodeDigest::kept(code.to_owned()),
                )
                .await
                .map_err(Tried::Refused)?;

            Ok(closed)
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn keep_code(&self, digest: &TokenDigest) -> Result<bool, SecretsError> {
        transacting::retrying!(self.transactions, |transaction| {
            let kept = self
                .store
                .keep_code(transaction, digest)
                .await
                .map_err(Tried::Refused)?;

            Ok(kept)
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn is_appointed(&self, account: &AccountId) -> Result<bool, SecretsError> {
        transacting::retrying!(self.transactions, |transaction| {
            let appointed = self
                .store
                .is_appointed(transaction, account)
                .await
                .map_err(Tried::Refused)?;

            Ok(appointed)
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn take_code(&self, digest: &TokenDigest) -> Result<bool, SecretsError> {
        transacting::retrying!(self.transactions, |transaction| {
            let done = self
                .store
                .take_code(transaction, digest)
                .await
                .map_err(Tried::Refused)?;

            Ok(done)
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn appoint(&self, account: &AccountId) -> Result<(), SecretsError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.store
                .appoint(transaction, account)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn dismiss(&self, account: &AccountId) -> Result<Dismissal, SecretsError> {
        transacting::retrying!(self.transactions, |transaction| {
            let done = self
                .store
                .dismiss(transaction, account)
                .await
                .map_err(Tried::Refused)?;

            Ok(done)
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    async fn begun(&self) -> Result<Transaction, SecretsError> {
        self.transactions
            .begin()
            .await
            .map_err(|failure| SecretsError::Backend(Box::new(failure)))
    }
}

fn unreachable(failure: transacting::Failure) -> SecretsError {
    SecretsError::Backend(Box::new(failure))
}

impl<S> Deref for Committing<'_, S> {
    type Target = S;

    fn deref(&self) -> &S {
        self.store
    }
}

fn an_address(raw: &str) -> EmailAddress {
    EmailAddress::new(raw).expect("a plain address")
}

fn a_hash(phc: &str) -> PasswordHash {
    PasswordHash::kept(phc.to_owned())
}

fn a_pending_code(code: &str, until: i64) -> PendingVerification {
    PendingVerification {
        digest: CodeDigest::kept(code.to_owned()),
        until: clock::at(until),
        tried: 0,
    }
}

pub async fn a_claimed_address_names_its_holder<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: an address claimed by an account
    let account = AccountId::generate(clock::at(1_000));
    secrets
        .claim(&an_address("ada@example.com"), &account, clock::at(1_000))
        .await
        .expect("claiming should succeed");

    // then: the address names that account
    assert_eq!(
        secrets
            .holder(&an_address("ada@example.com"))
            .await
            .expect("reading should succeed"),
        Some(account)
    );
}

pub async fn an_address_never_claimed_names_nobody<S: Secrets>(secrets: &Committing<'_, S>) {
    // expect: an address never claimed names nobody
    assert_eq!(
        secrets
            .holder(&an_address("nobody@example.com"))
            .await
            .expect("reading should succeed"),
        None
    );
}

pub async fn an_address_is_held_by_one_account_only<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: an address claimed by an account
    let first = AccountId::generate(clock::at(1_000));
    secrets
        .claim(&an_address("ada@example.com"), &first, clock::at(1_000))
        .await
        .expect("claiming should succeed");

    // when: another account claims it
    let second = secrets
        .claim(
            &an_address("ada@example.com"),
            &AccountId::generate(clock::at(2_000)),
            clock::at(1_000),
        )
        .await;

    // then: the claim is refused
    assert!(matches!(second, Err(SecretsError::Taken)), "{second:?}");
    // and: the first account still holds it
    assert_eq!(
        secrets
            .holder(&an_address("ada@example.com"))
            .await
            .expect("reading should succeed"),
        Some(first),
        "two people racing for one address must not both get it, nor the second take it over"
    );
}

pub async fn one_account_may_hold_two_addresses<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: an account
    let account = AccountId::generate(clock::at(1_000));
    // when: it claims two addresses
    for address in ["ada@example.com", "ada@example.org"] {
        secrets
            .claim(&an_address(address), &account, clock::at(1_000))
            .await
            .expect("claiming should succeed");
    }

    // then: it holds both
    for address in ["ada@example.com", "ada@example.org"] {
        assert_eq!(
            secrets
                .holder(&an_address(address))
                .await
                .expect("reading should succeed"),
            Some(account),
            "changing an address holds the new one before the old one is let go"
        );
    }
}

pub async fn a_released_address_can_be_claimed_again<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: an address claimed by an account
    let first = AccountId::generate(clock::at(1_000));
    secrets
        .claim(&an_address("ada@example.com"), &first, clock::at(1_000))
        .await
        .expect("claiming should succeed");

    // when: the account releases it
    secrets
        .release(&an_address("ada@example.com"), &first)
        .await
        .expect("releasing should succeed");
    // then: nobody holds it
    assert_eq!(
        secrets
            .holder(&an_address("ada@example.com"))
            .await
            .expect("reading should succeed"),
        None
    );

    // and: another account may claim it
    secrets
        .claim(
            &an_address("ada@example.com"),
            &AccountId::generate(clock::at(2_000)),
            clock::at(1_000),
        )
        .await
        .expect("a released address is free again");
}

pub async fn an_address_is_released_only_by_the_account_holding_it<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: an address claimed by an account
    let holder = AccountId::generate(clock::at(1_000));
    secrets
        .claim(&an_address("ada@example.com"), &holder, clock::at(1_000))
        .await
        .expect("claiming should succeed");

    // when: another account releases it
    secrets
        .release(
            &an_address("ada@example.com"),
            &AccountId::generate(clock::at(2_000)),
        )
        .await
        .expect("releasing what is not held is harmless");

    // then: the first account still holds it
    assert_eq!(
        secrets
            .holder(&an_address("ada@example.com"))
            .await
            .expect("reading should succeed"),
        Some(holder),
        "a sign-up undoing itself must never free an address someone else holds"
    );
}

pub async fn a_kept_password_is_read_back_until_a_new_one_replaces_it<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: an account
    let account = AccountId::generate(clock::at(1_000));
    // when: a password is kept for it
    secrets
        .keep(&account, &a_hash("$argon2id$first"))
        .await
        .expect("keeping should succeed");
    // then: it is read back
    assert_eq!(
        secrets.of(&account).await.expect("reading should succeed"),
        Some(a_hash("$argon2id$first"))
    );

    // when: a new one is kept
    secrets
        .keep(&account, &a_hash("$argon2id$second"))
        .await
        .expect("keeping again should succeed");
    // then: only the new one is read back
    assert_eq!(
        secrets.of(&account).await.expect("reading should succeed"),
        Some(a_hash("$argon2id$second")),
        "only the latest password is kept, so a leaked table holds no old one"
    );
}

pub async fn each_account_has_a_password_of_its_own_or_none<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: a password kept for one account
    let ada = AccountId::generate(clock::at(1_000));
    secrets
        .keep(&ada, &a_hash("$argon2id$ada"))
        .await
        .expect("keeping should succeed");

    // then: another account has none
    assert_eq!(
        secrets
            .of(&AccountId::generate(clock::at(2_000)))
            .await
            .expect("reading should succeed"),
        None,
        "an account may sign in without a password one day, by a key or elsewhere"
    );
}

pub async fn each_attempt_at_an_issued_code_is_counted_until_it_is_closed<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: a code issued to an account
    let account = AccountId::generate(clock::at(1_000));
    secrets
        .issue(
            &account,
            CodePurpose::ConfirmAddress,
            &a_pending_code("004217", 1_900),
        )
        .await
        .expect("issuing should succeed");

    // then: each attempt at it is counted
    for tried in 1..=2 {
        assert_eq!(
            secrets
                .attempt(&account, CodePurpose::ConfirmAddress, 5)
                .await
                .expect("attempting should succeed"),
            Attempt::Counted(PendingVerification {
                tried,
                ..a_pending_code("004217", 1_900)
            })
        );
    }

    // when: it is closed
    secrets
        .close(&account, CodePurpose::ConfirmAddress, "004217")
        .await
        .expect("closing should succeed");
    // then: there is nothing left to attempt
    assert_eq!(
        secrets
            .attempt(&account, CodePurpose::ConfirmAddress, 5)
            .await
            .expect("attempting should succeed"),
        Attempt::Nothing
    );
}

pub async fn attempts_beyond_the_limit_are_not_counted_but_exhausted<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: a code issued to an account
    let account = AccountId::generate(clock::at(1_000));
    secrets
        .issue(
            &account,
            CodePurpose::ConfirmAddress,
            &a_pending_code("004217", 1_900),
        )
        .await
        .expect("issuing should succeed");

    // and: attempted up to the limit
    for _ in 0..3 {
        assert!(matches!(
            secrets
                .attempt(&account, CodePurpose::ConfirmAddress, 3)
                .await,
            Ok(Attempt::Counted(_))
        ));
    }

    // then: the next attempt finds it exhausted
    assert_eq!(
        secrets
            .attempt(&account, CodePurpose::ConfirmAddress, 3)
            .await
            .expect("attempting should succeed"),
        Attempt::Exhausted
    );
}

pub async fn attempts_made_at_once_are_counted_one_by_one<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: a code issued to an account
    let account = AccountId::generate(clock::at(1_000));
    secrets
        .issue(
            &account,
            CodePurpose::ConfirmAddress,
            &a_pending_code("004217", 1_900),
        )
        .await
        .expect("issuing should succeed");

    // when: many attempts are made at once
    let attempts = future::join_all(
        (0..20).map(|_| secrets.attempt(&account, CodePurpose::ConfirmAddress, 5)),
    )
    .await;

    // then: as many are counted as the limit allows
    let counted = attempts
        .iter()
        .filter(|attempt| matches!(attempt, Ok(Attempt::Counted(_))))
        .count();
    assert_eq!(
        counted, 5,
        "guesses sent together must not all read the count before any of them raises it, \
         or the limit holds only for a guesser patient enough to wait"
    );
}

pub async fn a_new_code_starts_its_attempts_afresh<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: a code issued to an account
    let account = AccountId::generate(clock::at(1_000));
    secrets
        .issue(
            &account,
            CodePurpose::ConfirmAddress,
            &a_pending_code("004217", 1_900),
        )
        .await
        .expect("issuing should succeed");
    // and: attempted up to the limit
    for _ in 0..2 {
        let _counted = secrets
            .attempt(&account, CodePurpose::ConfirmAddress, 2)
            .await;
    }

    // when: a new code is issued
    secrets
        .issue(
            &account,
            CodePurpose::ConfirmAddress,
            &a_pending_code("731905", 2_900),
        )
        .await
        .expect("issuing again should succeed");

    // then: an attempt at it is counted as the first
    assert_eq!(
        secrets
            .attempt(&account, CodePurpose::ConfirmAddress, 2)
            .await
            .expect("attempting should succeed"),
        Attempt::Counted(PendingVerification {
            tried: 1,
            ..a_pending_code("731905", 2_900)
        }),
        "a new code replaces the old one, and the guesses at the old one no longer count"
    );
}

pub async fn each_account_has_a_code_of_its_own<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: a code issued to each of two accounts
    let ada = AccountId::generate(clock::at(1_000));
    let bea = AccountId::generate(clock::at(2_000));
    secrets
        .issue(
            &ada,
            CodePurpose::ConfirmAddress,
            &a_pending_code("111111", 1_900),
        )
        .await
        .expect("issuing should succeed");
    secrets
        .issue(
            &bea,
            CodePurpose::ConfirmAddress,
            &a_pending_code("222222", 2_900),
        )
        .await
        .expect("issuing should succeed");

    // when: one account's code is attempted and closed
    let _counted = secrets.attempt(&ada, CodePurpose::ConfirmAddress, 5).await;
    secrets
        .close(&ada, CodePurpose::ConfirmAddress, "111111")
        .await
        .expect("closing should succeed");

    // then: the other's is untouched
    assert_eq!(
        secrets
            .attempt(&bea, CodePurpose::ConfirmAddress, 5)
            .await
            .expect("attempting should succeed"),
        Attempt::Counted(PendingVerification {
            tried: 1,
            ..a_pending_code("222222", 2_900)
        })
    );
}

pub async fn attempting_or_closing_where_nothing_was_issued_is_harmless<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: an account with no code
    let account = AccountId::generate(clock::at(1_000));

    // then: attempting finds nothing
    assert_eq!(
        secrets
            .attempt(&account, CodePurpose::ConfirmAddress, 5)
            .await
            .expect("attempting should succeed"),
        Attempt::Nothing
    );
    // and: closing closes nothing
    assert!(
        !secrets
            .close(&account, CodePurpose::ConfirmAddress, "123456")
            .await
            .expect("closing nothing is fine"),
        "nothing was there to close"
    );
}

pub async fn a_code_is_closed_only_by_itself_and_only_once<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: an issued code
    let account = AccountId::generate(clock::at(1_000));
    secrets
        .issue(
            &account,
            CodePurpose::ResetPassword,
            &a_pending_code("314159", 1_900),
        )
        .await
        .expect("issuing should succeed");

    // when: it is closed by another code, by itself and by itself again
    let by_another = secrets
        .close(&account, CodePurpose::ResetPassword, "271828")
        .await
        .expect("closing should succeed");
    let by_itself = secrets
        .close(&account, CodePurpose::ResetPassword, "314159")
        .await
        .expect("closing should succeed");
    let again = secrets
        .close(&account, CodePurpose::ResetPassword, "314159")
        .await
        .expect("closing should succeed");

    // then: only the first close by itself closes it
    assert_eq!(
        (by_another, by_itself, again),
        (false, true, false),
        "a code closes only as the code it is, and a second use finds it gone"
    );
}

fn a_session(stage: Stage) -> Session {
    Session {
        account: AccountId::generate(clock::at(1_000)),
        stage,
        begun: clock::at(1_699_000_000),
        until: clock::at(1_700_000_000),
    }
}

pub async fn an_opened_session_is_found_by_its_tokens_digest_until_it_ends<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: a session opened under a token's digest
    let token = SessionToken::drawn();
    let session = a_session(Stage::SignedIn);
    secrets
        .open(&token.digest(), &session)
        .await
        .expect("opening should succeed");

    // then: the token presented finds it
    assert_eq!(
        secrets
            .find(&SessionToken::presented(token.as_str()).digest())
            .await
            .expect("finding should succeed"),
        Some(session)
    );

    // when: it ends
    secrets
        .end(&token.digest())
        .await
        .expect("closing should succeed");
    // then: it is found no more
    assert_eq!(
        secrets
            .find(&token.digest())
            .await
            .expect("finding should succeed"),
        None
    );
}

pub async fn a_token_nobody_was_given_finds_nothing<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: an opened session
    secrets
        .open(&SessionToken::drawn().digest(), &a_session(Stage::SignedIn))
        .await
        .expect("opening should succeed");

    // then: a token nobody was given finds nothing
    assert_eq!(
        secrets
            .find(&SessionToken::drawn().digest())
            .await
            .expect("finding should succeed"),
        None
    );
}

pub async fn each_stage_is_kept_as_it_was<S: Secrets>(secrets: &Committing<'_, S>) {
    // expect: a session opened at either stage is found at that stage
    for stage in [Stage::SignedIn, Stage::SecondFactorOwed] {
        let token = SessionToken::drawn();
        secrets
            .open(&token.digest(), &a_session(stage))
            .await
            .expect("opening should succeed");

        assert_eq!(
            secrets
                .find(&token.digest())
                .await
                .expect("finding should succeed")
                .map(|session| session.stage),
            Some(stage),
            "a session owing its second factor must never come back signed in"
        );
    }
}

pub async fn ending_one_session_leaves_the_others_open<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: two open sessions
    let phone = SessionToken::drawn();
    let laptop = SessionToken::drawn();
    for token in [&phone, &laptop] {
        secrets
            .open(&token.digest(), &a_session(Stage::SignedIn))
            .await
            .expect("opening should succeed");
    }

    // when: one of them ends
    secrets
        .end(&phone.digest())
        .await
        .expect("closing should succeed");
    // and: a session never opened ends
    secrets
        .end(&SessionToken::drawn().digest())
        .await
        .expect("ending a session never opened is harmless");

    // then: the other is still open
    assert!(
        secrets
            .find(&laptop.digest())
            .await
            .expect("finding should succeed")
            .is_some()
    );
}

pub async fn a_code_for_one_purpose_is_no_code_for_another<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: a code issued for each of two purposes
    let account = AccountId::generate(clock::at(1_000));
    secrets
        .issue(
            &account,
            CodePurpose::ConfirmAddress,
            &a_pending_code("111111", 1_900),
        )
        .await
        .expect("issuing should succeed");
    secrets
        .issue(
            &account,
            CodePurpose::ResetPassword,
            &a_pending_code("222222", 2_900),
        )
        .await
        .expect("issuing should succeed");

    // when: the one for confirming the address is closed
    secrets
        .close(&account, CodePurpose::ConfirmAddress, "111111")
        .await
        .expect("closing should succeed");

    // then: none is left for confirming the address
    assert_eq!(
        secrets
            .attempt(&account, CodePurpose::ConfirmAddress, 5)
            .await
            .expect("attempting should succeed"),
        Attempt::Nothing
    );
    // and: the one for resetting the password still counts its attempts
    assert_eq!(
        secrets
            .attempt(&account, CodePurpose::ResetPassword, 5)
            .await
            .expect("attempting should succeed"),
        Attempt::Counted(PendingVerification {
            tried: 1,
            ..a_pending_code("222222", 2_900)
        }),
        "a code sent to reset a password must not confirm an address, nor fall with it"
    );
}

fn a_digest(of: &str) -> TokenDigest {
    TokenDigest::kept(of.to_owned())
}

pub async fn a_kept_code_is_held_until_taken_and_no_other_is<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: a kept code
    secrets
        .keep_code(&a_digest("first"))
        .await
        .expect("keeping should succeed");

    // then: it is held however often it is looked at
    for _ in 0..2 {
        assert!(
            secrets
                .holds_code(&a_digest("first"))
                .await
                .expect("looking should succeed"),
            "looking at a code does not use it up"
        );
    }
    // and: no other code is held
    assert!(
        !secrets
            .holds_code(&a_digest("second"))
            .await
            .expect("looking should succeed")
    );
    // when: it is taken
    secrets
        .take_code(&a_digest("first"))
        .await
        .expect("taking should succeed");
    // then: it is held no more
    assert!(
        !secrets
            .holds_code(&a_digest("first"))
            .await
            .expect("looking should succeed")
    );
}

pub async fn a_kept_code_is_taken_once<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: a kept code
    secrets
        .keep_code(&a_digest("first"))
        .await
        .expect("keeping should succeed");

    // then: it is taken
    assert!(
        secrets
            .take_code(&a_digest("first"))
            .await
            .expect("taking should succeed")
    );
    // and: not a second time
    assert!(
        !secrets
            .take_code(&a_digest("first"))
            .await
            .expect("taking should succeed"),
        "a code makes one operator, used once"
    );
}

pub async fn only_the_latest_code_kept_can_be_taken<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: two codes kept one after the other
    for digest in ["first", "second"] {
        secrets
            .keep_code(&a_digest(digest))
            .await
            .expect("keeping should succeed");
    }

    // then: the first is taken no more
    assert!(
        !secrets
            .take_code(&a_digest("first"))
            .await
            .expect("taking should succeed"),
        "a code logged at an earlier start is good no longer"
    );
    // and: the latest is taken
    assert!(
        secrets
            .take_code(&a_digest("second"))
            .await
            .expect("taking should succeed")
    );
}

pub async fn no_code_is_kept_once_someone_is_an_operator<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: an operator
    an_operator_appointed_at(secrets, 1_000).await;

    // when: a code is kept
    let kept = secrets
        .keep_code(&a_digest("first"))
        .await
        .expect("keeping should succeed");

    // then: it is not kept
    assert!(
        !kept,
        "a second instance starting after the first operator was made draws no code beside them"
    );
    // and: it is not held
    assert!(
        !secrets
            .holds_code(&a_digest("first"))
            .await
            .expect("asking should succeed")
    );
}

pub async fn a_code_kept_before_someone_became_an_operator_is_taken_no_more<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: a kept code
    assert!(
        secrets
            .keep_code(&a_digest("first"))
            .await
            .expect("keeping should succeed")
    );
    // and: an operator appointed since
    an_operator_appointed_at(secrets, 1_000).await;

    // then: the code is taken no more
    assert!(
        !secrets
            .take_code(&a_digest("first"))
            .await
            .expect("taking should succeed"),
        "the code is for the first operator only, so once there is one it lets nobody in"
    );
}

pub async fn an_operator_is_appointed_until_dismissed<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: two operators and an account never appointed
    let ada = an_operator_appointed_at(secrets, 1_000).await;
    let grace = an_operator_appointed_at(secrets, 2_000).await;
    let nobody = AccountId::generate(clock::at(3_000));

    // when: one operator is dismissed
    secrets
        .dismiss(&grace)
        .await
        .expect("dismissing should succeed");

    // then: the other is still appointed
    assert!(
        secrets
            .is_appointed(&ada)
            .await
            .expect("asking should succeed")
    );
    // and: the dismissed one is not
    assert!(
        !secrets
            .is_appointed(&grace)
            .await
            .expect("asking should succeed")
    );
    // and: nor the one never appointed
    assert!(
        !secrets
            .is_appointed(&nobody)
            .await
            .expect("asking should succeed")
    );
}

pub async fn no_code_kept_takes_nothing<S: Secrets>(secrets: &Committing<'_, S>) {
    // expect: taking a code where none is kept takes nothing
    assert!(
        !secrets
            .take_code(&a_digest("first"))
            .await
            .expect("taking should succeed")
    );
}

pub async fn a_code_taken_by_many_at_once_goes_to_one<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: a kept code
    secrets
        .keep_code(&a_digest("first"))
        .await
        .expect("keeping should succeed");

    // when: many take it at once
    let digest = a_digest("first");
    let taken = future::join_all((0..20).map(|_| secrets.take_code(&digest))).await;

    // then: one of them gets it
    assert_eq!(
        taken
            .iter()
            .filter(|taken| matches!(taken, Ok(true)))
            .count(),
        1,
        "two sent together must not both find the code before either takes it"
    );
}

pub async fn nobody_is_an_operator_until_appointed<S: Secrets>(secrets: &Committing<'_, S>) {
    // expect: nobody is an operator at first
    assert!(
        !secrets
            .any_appointed()
            .await
            .expect("reading should succeed")
    );

    // when: an account is appointed
    secrets
        .appoint(&AccountId::generate(clock::at(1_000)))
        .await
        .expect("appointing should succeed");

    // then: there is an operator
    assert!(
        secrets
            .any_appointed()
            .await
            .expect("reading should succeed")
    );
}

pub async fn appointing_twice_is_appointing_once<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: an account
    let account = AccountId::generate(clock::at(1_000));

    // when: it is appointed twice
    for _ in 0..2 {
        secrets
            .appoint(&account)
            .await
            .expect("appointing again should succeed too");
    }

    // then: there is an operator
    assert!(
        secrets
            .any_appointed()
            .await
            .expect("reading should succeed")
    );
}

async fn an_operator_appointed_at<S: Secrets>(
    secrets: &Committing<'_, S>,
    seconds: i64,
) -> AccountId {
    let account = AccountId::generate(clock::at(seconds));
    secrets
        .appoint(&account)
        .await
        .expect("appointing should succeed");

    account
}

pub async fn one_of_two_operators_is_dismissed<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: two operators
    let ada = an_operator_appointed_at(secrets, 1_000).await;
    let grace = an_operator_appointed_at(secrets, 2_000).await;

    // then: dismissing one dismisses it
    assert_eq!(
        secrets
            .dismiss(&ada)
            .await
            .expect("dismissing should succeed"),
        Dismissal::Dismissed
    );
    // and: dismissing it again finds it not appointed
    assert_eq!(
        secrets
            .dismiss(&ada)
            .await
            .expect("dismissing should succeed"),
        Dismissal::NotAppointed,
        "she was dismissed, so the next dismissal finds her gone"
    );
    // and: dismissing the other finds it the last one
    assert_eq!(
        secrets
            .dismiss(&grace)
            .await
            .expect("dismissing should succeed"),
        Dismissal::LastOne
    );
}

pub async fn the_last_operator_is_kept<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: a single operator
    let ada = an_operator_appointed_at(secrets, 1_000).await;

    // then: dismissing it finds it the last one
    assert_eq!(
        secrets
            .dismiss(&ada)
            .await
            .expect("dismissing should succeed"),
        Dismissal::LastOne
    );
    // and: there is still an operator
    assert!(
        secrets
            .any_appointed()
            .await
            .expect("reading should succeed")
    );
}

pub async fn someone_never_appointed_is_not_dismissed<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: two operators
    let _ada = an_operator_appointed_at(secrets, 1_000).await;
    let _grace = an_operator_appointed_at(secrets, 2_000).await;

    // then: dismissing an account never appointed finds it not appointed
    assert_eq!(
        secrets
            .dismiss(&AccountId::generate(clock::at(3_000)))
            .await
            .expect("dismissing should succeed"),
        Dismissal::NotAppointed
    );
}

pub async fn operators_dismissed_all_at_once_leave_one<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: ten operators
    let mut everyone = Vec::new();
    for seconds in 1..=10 {
        everyone.push(an_operator_appointed_at(secrets, seconds * 1_000).await);
    }

    // when: all of them are dismissed at once
    let dismissals =
        future::join_all(everyone.iter().map(|account| secrets.dismiss(account))).await;

    // then: all but one are dismissed
    let told: Vec<Dismissal> = dismissals
        .into_iter()
        .map(|dismissal| dismissal.expect("dismissing should succeed"))
        .collect();
    assert_eq!(
        told.iter()
            .filter(|told| **told == Dismissal::Dismissed)
            .count(),
        9,
        "two operators taking each other's role at once must not both count two and leave none: {told:?}"
    );
    // and: one is the last
    assert_eq!(
        told.iter()
            .filter(|told| **told == Dismissal::LastOne)
            .count(),
        1
    );
    // and: there is still an operator
    assert!(
        secrets
            .any_appointed()
            .await
            .expect("reading should succeed")
    );
}

fn a_ticket(invitation: &str, to: &str) -> InvitationTicket {
    InvitationTicket {
        invitation: invitation.to_owned(),
        to: an_address(to),
        until: clock::at(5_000),
        scope: InvitationScope::Instance,
        by_an_operator: true,
    }
}

pub async fn a_project_s_ticket_is_kept_with_its_project_and_who_issued_it<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: a ticket into a project issued by someone who is no operator
    let into_a_project = InvitationTicket {
        scope: InvitationScope::Project {
            project: "project_loom".to_owned(),
        },
        by_an_operator: false,
        ..a_ticket("invitation_ada", "ada@example.com")
    };
    secrets
        .keep_ticket(&a_digest("ada's"), &into_a_project)
        .await
        .expect("keeping should succeed");

    // then: it is found as it was kept
    assert_eq!(
        secrets
            .find_ticket(&a_digest("ada's"))
            .await
            .expect("finding should succeed"),
        Some(into_a_project.clone())
    );
    // and: it is taken as it was kept
    assert_eq!(
        secrets
            .take_ticket(&a_digest("ada's"))
            .await
            .expect("taking should succeed"),
        Some(into_a_project),
        "only an operator's invitation into a project may sign a new address up where sign-up is closed"
    );
}

pub async fn a_ticket_still_open_is_found_by_its_invitation_to_take_it_up_signed_in<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: three kept tickets
    for (digest, invitation, to) in [
        ("ada's", "invitation_ada", "ada@example.com"),
        ("bob's", "invitation_bob", "bob@example.com"),
        ("cy's", "invitation_cy", "cy@example.com"),
    ] {
        secrets
            .keep_ticket(&a_digest(digest), &a_ticket(invitation, to))
            .await
            .expect("keeping should succeed");
    }
    // and: one of them forgotten
    secrets
        .forget_ticket("invitation_bob")
        .await
        .expect("forgetting should succeed");
    // and: one of them taken
    secrets
        .take_ticket(&a_digest("cy's"))
        .await
        .expect("taking should succeed");

    // then: only the one still open is found by its invitation
    let digest_of = |invitation: &'static str| async move {
        secrets
            .digest_of_ticket(invitation)
            .await
            .expect("asking should succeed")
    };
    assert_eq!(digest_of("invitation_ada").await, Some(a_digest("ada's")));
    assert_eq!(
        digest_of("invitation_bob").await,
        None,
        "a withdrawn one is no more"
    );
    assert_eq!(digest_of("invitation_cy").await, None, "nor one taken up");
    assert_eq!(digest_of("invitation_nobody").await, None);
}

pub async fn a_found_ticket_is_left_to_be_taken<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: a kept ticket
    secrets
        .keep_ticket(
            &a_digest("ada's"),
            &a_ticket("invitation_ada", "ada@example.com"),
        )
        .await
        .expect("keeping should succeed");
    // and: another kept and forgotten
    secrets
        .keep_ticket(
            &a_digest("bob's"),
            &a_ticket("invitation_bob", "bob@example.com"),
        )
        .await
        .expect("keeping should succeed");
    secrets
        .forget_ticket("invitation_bob")
        .await
        .expect("forgetting should succeed");

    // then: the kept one is found however often it is looked at
    for _ in 0..2 {
        assert_eq!(
            secrets
                .find_ticket(&a_digest("ada's"))
                .await
                .expect("finding should succeed"),
            Some(a_ticket("invitation_ada", "ada@example.com")),
            "a link may be looked at before it is used"
        );
    }
    // and: the forgotten one is not found
    assert_eq!(
        secrets
            .find_ticket(&a_digest("bob's"))
            .await
            .expect("finding should succeed"),
        None,
        "a withdrawn invitation's link leads nowhere"
    );
    // and: nor one never kept
    assert_eq!(
        secrets
            .find_ticket(&a_digest("someone else's"))
            .await
            .expect("finding should succeed"),
        None
    );
    // when: the kept one is taken
    secrets
        .take_ticket(&a_digest("ada's"))
        .await
        .expect("taking should succeed")
        .expect("the ticket is still there to be taken");
    // then: it is found no more
    assert_eq!(
        secrets
            .find_ticket(&a_digest("ada's"))
            .await
            .expect("finding should succeed"),
        None,
        "a ticket taken is used up"
    );
}

pub async fn a_kept_ticket_is_taken_once_and_whole<S: Secrets>(secrets: &Committing<'_, S>) {
    // expect: a ticket never kept before is news
    assert!(
        secrets
            .keep_ticket(
                &a_digest("ada's"),
                &a_ticket("invitation_ada", "ada@example.com"),
            )
            .await
            .expect("keeping should succeed"),
        "a ticket never kept before is news"
    );
    // when: it is kept again
    secrets
        .keep_ticket(
            &a_digest("ada's"),
            &a_ticket("invitation_ada", "ada@example.com"),
        )
        .await
        .expect("keeping should succeed");

    // then: it is taken whole
    assert_eq!(
        secrets
            .take_ticket(&a_digest("ada's"))
            .await
            .expect("taking should succeed"),
        Some(a_ticket("invitation_ada", "ada@example.com"))
    );
    // and: only once
    assert_eq!(
        secrets
            .take_ticket(&a_digest("ada's"))
            .await
            .expect("taking should succeed"),
        None,
        "a ticket lets one account in"
    );
}

pub async fn a_digest_nobody_was_given_takes_no_ticket<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: a kept ticket
    secrets
        .keep_ticket(
            &a_digest("ada's"),
            &a_ticket("invitation_ada", "ada@example.com"),
        )
        .await
        .expect("keeping should succeed");

    // then: a digest nobody was given takes no ticket
    assert_eq!(
        secrets
            .take_ticket(&a_digest("someone else's"))
            .await
            .expect("taking should succeed"),
        None
    );
}

pub async fn a_ticket_kept_again_is_kept_once_as_it_first_was<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: a kept ticket
    secrets
        .keep_ticket(
            &a_digest("ada's"),
            &a_ticket("invitation_ada", "ada@example.com"),
        )
        .await
        .expect("keeping should succeed");

    // then: keeping it again is no news
    assert!(
        !secrets
            .keep_ticket(
                &a_digest("ada's"),
                &a_ticket("invitation_ada", "ada@example.com"),
            )
            .await
            .expect("a message told twice keeps its ticket once"),
        "what was kept already is no news"
    );
    // and: keeping another under its digest keeps nothing
    assert!(
        !secrets
            .keep_ticket(
                &a_digest("ada's"),
                &a_ticket("invitation_other", "eve@example.com"),
            )
            .await
            .expect("a digest already kept is not taken over")
    );

    // and: it is taken as it was first kept
    assert_eq!(
        secrets
            .take_ticket(&a_digest("ada's"))
            .await
            .expect("taking should succeed"),
        Some(a_ticket("invitation_ada", "ada@example.com"))
    );
    // and: only once
    assert_eq!(
        secrets
            .take_ticket(&a_digest("ada's"))
            .await
            .expect("taking should succeed"),
        None
    );
}

pub async fn a_forgotten_invitation_leaves_no_ticket_and_the_others_kept<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: the tickets of two invitations
    secrets
        .keep_ticket(
            &a_digest("ada's"),
            &a_ticket("invitation_ada", "ada@example.com"),
        )
        .await
        .expect("keeping should succeed");
    secrets
        .keep_ticket(
            &a_digest("bob's"),
            &a_ticket("invitation_bob", "bob@example.com"),
        )
        .await
        .expect("keeping should succeed");

    // when: one invitation is forgotten
    secrets
        .forget_ticket("invitation_ada")
        .await
        .expect("forgetting should succeed");
    // and: one never kept
    secrets
        .forget_ticket("invitation_never")
        .await
        .expect("forgetting what was never kept is harmless");

    // then: the forgotten one's ticket is gone
    assert_eq!(
        secrets
            .take_ticket(&a_digest("ada's"))
            .await
            .expect("taking should succeed"),
        None
    );
    // and: the other's is still kept
    assert_eq!(
        secrets
            .take_ticket(&a_digest("bob's"))
            .await
            .expect("taking should succeed"),
        Some(a_ticket("invitation_bob", "bob@example.com"))
    );
}

pub async fn a_ticket_taken_by_many_at_once_goes_to_one<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: a kept ticket
    secrets
        .keep_ticket(
            &a_digest("ada's"),
            &a_ticket("invitation_ada", "ada@example.com"),
        )
        .await
        .expect("keeping should succeed");

    // when: many take it at once
    let digest = a_digest("ada's");
    let taken = future::join_all((0..8).map(|_| secrets.take_ticket(&digest))).await;

    // then: one of them gets it
    assert_eq!(
        taken
            .into_iter()
            .map(|taken| taken.expect("taking should succeed"))
            .filter(Option::is_some)
            .count(),
        1
    );
}

pub async fn an_invitation_kept_again_by_another_digest_keeps_its_first<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: an invitation's ticket
    secrets
        .keep_ticket(
            &a_digest("ada's"),
            &a_ticket("invitation_ada", "ada@example.com"),
        )
        .await
        .expect("keeping should succeed");

    // then: keeping it under another digest keeps nothing
    assert!(
        !secrets
            .keep_ticket(
                &a_digest("another"),
                &a_ticket("invitation_ada", "ada@example.com"),
            )
            .await
            .expect("keeping should succeed"),
        "an invitation has one link"
    );
    // and: the other digest takes nothing
    assert_eq!(
        secrets
            .take_ticket(&a_digest("another"))
            .await
            .expect("taking should succeed"),
        None
    );
    // and: the first digest still takes it
    assert!(
        secrets
            .take_ticket(&a_digest("ada's"))
            .await
            .expect("taking should succeed")
            .is_some()
    );
}

pub async fn a_ticket_taken_is_not_kept_again<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: a ticket kept and taken
    let ticket = a_ticket("invitation_ada", "ada@example.com");
    secrets
        .keep_ticket(&a_digest("ada's"), &ticket)
        .await
        .expect("keeping should succeed");
    secrets
        .take_ticket(&a_digest("ada's"))
        .await
        .expect("taking should succeed");

    // then: keeping it again keeps nothing
    assert!(
        !secrets
            .keep_ticket(&a_digest("ada's"), &ticket)
            .await
            .expect("an issue told again keeps nothing")
    );

    // and: it is not taken again
    assert_eq!(
        secrets
            .take_ticket(&a_digest("ada's"))
            .await
            .expect("taking should succeed"),
        None,
        "a message told again must not let a second account in"
    );
}

pub async fn a_ticket_forgotten_is_not_kept_again<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: a ticket kept and its invitation forgotten
    let ticket = a_ticket("invitation_ada", "ada@example.com");
    secrets
        .keep_ticket(&a_digest("ada's"), &ticket)
        .await
        .expect("keeping should succeed");
    secrets
        .forget_ticket("invitation_ada")
        .await
        .expect("forgetting should succeed");

    // then: keeping it again keeps nothing
    assert!(
        !secrets
            .keep_ticket(&a_digest("ada's"), &ticket)
            .await
            .expect("an issue told again keeps nothing")
    );

    // and: it is not taken
    assert_eq!(
        secrets
            .take_ticket(&a_digest("ada's"))
            .await
            .expect("taking should succeed"),
        None,
        "a withdrawn link stays withdrawn, however late its issue is told again"
    );
}

pub async fn an_invitation_forgotten_before_its_ticket_came_keeps_none<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: a forgotten invitation
    secrets
        .forget_ticket("invitation_ada")
        .await
        .expect("forgetting should succeed");

    // then: keeping its ticket later keeps nothing
    assert!(
        !secrets
            .keep_ticket(
                &a_digest("ada's"),
                &a_ticket("invitation_ada", "ada@example.com"),
            )
            .await
            .expect("an issue told after its withdrawal keeps nothing")
    );

    // and: the ticket is not taken
    assert_eq!(
        secrets
            .take_ticket(&a_digest("ada's"))
            .await
            .expect("taking should succeed"),
        None
    );
}

pub async fn a_claim_made_before_the_moment_given_is_taken_over_from_its_holder<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: an address claimed by an account
    let abandoned = AccountId::generate(clock::at(1_000));
    let taking = AccountId::generate(clock::at(3_000));
    secrets
        .claim(&an_address("ada@example.com"), &abandoned, clock::at(1_000))
        .await
        .expect("claiming should succeed");

    // then: it is taken over as claimed before the moment given
    assert!(
        secrets
            .take_over(
                &an_address("ada@example.com"),
                &abandoned,
                &taking,
                clock::at(2_000),
                clock::at(3_000)
            )
            .await
            .expect("taking over should succeed")
    );
    // and: the one taking it over holds it
    assert_eq!(
        secrets
            .holder(&an_address("ada@example.com"))
            .await
            .expect("reading should succeed"),
        Some(taking)
    );
}

pub async fn a_claim_made_since_the_moment_given_is_kept<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: an address claimed by an account
    let recent = AccountId::generate(clock::at(1_000));
    secrets
        .claim(&an_address("ada@example.com"), &recent, clock::at(1_000))
        .await
        .expect("claiming should succeed");

    // then: it is not taken over as claimed before a moment no later than its claim
    for claimed_before in [clock::at(1_000), clock::at(500)] {
        assert!(
            !secrets
                .take_over(
                    &an_address("ada@example.com"),
                    &recent,
                    &AccountId::generate(clock::at(3_000)),
                    claimed_before,
                    clock::at(3_000),
                )
                .await
                .expect("taking over should succeed"),
            "a sign-up may still be under way"
        );
    }
    // and: its holder still holds it
    assert_eq!(
        secrets
            .holder(&an_address("ada@example.com"))
            .await
            .expect("reading should succeed"),
        Some(recent)
    );
}

pub async fn a_claim_is_taken_over_only_from_the_holder_named<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: an address claimed by an account
    let holder = AccountId::generate(clock::at(1_000));
    secrets
        .claim(&an_address("ada@example.com"), &holder, clock::at(1_000))
        .await
        .expect("claiming should succeed");

    // then: it is not taken over from another account
    assert!(
        !secrets
            .take_over(
                &an_address("ada@example.com"),
                &AccountId::generate(clock::at(1_500)),
                &AccountId::generate(clock::at(3_000)),
                clock::at(2_000),
                clock::at(3_000),
            )
            .await
            .expect("taking over should succeed")
    );
    // and: an address nobody claimed is not taken over
    assert!(
        !secrets
            .take_over(
                &an_address("nobody@example.com"),
                &holder,
                &AccountId::generate(clock::at(3_000)),
                clock::at(2_000),
                clock::at(3_000),
            )
            .await
            .expect("an address nobody claimed is not taken over"),
    );
    // and: the holder still holds it
    assert_eq!(
        secrets
            .holder(&an_address("ada@example.com"))
            .await
            .expect("reading should succeed"),
        Some(holder)
    );
}

pub async fn a_claim_taken_over_is_new_from_then<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: an address taken over
    let abandoned = AccountId::generate(clock::at(1_000));
    let taking = AccountId::generate(clock::at(3_000));
    secrets
        .claim(&an_address("ada@example.com"), &abandoned, clock::at(1_000))
        .await
        .expect("claiming should succeed");
    secrets
        .take_over(
            &an_address("ada@example.com"),
            &abandoned,
            &taking,
            clock::at(2_000),
            clock::at(3_000),
        )
        .await
        .expect("taking over should succeed");

    // then: it is not taken over again as claimed before it was taken over
    assert!(
        !secrets
            .take_over(
                &an_address("ada@example.com"),
                &taking,
                &AccountId::generate(clock::at(4_000)),
                clock::at(2_500),
                clock::at(4_000),
            )
            .await
            .expect("taking over should succeed"),
        "the one that took it over is under way from the moment it did"
    );
}

pub async fn a_claim_taken_over_by_many_at_once_goes_to_one<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: an address claimed by an account
    let abandoned = AccountId::generate(clock::at(1_000));
    secrets
        .claim(&an_address("ada@example.com"), &abandoned, clock::at(1_000))
        .await
        .expect("claiming should succeed");
    // and: many who would take it over
    let takers: Vec<AccountId> = (0..8)
        .map(|n| AccountId::generate(clock::at(3_000 + n)))
        .collect();
    let address = an_address("ada@example.com");

    // when: they take it over at once
    let taken = future::join_all(takers.iter().map(|taker| {
        secrets.take_over(
            &address,
            &abandoned,
            taker,
            clock::at(2_000),
            clock::at(3_000),
        )
    }))
    .await;

    // then: one of them gets it
    assert_eq!(
        taken
            .into_iter()
            .map(|taken| taken.expect("taking over should succeed"))
            .filter(|took| *took)
            .count(),
        1
    );
}

pub async fn a_dropped_ticket_leaves_nothing_behind_open_ended_or_marked<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: the tickets of two invitations, one of them taken
    secrets
        .keep_ticket(
            &a_digest("ada's"),
            &a_ticket("invitation_ada", "ada@example.com"),
        )
        .await
        .expect("keeping should succeed");
    secrets
        .keep_ticket(
            &a_digest("bob's"),
            &a_ticket("invitation_bob", "bob@example.com"),
        )
        .await
        .expect("keeping should succeed");
    secrets
        .take_ticket(&a_digest("bob's"))
        .await
        .expect("taking should succeed");
    // and: an invitation forgotten before its ticket came
    secrets
        .forget_ticket("invitation_eve")
        .await
        .expect("forgetting should succeed");
    // and: the ticket of another invitation
    secrets
        .keep_ticket(
            &a_digest("grace's"),
            &a_ticket("invitation_grace", "grace@example.com"),
        )
        .await
        .expect("keeping should succeed");

    // when: every invitation but the last is dropped, and one never kept
    for invitation in [
        "invitation_ada",
        "invitation_bob",
        "invitation_eve",
        "invitation_never",
    ] {
        secrets
            .drop_ticket(invitation)
            .await
            .expect("dropping should succeed");
    }

    // then: the dropped open ticket is not taken
    assert_eq!(
        secrets
            .take_ticket(&a_digest("ada's"))
            .await
            .expect("taking should succeed"),
        None
    );
    // and: each dropped invitation is kept afresh
    for (digest, invitation, to) in [
        ("ada's", "invitation_ada", "ada@example.com"),
        ("bob's", "invitation_bob", "bob@example.com"),
        ("eve's", "invitation_eve", "eve@example.com"),
    ] {
        assert!(
            secrets
                .keep_ticket(&a_digest(digest), &a_ticket(invitation, to))
                .await
                .expect("keeping should succeed"),
            "nothing of {invitation} is left, its mark neither"
        );
    }
    // and: the other ticket stays
    assert!(
        secrets
            .take_ticket(&a_digest("grace's"))
            .await
            .expect("taking should succeed")
            .is_some(),
        "the others stay"
    );
}

pub async fn a_ticket_kept_in_a_dropped_transaction_was_never_kept<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: a ticket kept in a transaction
    let mut transaction = secrets.begun().await.expect("beginning should succeed");
    secrets
        .store
        .keep_ticket(
            &mut transaction,
            &a_digest("ada's"),
            &a_ticket("invitation_ada", "ada@example.com"),
        )
        .await
        .expect("keeping should succeed");

    // when: the transaction is dropped
    drop(transaction);

    // then: it is not found
    assert_eq!(
        secrets
            .find_ticket(&a_digest("ada's"))
            .await
            .expect("finding should succeed"),
        None
    );
    // and: it is kept afresh
    assert!(
        secrets
            .keep_ticket(
                &a_digest("ada's"),
                &a_ticket("invitation_ada", "ada@example.com")
            )
            .await
            .expect("keeping should succeed"),
        "kept afresh, as a redelivery after a failure must be"
    );
}

pub async fn a_ticket_taken_in_a_dropped_transaction_is_still_open<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: a kept ticket
    secrets
        .keep_ticket(
            &a_digest("ada's"),
            &a_ticket("invitation_ada", "ada@example.com"),
        )
        .await
        .expect("keeping should succeed");
    // and: taken in a transaction
    let mut transaction = secrets.begun().await.expect("beginning should succeed");
    secrets
        .store
        .take_ticket(&mut transaction, &a_digest("ada's"))
        .await
        .expect("taking should succeed")
        .expect("the ticket is open");

    // when: the transaction is dropped
    drop(transaction);

    // then: it is still taken
    assert_eq!(
        secrets
            .take_ticket(&a_digest("ada's"))
            .await
            .expect("taking should succeed")
            .map(|ticket| ticket.invitation),
        Some("invitation_ada".to_owned()),
        "a take rolled back leaves the ticket to be taken again"
    );
}

pub async fn a_ticket_forgotten_or_dropped_in_a_dropped_transaction_is_still_there<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: a kept ticket
    secrets
        .keep_ticket(
            &a_digest("ada's"),
            &a_ticket("invitation_ada", "ada@example.com"),
        )
        .await
        .expect("keeping should succeed");
    // and: forgotten in a transaction that is dropped
    let mut forgetting = secrets.begun().await.expect("beginning should succeed");
    secrets
        .store
        .forget_ticket(&mut forgetting, "invitation_ada")
        .await
        .expect("forgetting should succeed");
    drop(forgetting);
    // and: dropped in another transaction
    let mut dropping = secrets.begun().await.expect("beginning should succeed");
    secrets
        .store
        .drop_ticket(&mut dropping, "invitation_ada")
        .await
        .expect("dropping should succeed");

    // when: that transaction is dropped
    drop(dropping);

    // then: the ticket is still found by its invitation
    assert_eq!(
        secrets
            .digest_of_ticket("invitation_ada")
            .await
            .expect("finding should succeed"),
        Some(a_digest("ada's"))
    );
}

pub async fn a_used_ticket_dropped_in_a_dropped_transaction_stays_used<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: a ticket kept and taken
    secrets
        .keep_ticket(
            &a_digest("ada's"),
            &a_ticket("invitation_ada", "ada@example.com"),
        )
        .await
        .expect("keeping should succeed");
    secrets
        .take_ticket(&a_digest("ada's"))
        .await
        .expect("taking should succeed")
        .expect("the ticket is open");
    // and: dropped in a transaction
    let mut transaction = secrets.begun().await.expect("beginning should succeed");
    secrets
        .store
        .drop_ticket(&mut transaction, "invitation_ada")
        .await
        .expect("dropping should succeed");

    // when: the transaction is dropped
    drop(transaction);

    // then: it stays used
    assert_eq!(
        secrets
            .take_ticket(&a_digest("ada's"))
            .await
            .expect("taking should succeed"),
        None,
        "a drop rolled back leaves a used ticket used"
    );
}

pub async fn an_address_claimed_or_taken_over_in_a_dropped_transaction_is_held_as_before<
    S: Secrets,
>(
    secrets: &Committing<'_, S>,
) {
    // given: an address claimed in a transaction that is dropped
    let (ada, bob) = (
        AccountId::generate(clock::at(1_000)),
        AccountId::generate(clock::at(1_001)),
    );
    let mut claiming = secrets.begun().await.expect("beginning should succeed");
    secrets
        .store
        .claim(
            &mut claiming,
            &an_address("ada@example.com"),
            &ada,
            clock::at(1_000),
        )
        .await
        .expect("claiming should succeed");
    drop(claiming);
    let unclaimed = secrets
        .holder(&an_address("ada@example.com"))
        .await
        .expect("reading should succeed");
    // and: claimed for good
    secrets
        .claim(&an_address("ada@example.com"), &ada, clock::at(1_000))
        .await
        .expect("claiming should succeed");
    // and: taken over in another transaction
    let mut taking_over = secrets.begun().await.expect("beginning should succeed");
    let taken = secrets
        .store
        .take_over(
            &mut taking_over,
            &an_address("ada@example.com"),
            &ada,
            &bob,
            clock::at(2_000),
            clock::at(2_000),
        )
        .await
        .expect("taking over should succeed");

    // when: that transaction is dropped
    drop(taking_over);

    // then: the dropped claim held nothing
    assert_eq!(unclaimed, None);
    // and: the take over went through inside its transaction
    assert!(taken);
    // and: the address is held as before
    assert_eq!(
        secrets
            .holder(&an_address("ada@example.com"))
            .await
            .expect("reading should succeed"),
        Some(ada),
        "a take-over rolled back leaves the claim with whoever held it"
    );
}

pub async fn an_address_released_or_a_password_kept_in_a_dropped_transaction_is_as_before<
    S: Secrets,
>(
    secrets: &Committing<'_, S>,
) {
    // given: an address claimed and a password kept
    let ada = AccountId::generate(clock::at(1_000));
    secrets
        .claim(&an_address("ada@example.com"), &ada, clock::at(1_000))
        .await
        .expect("claiming should succeed");
    secrets
        .keep(&ada, &a_hash("$argon2id$first"))
        .await
        .expect("keeping should succeed");
    // and: the address released in a transaction that is dropped
    let mut releasing = secrets.begun().await.expect("beginning should succeed");
    secrets
        .store
        .release(&mut releasing, &an_address("ada@example.com"), &ada)
        .await
        .expect("releasing should succeed");
    drop(releasing);
    // and: a new password kept in another transaction
    let mut keeping = secrets.begun().await.expect("beginning should succeed");
    secrets
        .store
        .keep(&mut keeping, &ada, &a_hash("$argon2id$second"))
        .await
        .expect("keeping should succeed");

    // when: that transaction is dropped
    drop(keeping);

    // then: the address is held as before
    assert_eq!(
        secrets
            .holder(&an_address("ada@example.com"))
            .await
            .expect("reading should succeed"),
        Some(ada)
    );
    // and: the password is the first
    assert_eq!(
        secrets.of(&ada).await.expect("reading should succeed"),
        Some(a_hash("$argon2id$first"))
    );
}

pub async fn a_first_password_kept_in_a_dropped_transaction_was_never_kept<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: a first password kept in a transaction
    let ada = AccountId::generate(clock::at(1_000));
    let mut keeping = secrets.begun().await.expect("beginning should succeed");
    secrets
        .store
        .keep(&mut keeping, &ada, &a_hash("$argon2id$first"))
        .await
        .expect("keeping should succeed");

    // when: the transaction is dropped
    drop(keeping);

    // then: the account has no password
    assert_eq!(
        secrets.of(&ada).await.expect("reading should succeed"),
        None
    );
}

pub async fn a_code_kept_or_taken_in_a_transaction_that_is_dropped_is_as_it_was<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: a kept code
    secrets
        .keep_code(&TokenDigest::kept("kept".to_owned()))
        .await
        .expect("keeping a code should succeed");
    // and: in a transaction it taken and another kept
    let mut transaction = secrets.begun().await.expect("beginning should succeed");
    let store = secrets.store;
    store
        .take_code(&mut transaction, &TokenDigest::kept("kept".to_owned()))
        .await
        .expect("taking a code should succeed");
    store
        .keep_code(&mut transaction, &TokenDigest::kept("another".to_owned()))
        .await
        .expect("keeping a code should succeed");

    // when: the transaction is dropped
    drop(transaction);

    // then: the first is still held
    assert!(
        secrets
            .holds_code(&TokenDigest::kept("kept".to_owned()))
            .await
            .expect("reading should succeed")
    );
    // and: the other is not
    assert!(
        !secrets
            .holds_code(&TokenDigest::kept("another".to_owned()))
            .await
            .expect("reading should succeed")
    );
}

pub async fn sessions_codes_and_operators_written_in_a_transaction_that_is_dropped_are_gone<
    S: Secrets,
>(
    secrets: &Committing<'_, S>,
) {
    // given: an account with an issued code and an open session, and two operators
    let ada = AccountId::generate(clock::at(1_000));
    let grace = an_operator_appointed_at(secrets, 2_000).await;
    an_operator_appointed_at(secrets, 3_000).await;
    secrets
        .issue(
            &ada,
            CodePurpose::ConfirmAddress,
            &a_pending_code("004217", 1_900),
        )
        .await
        .expect("issuing should succeed");
    let kept_session = SessionToken::drawn().digest();
    secrets
        .open(&kept_session, &a_session(Stage::SignedIn))
        .await
        .expect("opening should succeed");
    // and: in a transaction a session opened, the open one ended, the code attempted and closed, another code issued, the account appointed and an operator dismissed
    let dropped_session = SessionToken::drawn().digest();
    let mut transaction = secrets.begun().await.expect("beginning should succeed");
    let store = secrets.store;
    store
        .open(
            &mut transaction,
            &dropped_session,
            &a_session(Stage::SignedIn),
        )
        .await
        .expect("opening should succeed");
    store
        .end(&mut transaction, &kept_session)
        .await
        .expect("ending should succeed");
    store
        .attempt(&mut transaction, &ada, CodePurpose::ConfirmAddress, 5)
        .await
        .expect("attempting should succeed");
    store
        .issue(
            &mut transaction,
            &ada,
            CodePurpose::ResetPassword,
            &a_pending_code("777777", 1_900),
        )
        .await
        .expect("issuing should succeed");
    store
        .close(
            &mut transaction,
            &ada,
            CodePurpose::ConfirmAddress,
            &CodeDigest::kept("004217".to_owned()),
        )
        .await
        .expect("closing should succeed");
    store
        .appoint(&mut transaction, &ada)
        .await
        .expect("appointing should succeed");
    store
        .dismiss(&mut transaction, &grace)
        .await
        .expect("dismissing should succeed");

    // when: the transaction is dropped
    drop(transaction);

    // then: the session opened in it is not found
    assert!(
        secrets
            .find(&dropped_session)
            .await
            .expect("finding should succeed")
            .is_none()
    );
    // and: the one ended in it is still open
    assert!(
        secrets
            .find(&kept_session)
            .await
            .expect("finding should succeed")
            .is_some()
    );
    // and: the code is still open and unattempted
    assert_eq!(
        secrets
            .attempt(&ada, CodePurpose::ConfirmAddress, 5)
            .await
            .expect("attempting should succeed"),
        Attempt::Counted(PendingVerification {
            tried: 1,
            ..a_pending_code("004217", 1_900)
        }),
        "the code is still open and the dropped attempt never counted"
    );
    // and: no code issued in it is open
    assert_eq!(
        secrets
            .attempt(&ada, CodePurpose::ResetPassword, 5)
            .await
            .expect("attempting should succeed"),
        Attempt::Nothing
    );
    // and: nobody was appointed or dismissed in it
    assert_eq!(
        (
            secrets
                .dismiss(&ada)
                .await
                .expect("dismissing should succeed"),
            secrets
                .dismiss(&grace)
                .await
                .expect("dismissing should succeed"),
        ),
        (Dismissal::NotAppointed, Dismissal::Dismissed),
        "Ada was never appointed and Grace never dismissed"
    );
}

#[macro_export]
macro_rules! secrets_conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::suite::$case(&bench.committing()).await;
            bench.cleanup().await;
        }
    };
}

#[macro_export]
macro_rules! conformance_tests {
    ($workbench:ty) => {
        $crate::secrets_conformance_case!(
            $workbench,
            sessions_codes_and_operators_written_in_a_transaction_that_is_dropped_are_gone
        );
        $crate::secrets_conformance_case!($workbench, a_kept_ticket_is_taken_once_and_whole);
        $crate::secrets_conformance_case!($workbench, no_code_is_kept_once_someone_is_an_operator);
        $crate::secrets_conformance_case!(
            $workbench,
            a_code_kept_before_someone_became_an_operator_is_taken_no_more
        );
        $crate::secrets_conformance_case!($workbench, an_operator_is_appointed_until_dismissed);
        $crate::secrets_conformance_case!(
            $workbench,
            a_code_kept_or_taken_in_a_transaction_that_is_dropped_is_as_it_was
        );
        $crate::secrets_conformance_case!(
            $workbench,
            an_address_claimed_or_taken_over_in_a_dropped_transaction_is_held_as_before
        );
        $crate::secrets_conformance_case!(
            $workbench,
            an_address_released_or_a_password_kept_in_a_dropped_transaction_is_as_before
        );
        $crate::secrets_conformance_case!(
            $workbench,
            a_first_password_kept_in_a_dropped_transaction_was_never_kept
        );
        $crate::secrets_conformance_case!(
            $workbench,
            a_used_ticket_dropped_in_a_dropped_transaction_stays_used
        );
        $crate::secrets_conformance_case!(
            $workbench,
            a_ticket_kept_in_a_dropped_transaction_was_never_kept
        );
        $crate::secrets_conformance_case!(
            $workbench,
            a_ticket_taken_in_a_dropped_transaction_is_still_open
        );
        $crate::secrets_conformance_case!(
            $workbench,
            a_ticket_forgotten_or_dropped_in_a_dropped_transaction_is_still_there
        );
        $crate::secrets_conformance_case!($workbench, a_digest_nobody_was_given_takes_no_ticket);
        $crate::secrets_conformance_case!(
            $workbench,
            a_ticket_kept_again_is_kept_once_as_it_first_was
        );
        $crate::secrets_conformance_case!(
            $workbench,
            a_forgotten_invitation_leaves_no_ticket_and_the_others_kept
        );
        $crate::secrets_conformance_case!($workbench, a_ticket_taken_by_many_at_once_goes_to_one);
        $crate::secrets_conformance_case!(
            $workbench,
            an_invitation_kept_again_by_another_digest_keeps_its_first
        );
        $crate::secrets_conformance_case!(
            $workbench,
            a_dropped_ticket_leaves_nothing_behind_open_ended_or_marked
        );
        $crate::secrets_conformance_case!($workbench, a_ticket_taken_is_not_kept_again);
        $crate::secrets_conformance_case!(
            $workbench,
            a_project_s_ticket_is_kept_with_its_project_and_who_issued_it
        );
        $crate::secrets_conformance_case!($workbench, a_found_ticket_is_left_to_be_taken);
        $crate::secrets_conformance_case!(
            $workbench,
            a_ticket_still_open_is_found_by_its_invitation_to_take_it_up_signed_in
        );
        $crate::secrets_conformance_case!($workbench, a_ticket_forgotten_is_not_kept_again);
        $crate::secrets_conformance_case!(
            $workbench,
            an_invitation_forgotten_before_its_ticket_came_keeps_none
        );
        $crate::secrets_conformance_case!(
            $workbench,
            a_claim_made_before_the_moment_given_is_taken_over_from_its_holder
        );
        $crate::secrets_conformance_case!($workbench, a_claim_made_since_the_moment_given_is_kept);
        $crate::secrets_conformance_case!(
            $workbench,
            a_claim_is_taken_over_only_from_the_holder_named
        );
        $crate::secrets_conformance_case!($workbench, a_claim_taken_over_is_new_from_then);
        $crate::secrets_conformance_case!(
            $workbench,
            a_claim_taken_over_by_many_at_once_goes_to_one
        );
        $crate::secrets_conformance_case!(
            $workbench,
            a_kept_code_is_held_until_taken_and_no_other_is
        );
        $crate::secrets_conformance_case!($workbench, a_kept_code_is_taken_once);
        $crate::secrets_conformance_case!($workbench, only_the_latest_code_kept_can_be_taken);
        $crate::secrets_conformance_case!($workbench, no_code_kept_takes_nothing);
        $crate::secrets_conformance_case!($workbench, a_code_taken_by_many_at_once_goes_to_one);
        $crate::secrets_conformance_case!($workbench, nobody_is_an_operator_until_appointed);
        $crate::secrets_conformance_case!($workbench, appointing_twice_is_appointing_once);
        $crate::secrets_conformance_case!($workbench, one_of_two_operators_is_dismissed);
        $crate::secrets_conformance_case!($workbench, the_last_operator_is_kept);
        $crate::secrets_conformance_case!($workbench, someone_never_appointed_is_not_dismissed);
        $crate::secrets_conformance_case!($workbench, operators_dismissed_all_at_once_leave_one);
        $crate::secrets_conformance_case!($workbench, a_claimed_address_names_its_holder);
        $crate::secrets_conformance_case!($workbench, an_address_never_claimed_names_nobody);
        $crate::secrets_conformance_case!($workbench, an_address_is_held_by_one_account_only);
        $crate::secrets_conformance_case!($workbench, one_account_may_hold_two_addresses);
        $crate::secrets_conformance_case!($workbench, a_released_address_can_be_claimed_again);
        $crate::secrets_conformance_case!(
            $workbench,
            an_address_is_released_only_by_the_account_holding_it
        );
        $crate::secrets_conformance_case!(
            $workbench,
            a_kept_password_is_read_back_until_a_new_one_replaces_it
        );
        $crate::secrets_conformance_case!(
            $workbench,
            each_account_has_a_password_of_its_own_or_none
        );
        $crate::secrets_conformance_case!(
            $workbench,
            each_attempt_at_an_issued_code_is_counted_until_it_is_closed
        );
        $crate::secrets_conformance_case!(
            $workbench,
            a_code_is_closed_only_by_itself_and_only_once
        );
        $crate::secrets_conformance_case!(
            $workbench,
            attempts_beyond_the_limit_are_not_counted_but_exhausted
        );
        $crate::secrets_conformance_case!($workbench, attempts_made_at_once_are_counted_one_by_one);
        $crate::secrets_conformance_case!($workbench, a_new_code_starts_its_attempts_afresh);
        $crate::secrets_conformance_case!($workbench, each_account_has_a_code_of_its_own);
        $crate::secrets_conformance_case!(
            $workbench,
            attempting_or_closing_where_nothing_was_issued_is_harmless
        );
        $crate::secrets_conformance_case!(
            $workbench,
            an_opened_session_is_found_by_its_tokens_digest_until_it_ends
        );
        $crate::secrets_conformance_case!($workbench, a_token_nobody_was_given_finds_nothing);
        $crate::secrets_conformance_case!($workbench, each_stage_is_kept_as_it_was);
        $crate::secrets_conformance_case!($workbench, ending_one_session_leaves_the_others_open);
        $crate::secrets_conformance_case!(
            $workbench,
            a_code_for_one_purpose_is_no_code_for_another
        );
        $crate::secrets_conformance_case!(
            $workbench,
            a_totp_being_set_up_is_in_use_only_once_finished
        );
        $crate::secrets_conformance_case!(
            $workbench,
            only_the_latest_setup_is_finished_and_only_once
        );
        $crate::secrets_conformance_case!(
            $workbench,
            a_totp_set_up_anew_stays_in_use_until_the_new_one_is_finished
        );
        $crate::secrets_conformance_case!(
            $workbench,
            a_step_is_accepted_only_after_the_last_accepted
        );
        $crate::secrets_conformance_case!($workbench, a_step_accepted_by_many_at_once_goes_to_one);
        $crate::secrets_conformance_case!($workbench, no_step_is_accepted_without_a_totp_in_use);
        $crate::secrets_conformance_case!($workbench, recovery_codes_kept_replace_the_ones_before);
        $crate::secrets_conformance_case!($workbench, a_recovery_code_is_used_once);
        $crate::secrets_conformance_case!(
            $workbench,
            a_recovery_code_used_by_many_at_once_goes_to_one
        );
        $crate::secrets_conformance_case!(
            $workbench,
            forgetting_a_totp_takes_its_setup_along_and_leaves_the_codes
        );
        $crate::secrets_conformance_case!(
            $workbench,
            totp_writes_in_a_dropped_transaction_leave_it_as_it_was
        );
        $crate::secrets_conformance_case!(
            $workbench,
            a_first_setup_begun_in_a_dropped_transaction_was_never_begun
        );
        $crate::secrets_conformance_case!(
            $workbench,
            an_accounts_credentials_are_its_own_the_oldest_key_first
        );
        $crate::secrets_conformance_case!(
            $workbench,
            no_credential_is_held_by_two_keys_of_any_accounts
        );
        $crate::secrets_conformance_case!(
            $workbench,
            a_credential_offered_by_many_at_once_is_kept_once
        );
        $crate::secrets_conformance_case!(
            $workbench,
            a_renewed_credential_reads_as_renewed_and_only_where_it_is_held
        );
        $crate::secrets_conformance_case!(
            $workbench,
            a_forgotten_credential_is_gone_and_may_be_kept_again
        );
        $crate::secrets_conformance_case!(
            $workbench,
            credential_writes_in_a_dropped_transaction_leave_them_as_they_were
        );
        $crate::secrets_conformance_case!(
            $workbench,
            the_way_that_answered_last_is_noted_for_each_account_in_place_of_the_one_before
        );
        $crate::secrets_conformance_case!(
            $workbench,
            a_forgotten_account_holds_no_secret_and_others_keep_theirs
        );
        $crate::secrets_conformance_case!(
            $workbench,
            sessions_and_codes_run_out_are_forgotten_oldest_first_and_the_rest_kept
        );
    };
}
