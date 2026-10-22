use accounts_core::{
    AccountId, Attempt, CodePurpose, EmailAddress, EmailAddresses, PasswordHash, Passwords,
    PendingVerification, SecretsError, Session, SessionToken, Sessions, Stage, VerificationCode,
    Verifications,
};
use futures_util::future::join_all;
use time::OffsetDateTime;

pub trait Secrets: EmailAddresses + Passwords + Sessions + Verifications {}

impl<S: EmailAddresses + Passwords + Sessions + Verifications> Secrets for S {}

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: Secrets;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

    async fn cleanup(self);
}

pub fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(seconds).expect("a plausible moment")
}

fn an_address(raw: &str) -> EmailAddress {
    EmailAddress::new(raw).expect("a plain address")
}

fn a_hash(phc: &str) -> PasswordHash {
    PasswordHash::kept(phc.to_owned())
}

fn pending(code: &str, until: i64) -> PendingVerification {
    PendingVerification {
        code: VerificationCode::kept(code.to_owned()),
        until: at(until),
        tried: 0,
    }
}

pub async fn a_claimed_address_names_its_holder<S: Secrets>(secrets: &S) {
    let account = AccountId::generate(at(1_000));
    secrets
        .claim(&an_address("ada@example.com"), &account)
        .await
        .expect("claiming should succeed");

    assert_eq!(
        secrets
            .holder(&an_address("ada@example.com"))
            .await
            .expect("reading should succeed"),
        Some(account)
    );
}

pub async fn an_address_never_claimed_names_nobody<S: Secrets>(secrets: &S) {
    assert_eq!(
        secrets
            .holder(&an_address("nobody@example.com"))
            .await
            .expect("reading should succeed"),
        None
    );
}

pub async fn an_address_is_held_by_one_account_only<S: Secrets>(secrets: &S) {
    let first = AccountId::generate(at(1_000));
    secrets
        .claim(&an_address("ada@example.com"), &first)
        .await
        .expect("claiming should succeed");

    let second = secrets
        .claim(
            &an_address("ada@example.com"),
            &AccountId::generate(at(2_000)),
        )
        .await;

    assert!(matches!(second, Err(SecretsError::Taken)), "{second:?}");
    assert_eq!(
        secrets
            .holder(&an_address("ada@example.com"))
            .await
            .expect("reading should succeed"),
        Some(first),
        "two people racing for one address must not both get it, nor the second take it over"
    );
}

pub async fn one_account_may_hold_two_addresses<S: Secrets>(secrets: &S) {
    let account = AccountId::generate(at(1_000));
    for address in ["ada@example.com", "ada@example.org"] {
        secrets
            .claim(&an_address(address), &account)
            .await
            .expect("claiming should succeed");
    }

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

pub async fn a_released_address_can_be_claimed_again<S: Secrets>(secrets: &S) {
    let first = AccountId::generate(at(1_000));
    secrets
        .claim(&an_address("ada@example.com"), &first)
        .await
        .expect("claiming should succeed");

    secrets
        .release(&an_address("ada@example.com"), &first)
        .await
        .expect("releasing should succeed");
    assert_eq!(
        secrets
            .holder(&an_address("ada@example.com"))
            .await
            .expect("reading should succeed"),
        None
    );

    secrets
        .claim(
            &an_address("ada@example.com"),
            &AccountId::generate(at(2_000)),
        )
        .await
        .expect("a released address is free again");
}

pub async fn an_address_is_released_only_by_the_account_holding_it<S: Secrets>(secrets: &S) {
    let holder = AccountId::generate(at(1_000));
    secrets
        .claim(&an_address("ada@example.com"), &holder)
        .await
        .expect("claiming should succeed");

    secrets
        .release(
            &an_address("ada@example.com"),
            &AccountId::generate(at(2_000)),
        )
        .await
        .expect("releasing what is not held is harmless");

    assert_eq!(
        secrets
            .holder(&an_address("ada@example.com"))
            .await
            .expect("reading should succeed"),
        Some(holder),
        "a sign-up undoing itself must never free an address someone else holds"
    );
}

pub async fn a_kept_password_is_read_back_until_a_new_one_replaces_it<S: Secrets>(secrets: &S) {
    let account = AccountId::generate(at(1_000));
    secrets
        .keep(&account, &a_hash("$argon2id$first"))
        .await
        .expect("keeping should succeed");
    assert_eq!(
        secrets.of(&account).await.expect("reading should succeed"),
        Some(a_hash("$argon2id$first"))
    );

    secrets
        .keep(&account, &a_hash("$argon2id$second"))
        .await
        .expect("keeping again should succeed");
    assert_eq!(
        secrets.of(&account).await.expect("reading should succeed"),
        Some(a_hash("$argon2id$second")),
        "only the latest password is kept, so a leaked table holds no old one"
    );
}

pub async fn each_account_has_a_password_of_its_own_or_none<S: Secrets>(secrets: &S) {
    let ada = AccountId::generate(at(1_000));
    secrets
        .keep(&ada, &a_hash("$argon2id$ada"))
        .await
        .expect("keeping should succeed");

    assert_eq!(
        secrets
            .of(&AccountId::generate(at(2_000)))
            .await
            .expect("reading should succeed"),
        None,
        "an account may sign in without a password one day, by a key or elsewhere"
    );
}

pub async fn each_attempt_at_an_issued_code_is_counted_until_it_is_closed<S: Secrets>(secrets: &S) {
    let account = AccountId::generate(at(1_000));
    secrets
        .issue(
            &account,
            CodePurpose::ConfirmAddress,
            &pending("004217", 1_900),
        )
        .await
        .expect("issuing should succeed");

    for tried in 1..=2 {
        assert_eq!(
            secrets
                .attempt(&account, CodePurpose::ConfirmAddress, 5)
                .await
                .expect("attempting should succeed"),
            Attempt::Counted(PendingVerification {
                tried,
                ..pending("004217", 1_900)
            })
        );
    }

    secrets
        .close(&account, CodePurpose::ConfirmAddress)
        .await
        .expect("closing should succeed");
    assert_eq!(
        secrets
            .attempt(&account, CodePurpose::ConfirmAddress, 5)
            .await
            .expect("attempting should succeed"),
        Attempt::Nothing
    );
}

pub async fn attempts_beyond_the_limit_are_not_counted_but_exhausted<S: Secrets>(secrets: &S) {
    let account = AccountId::generate(at(1_000));
    secrets
        .issue(
            &account,
            CodePurpose::ConfirmAddress,
            &pending("004217", 1_900),
        )
        .await
        .expect("issuing should succeed");

    for _ in 0..3 {
        assert!(matches!(
            secrets
                .attempt(&account, CodePurpose::ConfirmAddress, 3)
                .await,
            Ok(Attempt::Counted(_))
        ));
    }

    assert_eq!(
        secrets
            .attempt(&account, CodePurpose::ConfirmAddress, 3)
            .await
            .expect("attempting should succeed"),
        Attempt::Exhausted
    );
}

pub async fn attempts_made_at_once_are_counted_one_by_one<S: Secrets>(secrets: &S) {
    let account = AccountId::generate(at(1_000));
    secrets
        .issue(
            &account,
            CodePurpose::ConfirmAddress,
            &pending("004217", 1_900),
        )
        .await
        .expect("issuing should succeed");

    let attempts =
        join_all((0..20).map(|_| secrets.attempt(&account, CodePurpose::ConfirmAddress, 5))).await;

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

pub async fn a_new_code_starts_its_attempts_afresh<S: Secrets>(secrets: &S) {
    let account = AccountId::generate(at(1_000));
    secrets
        .issue(
            &account,
            CodePurpose::ConfirmAddress,
            &pending("004217", 1_900),
        )
        .await
        .expect("issuing should succeed");
    for _ in 0..2 {
        let _counted = secrets
            .attempt(&account, CodePurpose::ConfirmAddress, 2)
            .await;
    }

    secrets
        .issue(
            &account,
            CodePurpose::ConfirmAddress,
            &pending("731905", 2_900),
        )
        .await
        .expect("issuing again should succeed");

    assert_eq!(
        secrets
            .attempt(&account, CodePurpose::ConfirmAddress, 2)
            .await
            .expect("attempting should succeed"),
        Attempt::Counted(PendingVerification {
            tried: 1,
            ..pending("731905", 2_900)
        }),
        "a new code replaces the old one, and the guesses at the old one no longer count"
    );
}

pub async fn each_account_has_a_code_of_its_own<S: Secrets>(secrets: &S) {
    let ada = AccountId::generate(at(1_000));
    let bea = AccountId::generate(at(2_000));
    secrets
        .issue(&ada, CodePurpose::ConfirmAddress, &pending("111111", 1_900))
        .await
        .expect("issuing should succeed");
    secrets
        .issue(&bea, CodePurpose::ConfirmAddress, &pending("222222", 2_900))
        .await
        .expect("issuing should succeed");

    let _counted = secrets.attempt(&ada, CodePurpose::ConfirmAddress, 5).await;
    secrets
        .close(&ada, CodePurpose::ConfirmAddress)
        .await
        .expect("closing should succeed");

    assert_eq!(
        secrets
            .attempt(&bea, CodePurpose::ConfirmAddress, 5)
            .await
            .expect("attempting should succeed"),
        Attempt::Counted(PendingVerification {
            tried: 1,
            ..pending("222222", 2_900)
        })
    );
}

pub async fn attempting_or_closing_where_nothing_was_issued_is_harmless<S: Secrets>(secrets: &S) {
    let account = AccountId::generate(at(1_000));

    assert_eq!(
        secrets
            .attempt(&account, CodePurpose::ConfirmAddress, 5)
            .await
            .expect("attempting should succeed"),
        Attempt::Nothing
    );
    secrets
        .close(&account, CodePurpose::ConfirmAddress)
        .await
        .expect("closing nothing is fine");
}

fn a_session(stage: Stage) -> Session {
    Session {
        account: AccountId::generate(at(1_000)),
        stage,
        begun: at(1_699_000_000),
        until: at(1_700_000_000),
    }
}

pub async fn an_opened_session_is_found_by_its_tokens_digest_until_it_ends<S: Secrets>(
    secrets: &S,
) {
    let token = SessionToken::drawn();
    let session = a_session(Stage::SignedIn);
    secrets
        .open(&token.digest(), &session)
        .await
        .expect("opening should succeed");

    assert_eq!(
        secrets
            .find(&SessionToken::presented(token.as_str()).digest())
            .await
            .expect("finding should succeed"),
        Some(session)
    );

    secrets
        .end(&token.digest())
        .await
        .expect("closing should succeed");
    assert_eq!(
        secrets
            .find(&token.digest())
            .await
            .expect("finding should succeed"),
        None
    );
}

pub async fn a_token_nobody_was_given_finds_nothing<S: Secrets>(secrets: &S) {
    secrets
        .open(&SessionToken::drawn().digest(), &a_session(Stage::SignedIn))
        .await
        .expect("opening should succeed");

    assert_eq!(
        secrets
            .find(&SessionToken::drawn().digest())
            .await
            .expect("finding should succeed"),
        None
    );
}

pub async fn each_stage_is_kept_as_it_was<S: Secrets>(secrets: &S) {
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

pub async fn ending_one_session_leaves_the_others_open<S: Secrets>(secrets: &S) {
    let phone = SessionToken::drawn();
    let laptop = SessionToken::drawn();
    for token in [&phone, &laptop] {
        secrets
            .open(&token.digest(), &a_session(Stage::SignedIn))
            .await
            .expect("opening should succeed");
    }

    secrets
        .end(&phone.digest())
        .await
        .expect("closing should succeed");
    secrets
        .end(&SessionToken::drawn().digest())
        .await
        .expect("ending a session never opened is harmless");

    assert!(
        secrets
            .find(&laptop.digest())
            .await
            .expect("finding should succeed")
            .is_some()
    );
}

pub async fn a_code_for_one_purpose_is_no_code_for_another<S: Secrets>(secrets: &S) {
    let account = AccountId::generate(at(1_000));
    secrets
        .issue(
            &account,
            CodePurpose::ConfirmAddress,
            &pending("111111", 1_900),
        )
        .await
        .expect("issuing should succeed");
    secrets
        .issue(
            &account,
            CodePurpose::ResetPassword,
            &pending("222222", 2_900),
        )
        .await
        .expect("issuing should succeed");

    secrets
        .close(&account, CodePurpose::ConfirmAddress)
        .await
        .expect("closing should succeed");

    assert_eq!(
        secrets
            .attempt(&account, CodePurpose::ConfirmAddress, 5)
            .await
            .expect("attempting should succeed"),
        Attempt::Nothing
    );
    assert_eq!(
        secrets
            .attempt(&account, CodePurpose::ResetPassword, 5)
            .await
            .expect("attempting should succeed"),
        Attempt::Counted(PendingVerification {
            tried: 1,
            ..pending("222222", 2_900)
        }),
        "a code sent to reset a password must not confirm an address, nor fall with it"
    );
}

#[macro_export]
macro_rules! secrets_conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::suite::$case(bench.store()).await;
            bench.cleanup().await;
        }
    };
}

#[macro_export]
macro_rules! conformance_tests {
    ($workbench:ty) => {
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
    };
}
