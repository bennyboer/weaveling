use accounts_core::{
    AccountId, KeptTotp, RecoveryCodeHash, SealedTotpSecret, SecretsError, TotpStep,
};
use futures_util::future;
use transacting::Tried;

use super::{Committing, Secrets, unreachable};

impl<S: Secrets> Committing<'_, S> {
    pub async fn begin_setting_up(
        &self,
        account: &AccountId,
        secret: &SealedTotpSecret,
    ) -> Result<(), SecretsError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.store
                .begin_setting_up(transaction, account, secret)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn finish_setting_up(
        &self,
        account: &AccountId,
        being_set_up: &SealedTotpSecret,
        accepted: TotpStep,
    ) -> Result<bool, SecretsError> {
        transacting::retrying!(self.transactions, |transaction| {
            let finished = self
                .store
                .finish_setting_up(transaction, account, being_set_up, accepted)
                .await
                .map_err(Tried::Refused)?;

            Ok(finished)
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn accept_step(
        &self,
        account: &AccountId,
        step: TotpStep,
    ) -> Result<bool, SecretsError> {
        transacting::retrying!(self.transactions, |transaction| {
            let accepted = self
                .store
                .accept_step(transaction, account, step)
                .await
                .map_err(Tried::Refused)?;

            Ok(accepted)
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn keep_recovery_codes(
        &self,
        account: &AccountId,
        codes: &[RecoveryCodeHash],
    ) -> Result<(), SecretsError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.store
                .keep_recovery_codes(transaction, account, codes)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn use_recovery_code(
        &self,
        account: &AccountId,
        code: &RecoveryCodeHash,
    ) -> Result<bool, SecretsError> {
        transacting::retrying!(self.transactions, |transaction| {
            let used = self
                .store
                .use_recovery_code(transaction, account, code)
                .await
                .map_err(Tried::Refused)?;

            Ok(used)
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn forget_totp(&self, account: &AccountId) -> Result<(), SecretsError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.store
                .forget_totp(transaction, account)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    async fn codes_of(&self, account: &AccountId) -> Vec<String> {
        let mut codes: Vec<String> = self
            .store
            .recovery_codes_of(account)
            .await
            .expect("reading should succeed")
            .iter()
            .map(|code| code.as_str().to_owned())
            .collect();
        codes.sort();

        codes
    }

    async fn set_up(&self, account: &AccountId, secret: &SealedTotpSecret, step: u64) {
        self.begin_setting_up(account, secret)
            .await
            .expect("setting up should succeed");
        assert!(
            self.finish_setting_up(account, secret, TotpStep::kept(step))
                .await
                .expect("finishing should succeed")
        );
    }
}

fn an_account(at: i64) -> AccountId {
    AccountId::generate(clock::at(at))
}

fn sealed(bytes: &[u8]) -> SealedTotpSecret {
    SealedTotpSecret::kept(bytes.to_vec())
}

fn hashes(codes: &[&str]) -> Vec<RecoveryCodeHash> {
    codes
        .iter()
        .map(|code| RecoveryCodeHash::kept((*code).to_owned()))
        .collect()
}

pub async fn a_totp_being_set_up_is_in_use_only_once_finished<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: a second factor being set up for an account
    let ada = an_account(1_000);
    secrets
        .begin_setting_up(&ada, &sealed(b"first"))
        .await
        .expect("setting up should succeed");

    // then: it is being set up, and the account has none in use yet
    assert_eq!(
        secrets
            .totp_being_set_up(&ada)
            .await
            .expect("reading should succeed"),
        Some(sealed(b"first"))
    );
    assert_eq!(
        secrets.totp_of(&ada).await.expect("reading should succeed"),
        None
    );
    // when: setting it up is finished by a code of one step
    let finished = secrets
        .finish_setting_up(&ada, &sealed(b"first"), TotpStep::kept(100))
        .await
        .expect("finishing should succeed");
    // then: it is in use, that step its last accepted
    assert!(finished);
    assert_eq!(
        secrets.totp_of(&ada).await.expect("reading should succeed"),
        Some(KeptTotp {
            secret: sealed(b"first"),
            last_accepted: TotpStep::kept(100),
        })
    );
    // and: nothing is being set up any more
    assert_eq!(
        secrets
            .totp_being_set_up(&ada)
            .await
            .expect("reading should succeed"),
        None
    );
}

pub async fn only_the_latest_setup_is_finished_and_only_once<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: two second factors set up one after the other
    let ada = an_account(1_000);
    for begun in [b"first", b"later"] {
        secrets
            .begin_setting_up(&ada, &sealed(begun))
            .await
            .expect("setting up should succeed");
    }

    // expect: finishing the first finishes nothing
    assert!(
        !secrets
            .finish_setting_up(&ada, &sealed(b"first"), TotpStep::kept(100))
            .await
            .expect("finishing should succeed"),
        "a code from a page left open must not finish a setup no longer kept"
    );
    // and: the later is finished once
    assert!(
        secrets
            .finish_setting_up(&ada, &sealed(b"later"), TotpStep::kept(100))
            .await
            .expect("finishing should succeed")
    );
    assert!(
        !secrets
            .finish_setting_up(&ada, &sealed(b"later"), TotpStep::kept(101))
            .await
            .expect("finishing should succeed")
    );
    // and: finishing where nothing was ever begun finishes nothing
    assert!(
        !secrets
            .finish_setting_up(&an_account(2_000), &sealed(b"later"), TotpStep::kept(100))
            .await
            .expect("finishing should succeed")
    );
}

pub async fn a_totp_set_up_anew_stays_in_use_until_the_new_one_is_finished<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: an account with a second factor
    let ada = an_account(1_000);
    secrets.set_up(&ada, &sealed(b"first"), 100).await;

    // when: a new one is being set up
    secrets
        .begin_setting_up(&ada, &sealed(b"anew"))
        .await
        .expect("setting up should succeed");
    // then: the first is still in use
    assert_eq!(
        secrets
            .totp_of(&ada)
            .await
            .expect("reading should succeed")
            .map(|factor| factor.secret),
        Some(sealed(b"first"))
    );
    // when: setting up the new one is finished
    assert!(
        secrets
            .finish_setting_up(&ada, &sealed(b"anew"), TotpStep::kept(50))
            .await
            .expect("finishing should succeed")
    );
    // then: it is in use in place of the first, with its own step
    assert_eq!(
        secrets.totp_of(&ada).await.expect("reading should succeed"),
        Some(KeptTotp {
            secret: sealed(b"anew"),
            last_accepted: TotpStep::kept(50),
        })
    );
}

pub async fn a_step_is_accepted_only_after_the_last_accepted<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: an account whose second factor last accepted step 100
    let ada = an_account(1_000);
    secrets.set_up(&ada, &sealed(b"first"), 100).await;

    // expect: step 100 and the one before are refused
    for step in [100, 99] {
        assert!(
            !secrets
                .accept_step(&ada, TotpStep::kept(step))
                .await
                .expect("accepting should succeed"),
            "{step}"
        );
    }
    // and: step 101 is accepted, and becomes the last
    assert!(
        secrets
            .accept_step(&ada, TotpStep::kept(101))
            .await
            .expect("accepting should succeed")
    );
    assert_eq!(
        secrets
            .totp_of(&ada)
            .await
            .expect("reading should succeed")
            .map(|factor| factor.last_accepted),
        Some(TotpStep::kept(101))
    );
}

pub async fn a_step_accepted_by_many_at_once_goes_to_one<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: an account with a second factor
    let ada = an_account(1_000);
    secrets.set_up(&ada, &sealed(b"first"), 100).await;

    // when: many try the same later step at once
    let accepted =
        future::join_all((0..20).map(|_| secrets.accept_step(&ada, TotpStep::kept(101)))).await;

    // then: one of them is accepted
    assert_eq!(
        accepted
            .iter()
            .filter(|accepted| matches!(accepted, Ok(true)))
            .count(),
        1,
        "a code seen over someone's shoulder must not sign in a second time beside the first"
    );
}

pub async fn no_step_is_accepted_without_a_totp_in_use<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: an account with a second factor being set up only
    let ada = an_account(1_000);
    secrets
        .begin_setting_up(&ada, &sealed(b"first"))
        .await
        .expect("setting up should succeed");

    // expect: no step is accepted for it
    assert!(
        !secrets
            .accept_step(&ada, TotpStep::kept(101))
            .await
            .expect("accepting should succeed")
    );
    // and: none for an account with nothing at all
    assert!(
        !secrets
            .accept_step(&an_account(2_000), TotpStep::kept(101))
            .await
            .expect("accepting should succeed")
    );
}

pub async fn recovery_codes_kept_replace_the_ones_before<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: an account with two recovery codes
    let ada = an_account(1_000);
    secrets
        .keep_recovery_codes(&ada, &hashes(&["$argon2id$a", "$argon2id$b"]))
        .await
        .expect("keeping should succeed");
    assert_eq!(secrets.codes_of(&ada).await, ["$argon2id$a", "$argon2id$b"]);

    // when: codes are kept anew
    secrets
        .keep_recovery_codes(&ada, &hashes(&["$argon2id$c"]))
        .await
        .expect("keeping should succeed");

    // then: only the new ones are kept
    assert_eq!(secrets.codes_of(&ada).await, ["$argon2id$c"]);
}

pub async fn a_recovery_code_is_used_once<S: Secrets>(secrets: &Committing<'_, S>) {
    // given: an account with two recovery codes
    let ada = an_account(1_000);
    secrets
        .keep_recovery_codes(&ada, &hashes(&["$argon2id$a", "$argon2id$b"]))
        .await
        .expect("keeping should succeed");

    // when: one is used
    let used = secrets
        .use_recovery_code(&ada, &hashes(&["$argon2id$a"])[0])
        .await
        .expect("using should succeed");

    // then: it was used, and the other is left
    assert!(used);
    assert_eq!(secrets.codes_of(&ada).await, ["$argon2id$b"]);
    // and: using it again uses nothing
    assert!(
        !secrets
            .use_recovery_code(&ada, &hashes(&["$argon2id$a"])[0])
            .await
            .expect("using should succeed")
    );
    // and: another account cannot use the one left
    assert!(
        !secrets
            .use_recovery_code(&an_account(2_000), &hashes(&["$argon2id$b"])[0])
            .await
            .expect("using should succeed")
    );
}

pub async fn a_recovery_code_used_by_many_at_once_goes_to_one<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: an account with a recovery code
    let ada = an_account(1_000);
    secrets
        .keep_recovery_codes(&ada, &hashes(&["$argon2id$a"]))
        .await
        .expect("keeping should succeed");

    // when: many use it at once
    let code = &hashes(&["$argon2id$a"])[0];
    let used = future::join_all((0..20).map(|_| secrets.use_recovery_code(&ada, code))).await;

    // then: one of them uses it
    assert_eq!(
        used.iter().filter(|used| matches!(used, Ok(true))).count(),
        1,
        "a recovery code serves once"
    );
}

pub async fn forgetting_a_totp_takes_its_setup_and_codes_along<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: an account with a second factor, a new one being set up, and recovery codes
    let ada = an_account(1_000);
    secrets.set_up(&ada, &sealed(b"first"), 100).await;
    secrets
        .begin_setting_up(&ada, &sealed(b"anew"))
        .await
        .expect("setting up should succeed");
    secrets
        .keep_recovery_codes(&ada, &hashes(&["$argon2id$a"]))
        .await
        .expect("keeping should succeed");
    // and: another account with the same
    let grace = an_account(2_000);
    secrets.set_up(&grace, &sealed(b"hers"), 100).await;
    secrets
        .keep_recovery_codes(&grace, &hashes(&["$argon2id$g"]))
        .await
        .expect("keeping should succeed");

    // when: the first account's second factor is forgotten
    secrets
        .forget_totp(&ada)
        .await
        .expect("forgetting should succeed");

    // then: it has none in use, none being set up and no codes
    assert_eq!(
        secrets.totp_of(&ada).await.expect("reading should succeed"),
        None
    );
    assert_eq!(
        secrets
            .totp_being_set_up(&ada)
            .await
            .expect("reading should succeed"),
        None
    );
    assert!(secrets.codes_of(&ada).await.is_empty());
    // and: the other account keeps its own
    assert!(
        secrets
            .totp_of(&grace)
            .await
            .expect("reading should succeed")
            .is_some()
    );
    assert_eq!(secrets.codes_of(&grace).await, ["$argon2id$g"]);
}

pub async fn totp_writes_in_a_dropped_transaction_leave_it_as_it_was<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: an account with a second factor, a new one being set up, and recovery codes
    let ada = an_account(1_000);
    secrets.set_up(&ada, &sealed(b"first"), 100).await;
    secrets
        .begin_setting_up(&ada, &sealed(b"anew"))
        .await
        .expect("setting up should succeed");
    secrets
        .keep_recovery_codes(&ada, &hashes(&["$argon2id$a", "$argon2id$b"]))
        .await
        .expect("keeping should succeed");

    // when: every kind of write is made in a transaction that is dropped
    let mut writing = secrets.begun().await.expect("beginning should succeed");
    assert!(
        secrets
            .store
            .accept_step(&mut writing, &ada, TotpStep::kept(101))
            .await
            .expect("accepting should succeed")
    );
    assert!(
        secrets
            .store
            .use_recovery_code(&mut writing, &ada, &hashes(&["$argon2id$a"])[0])
            .await
            .expect("using should succeed")
    );
    assert!(
        secrets
            .store
            .finish_setting_up(&mut writing, &ada, &sealed(b"anew"), TotpStep::kept(7))
            .await
            .expect("finishing should succeed")
    );
    secrets
        .store
        .keep_recovery_codes(&mut writing, &ada, &hashes(&["$argon2id$c"]))
        .await
        .expect("keeping should succeed");
    secrets
        .store
        .forget_totp(&mut writing, &ada)
        .await
        .expect("forgetting should succeed");
    drop(writing);

    // then: the second factor in use, the one being set up and the codes are as they were
    assert_eq!(
        secrets.totp_of(&ada).await.expect("reading should succeed"),
        Some(KeptTotp {
            secret: sealed(b"first"),
            last_accepted: TotpStep::kept(100),
        })
    );
    assert_eq!(
        secrets
            .totp_being_set_up(&ada)
            .await
            .expect("reading should succeed"),
        Some(sealed(b"anew"))
    );
    assert_eq!(secrets.codes_of(&ada).await, ["$argon2id$a", "$argon2id$b"]);
}

pub async fn a_first_setup_begun_in_a_dropped_transaction_was_never_begun<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: a second factor begun and codes kept for an account with none, in a transaction
    let ada = an_account(1_000);
    let mut writing = secrets.begun().await.expect("beginning should succeed");
    secrets
        .store
        .begin_setting_up(&mut writing, &ada, &sealed(b"first"))
        .await
        .expect("setting up should succeed");
    secrets
        .store
        .keep_recovery_codes(&mut writing, &ada, &hashes(&["$argon2id$a"]))
        .await
        .expect("keeping should succeed");

    // when: the transaction is dropped
    drop(writing);

    // then: nothing is being set up and no code kept
    assert_eq!(
        secrets
            .totp_being_set_up(&ada)
            .await
            .expect("reading should succeed"),
        None
    );
    assert!(secrets.codes_of(&ada).await.is_empty());
}
