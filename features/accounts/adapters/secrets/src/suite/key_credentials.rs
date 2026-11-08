use accounts_core::{AccountId, KeyCredential, KeyId, SecretsError};
use futures_util::future;
use transacting::Tried;

use super::{Committing, Secrets, unreachable};

impl<S: Secrets> Committing<'_, S> {
    pub async fn keep_credential(
        &self,
        account: &AccountId,
        key: &KeyId,
        credential: &KeyCredential,
    ) -> Result<bool, SecretsError> {
        transacting::retrying!(self.transactions, |transaction| {
            let kept = self
                .store
                .keep_credential(transaction, account, key, credential)
                .await
                .map_err(Tried::Refused)?;

            Ok(kept)
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn renew_credential(
        &self,
        account: &AccountId,
        key: &KeyId,
        written: &str,
    ) -> Result<bool, SecretsError> {
        transacting::retrying!(self.transactions, |transaction| {
            let renewed = self
                .store
                .renew_credential(transaction, account, key, written)
                .await
                .map_err(Tried::Refused)?;

            Ok(renewed)
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn forget_credential(
        &self,
        account: &AccountId,
        key: &KeyId,
    ) -> Result<(), SecretsError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.store
                .forget_credential(transaction, account, key)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    async fn held_by(&self, account: &AccountId) -> Vec<(KeyId, KeyCredential)> {
        self.store
            .credentials_of(account)
            .await
            .expect("reading should succeed")
    }

    async fn kept(&self, account: &AccountId, key: &KeyId, credential: &KeyCredential) {
        assert!(
            self.keep_credential(account, key, credential)
                .await
                .expect("keeping should succeed"),
            "a credential nobody holds is kept"
        );
    }
}

fn an_account(at: i64) -> AccountId {
    AccountId::generate(clock::at(at))
}

fn a_key(at: i64) -> KeyId {
    KeyId::generate(clock::at(at))
}

fn credential(id: &[u8], written: &str) -> KeyCredential {
    KeyCredential {
        id: id.to_vec(),
        written: written.to_owned(),
    }
}

pub async fn an_accounts_credentials_are_its_own_the_oldest_key_first<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: two accounts, one holding two keys added in turn, the other one
    let ada = an_account(1_000);
    let grace = an_account(1_001);
    let older = a_key(2_000);
    let newer = a_key(3_000);
    let hers = a_key(2_500);
    secrets
        .kept(&ada, &newer, &credential(b"newer", r#"{"counter":0}"#))
        .await;
    secrets
        .kept(&ada, &older, &credential(b"older", r#"{"counter":4}"#))
        .await;
    secrets
        .kept(&grace, &hers, &credential(b"hers", r#"{"counter":1}"#))
        .await;

    // when: the first account's credentials are read
    let held = secrets.held_by(&ada).await;

    // then: they are its two, the older key first, written as they were given
    assert_eq!(
        held,
        vec![
            (older, credential(b"older", r#"{"counter":4}"#)),
            (newer, credential(b"newer", r#"{"counter":0}"#)),
        ]
    );
    // and: an account without keys holds none
    assert!(secrets.held_by(&an_account(1_002)).await.is_empty());
}

pub async fn no_credential_is_held_by_two_keys_of_any_accounts<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: a credential kept for a key of one account
    let ada = an_account(1_000);
    let grace = an_account(1_001);
    let first = a_key(2_000);
    secrets
        .kept(&ada, &first, &credential(b"shared", "first"))
        .await;

    // when: the same credential is offered for another key of that account, and of another
    let again_hers = secrets
        .keep_credential(&ada, &a_key(2_001), &credential(b"shared", "again"))
        .await
        .expect("keeping should succeed");
    let again_others = secrets
        .keep_credential(&grace, &a_key(2_002), &credential(b"shared", "other"))
        .await
        .expect("keeping should succeed");

    // then: neither is kept, and the first stays as it was
    assert!(!again_hers && !again_others);
    assert_eq!(
        secrets.held_by(&ada).await,
        vec![(first, credential(b"shared", "first"))]
    );
    assert!(secrets.held_by(&grace).await.is_empty());
}

pub async fn a_credential_offered_by_many_at_once_is_kept_once<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: one credential offered for ten keys of ten accounts at once
    let offers: Vec<(AccountId, KeyId)> = (0..10)
        .map(|n| (an_account(1_000 + n), a_key(2_000 + n)))
        .collect();

    let contested = credential(b"contested", "written");

    // when: they are all kept together
    let kept = future::join_all(
        offers
            .iter()
            .map(|(account, key)| secrets.keep_credential(account, key, &contested)),
    )
    .await;

    // then: exactly one is kept
    assert_eq!(
        kept.into_iter()
            .map(|kept| kept.expect("keeping should succeed"))
            .filter(|kept| *kept)
            .count(),
        1
    );
}

pub async fn a_renewed_credential_reads_as_renewed_and_only_where_it_is_held<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: a credential kept for a key
    let ada = an_account(1_000);
    let key = a_key(2_000);
    secrets
        .kept(&ada, &key, &credential(b"key", r#"{"counter":1}"#))
        .await;

    // when: it is written anew after a use
    let renewed = secrets
        .renew_credential(&ada, &key, r#"{"counter":2}"#)
        .await
        .expect("renewing should succeed");

    // then: it reads as renewed, under the same id
    assert!(renewed);
    assert_eq!(
        secrets.held_by(&ada).await,
        vec![(key, credential(b"key", r#"{"counter":2}"#))]
    );
    // and: renewing a key the account does not hold changes nothing, nor does another account's
    assert!(
        !secrets
            .renew_credential(&ada, &a_key(2_001), "{}")
            .await
            .expect("renewing should succeed")
    );
    assert!(
        !secrets
            .renew_credential(&an_account(1_001), &key, "{}")
            .await
            .expect("renewing should succeed")
    );
    assert_eq!(
        secrets.held_by(&ada).await,
        vec![(key, credential(b"key", r#"{"counter":2}"#))]
    );
}

pub async fn a_forgotten_credential_is_gone_and_may_be_kept_again<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: an account holding two keys
    let ada = an_account(1_000);
    let lost = a_key(2_000);
    let kept = a_key(3_000);
    secrets
        .kept(&ada, &lost, &credential(b"lost", "lost"))
        .await;
    secrets
        .kept(&ada, &kept, &credential(b"kept", "kept"))
        .await;

    // when: one key's credential is forgotten
    secrets
        .forget_credential(&ada, &lost)
        .await
        .expect("forgetting should succeed");

    // then: only the other is held
    assert_eq!(
        secrets.held_by(&ada).await,
        vec![(kept, credential(b"kept", "kept"))]
    );
    // and: forgetting it again is harmless
    secrets
        .forget_credential(&ada, &lost)
        .await
        .expect("forgetting twice should succeed");
    // and: its credential may be kept for a key again
    secrets
        .kept(&ada, &a_key(4_000), &credential(b"lost", "found"))
        .await;
}

pub async fn credential_writes_in_a_dropped_transaction_leave_them_as_they_were<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: an account holding a key
    let ada = an_account(1_000);
    let key = a_key(2_000);
    secrets
        .kept(&ada, &key, &credential(b"key", "before"))
        .await;

    // when: every kind of write is made in a transaction that is dropped
    let mut writing = secrets.begun().await.expect("beginning should succeed");
    assert!(
        secrets
            .store
            .keep_credential(
                &mut writing,
                &ada,
                &a_key(3_000),
                &credential(b"new", "new")
            )
            .await
            .expect("keeping should succeed")
    );
    assert!(
        secrets
            .store
            .renew_credential(&mut writing, &ada, &key, "after")
            .await
            .expect("renewing should succeed")
    );
    secrets
        .store
        .forget_credential(&mut writing, &ada, &key)
        .await
        .expect("forgetting should succeed");
    drop(writing);

    // then: the account holds its key as it was, and nothing else
    assert_eq!(
        secrets.held_by(&ada).await,
        vec![(key, credential(b"key", "before"))]
    );
    // and: the dropped new credential's id is free
    secrets
        .kept(&ada, &a_key(4_000), &credential(b"new", "new"))
        .await;
}
