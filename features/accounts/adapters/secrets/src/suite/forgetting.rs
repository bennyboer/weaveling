use accounts_core::{
    AccountId, Attempt, CodeDigest, CodePurpose, KeyCredential, KeyId, PasswordHash, Passwords,
    PendingVerification, RecoveryCodeHash, SealedTotpSecret, SecondStepWay, SecretsError, Session,
    Sessions, Stage, TokenDigest, TotpStep,
};
use transacting::{Transaction, Tried};

use super::{Committing, Secrets, unreachable};

const EVERYTHING: [&str; 8] = [
    "password",
    "session",
    "code",
    "totp",
    "totp being set up",
    "recovery codes",
    "the way that answered last",
    "key",
];

impl<S: Secrets> Committing<'_, S> {
    pub async fn forget_every_of(&self, account: &AccountId) -> Result<(), SecretsError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.forgetting(transaction, account)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    async fn forgetting(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), SecretsError> {
        self.store.forget_password_of(transaction, account).await?;
        self.store
            .end_every_session_of(transaction, account)
            .await?;
        self.store.forget_codes_of(transaction, account).await?;
        self.store
            .forget_second_factors_of(transaction, account)
            .await?;
        self.store.forget_credentials_of(transaction, account).await
    }

    async fn holding_everything(&self, account: &AccountId, name: &str) {
        self.keep(account, &PasswordHash::kept(format!("hash of {name}")))
            .await
            .expect("keeping should succeed");
        self.open(&digest_of(name), &session_of(account))
            .await
            .expect("opening should succeed");
        self.issue(
            account,
            CodePurpose::ResetPassword,
            &PendingVerification {
                digest: CodeDigest::kept("123456".to_owned()),
                until: clock::at(1_700_000_000),
                tried: 0,
            },
        )
        .await
        .expect("issuing should succeed");
        let in_use = SealedTotpSecret::kept(format!("{name} in use").into_bytes());
        self.begin_setting_up(account, &in_use)
            .await
            .expect("setting up should succeed");
        assert!(
            self.finish_setting_up(account, &in_use, TotpStep::kept(1))
                .await
                .expect("finishing should succeed")
        );
        self.begin_setting_up(
            account,
            &SealedTotpSecret::kept(format!("{name} anew").into_bytes()),
        )
        .await
        .expect("setting up anew should succeed");
        self.keep_recovery_codes(
            account,
            &[RecoveryCodeHash::kept(format!("code of {name}"))],
        )
        .await
        .expect("keeping should succeed");
        self.note_answered_by(account, SecondStepWay::Totp)
            .await
            .expect("noting should succeed");
        assert!(
            self.keep_credential(
                account,
                &KeyId::generate(clock::at(3_000)),
                &KeyCredential {
                    id: name.as_bytes().to_vec(),
                    written: name.to_owned(),
                },
            )
            .await
            .expect("keeping should succeed")
        );
    }

    async fn secrets_held_by(&self, account: &AccountId, name: &str) -> Vec<&'static str> {
        let store = self.store;
        let read = "reading should succeed";
        let held = [
            Passwords::of(store, account).await.expect(read).is_some(),
            store.find(&digest_of(name)).await.expect(read).is_some(),
            matches!(
                self.attempt(account, CodePurpose::ResetPassword, 100)
                    .await
                    .expect(read),
                Attempt::Counted(_)
            ),
            store.totp_of(account).await.expect(read).is_some(),
            store
                .totp_being_set_up(account)
                .await
                .expect(read)
                .is_some(),
            !store
                .recovery_codes_of(account)
                .await
                .expect(read)
                .is_empty(),
            store.answered_last_by(account).await.expect(read).is_some(),
            !store.credentials_of(account).await.expect(read).is_empty(),
        ];

        EVERYTHING
            .into_iter()
            .zip(held)
            .filter_map(|(secret, held)| held.then_some(secret))
            .collect()
    }
}

fn digest_of(name: &str) -> TokenDigest {
    TokenDigest::kept(format!("session of {name}"))
}

fn session_of(account: &AccountId) -> Session {
    Session {
        account: *account,
        stage: Stage::SignedIn,
        begun: clock::at(1_699_000_000),
        until: clock::at(1_700_000_000),
    }
}

pub async fn a_forgotten_account_holds_no_secret_and_others_keep_theirs<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: two accounts, each holding every kind of secret
    let ada = AccountId::generate(clock::at(1_000));
    let grace = AccountId::generate(clock::at(2_000));
    secrets.holding_everything(&ada, "ada").await;
    secrets.holding_everything(&grace, "grace").await;

    // when: the first is forgotten in a transaction that is dropped
    let mut writing = secrets.begun().await.expect("beginning should succeed");
    secrets
        .forgetting(&mut writing, &ada)
        .await
        .expect("forgetting should succeed");
    drop(writing);

    // then: she still holds everything
    assert_eq!(secrets.secrets_held_by(&ada, "ada").await, EVERYTHING);

    // when: she is forgotten
    secrets
        .forget_every_of(&ada)
        .await
        .expect("forgetting should succeed");

    // then: she holds nothing, and the other keeps everything
    assert!(secrets.secrets_held_by(&ada, "ada").await.is_empty());
    assert_eq!(secrets.secrets_held_by(&grace, "grace").await, EVERYTHING);
    // and: forgetting her again is harmless
    secrets
        .forget_every_of(&ada)
        .await
        .expect("forgetting twice should succeed");
}

pub async fn sessions_and_codes_run_out_are_forgotten_oldest_first_and_the_rest_kept<S: Secrets>(
    secrets: &Committing<'_, S>,
) {
    // given: a session and a code of each of three accounts, run out at one, two and five
    let accounts: Vec<(AccountId, &str, i64)> = vec![
        (AccountId::generate(clock::at(1_000)), "ada", 1_000),
        (AccountId::generate(clock::at(2_000)), "grace", 2_000),
        (AccountId::generate(clock::at(3_000)), "bea", 5_000),
    ];
    for (account, name, until) in &accounts {
        secrets
            .open(
                &digest_of(name),
                &Session {
                    until: clock::at(*until),
                    ..session_of(account)
                },
            )
            .await
            .expect("opening should succeed");
        secrets
            .issue(
                account,
                CodePurpose::ConfirmAddress,
                &PendingVerification {
                    digest: CodeDigest::kept(format!("code of {name}")),
                    until: clock::at(*until),
                    tried: 0,
                },
            )
            .await
            .expect("issuing should succeed");
    }
    let store = secrets.store;
    let before = clock::at(3_000);

    // when: one of each run out before three is forgotten
    let sessions = store
        .forget_sessions_run_out_before(before, 1)
        .await
        .expect("forgetting should succeed");
    let codes = store
        .forget_codes_run_out_before(before, 1)
        .await
        .expect("forgetting should succeed");

    // then: the oldest of each is gone, and only it
    assert_eq!((sessions, codes), (1, 1));
    assert_eq!(
        secrets.run_out_held(&accounts).await,
        [(false, false), (true, true), (true, true)]
    );

    // when: the rest run out before three are forgotten
    let sessions = store
        .forget_sessions_run_out_before(before, 100)
        .await
        .expect("forgetting should succeed");
    let codes = store
        .forget_codes_run_out_before(before, 100)
        .await
        .expect("forgetting should succeed");

    // then: the one left of each is gone, and those running out later are kept
    assert_eq!((sessions, codes), (1, 1));
    assert_eq!(
        secrets.run_out_held(&accounts).await,
        [(false, false), (false, false), (true, true)]
    );
}

impl<S: Secrets> Committing<'_, S> {
    async fn run_out_held(&self, accounts: &[(AccountId, &str, i64)]) -> Vec<(bool, bool)> {
        let mut held = Vec::new();
        for (account, name, _) in accounts {
            let session = Sessions::find(self.store, &digest_of(name))
                .await
                .expect("reading should succeed")
                .is_some();
            let code = matches!(
                self.attempt(account, CodePurpose::ConfirmAddress, 100)
                    .await
                    .expect("reading should succeed"),
                Attempt::Counted(_)
            );
            held.push((session, code));
        }

        held
    }
}
