use accounts_core::{
    AccountId, Attempt, Credentials, EmailAddress, PasswordHash, PendingVerification, Registered,
    SecretsError, VerificationCode, Verifications,
};
use futures_util::future::join_all;
use time::OffsetDateTime;

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: Credentials + Verifications;

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

pub async fn a_registered_address_names_its_account_and_password<S>(secrets: &S)
where
    S: Credentials + Verifications,
{
    let account = AccountId::generate(at(1_000));
    secrets
        .register(
            &an_address("ada@example.com"),
            &account,
            &a_hash("$argon2id$ada"),
        )
        .await
        .expect("registering should succeed");

    assert_eq!(
        secrets
            .registered(&an_address("ada@example.com"))
            .await
            .expect("reading should succeed"),
        Some(Registered {
            account,
            password: a_hash("$argon2id$ada"),
        })
    );
}

pub async fn an_address_never_registered_names_nobody<S>(secrets: &S)
where
    S: Credentials + Verifications,
{
    assert_eq!(
        secrets
            .registered(&an_address("nobody@example.com"))
            .await
            .expect("reading should succeed"),
        None
    );
}

pub async fn an_address_belongs_to_one_account_only<S>(secrets: &S)
where
    S: Credentials + Verifications,
{
    let first = AccountId::generate(at(1_000));
    secrets
        .register(
            &an_address("ada@example.com"),
            &first,
            &a_hash("$argon2id$first"),
        )
        .await
        .expect("registering should succeed");

    let second = secrets
        .register(
            &an_address("ada@example.com"),
            &AccountId::generate(at(2_000)),
            &a_hash("$argon2id$second"),
        )
        .await;

    assert!(matches!(second, Err(SecretsError::Taken)), "{second:?}");
    assert_eq!(
        secrets
            .registered(&an_address("ada@example.com"))
            .await
            .expect("reading should succeed")
            .map(|registered| registered.account),
        Some(first),
        "two people racing for one address must not both get it, nor the second take it over"
    );
}

pub async fn a_released_address_can_be_registered_again<S>(secrets: &S)
where
    S: Credentials + Verifications,
{
    let first = AccountId::generate(at(1_000));
    secrets
        .register(
            &an_address("ada@example.com"),
            &first,
            &a_hash("$argon2id$first"),
        )
        .await
        .expect("registering should succeed");

    secrets
        .release(&an_address("ada@example.com"), &first)
        .await
        .expect("releasing should succeed");
    assert_eq!(
        secrets
            .registered(&an_address("ada@example.com"))
            .await
            .expect("reading should succeed"),
        None
    );

    let second = AccountId::generate(at(2_000));
    secrets
        .register(
            &an_address("ada@example.com"),
            &second,
            &a_hash("$argon2id$second"),
        )
        .await
        .expect("a released address is free again");
}

pub async fn an_address_is_released_only_by_the_account_holding_it<S>(secrets: &S)
where
    S: Credentials + Verifications,
{
    let holder = AccountId::generate(at(1_000));
    secrets
        .register(
            &an_address("ada@example.com"),
            &holder,
            &a_hash("$argon2id$ada"),
        )
        .await
        .expect("registering should succeed");

    secrets
        .release(
            &an_address("ada@example.com"),
            &AccountId::generate(at(2_000)),
        )
        .await
        .expect("releasing what is not held is harmless");

    assert_eq!(
        secrets
            .registered(&an_address("ada@example.com"))
            .await
            .expect("reading should succeed")
            .map(|registered| registered.account),
        Some(holder),
        "a sign-up undoing itself must never free an address someone else holds"
    );
}

pub async fn each_attempt_at_an_issued_code_is_counted_until_it_is_closed<S>(secrets: &S)
where
    S: Credentials + Verifications,
{
    let account = AccountId::generate(at(1_000));
    secrets
        .issue(&account, &pending("004217", 1_900))
        .await
        .expect("issuing should succeed");

    for tried in 1..=2 {
        assert_eq!(
            secrets
                .attempt(&account, 5)
                .await
                .expect("attempting should succeed"),
            Attempt::Counted(PendingVerification {
                tried,
                ..pending("004217", 1_900)
            })
        );
    }

    secrets
        .close(&account)
        .await
        .expect("closing should succeed");
    assert_eq!(
        secrets
            .attempt(&account, 5)
            .await
            .expect("attempting should succeed"),
        Attempt::Nothing
    );
}

pub async fn attempts_beyond_the_limit_are_not_counted_but_exhausted<S>(secrets: &S)
where
    S: Credentials + Verifications,
{
    let account = AccountId::generate(at(1_000));
    secrets
        .issue(&account, &pending("004217", 1_900))
        .await
        .expect("issuing should succeed");

    for _ in 0..3 {
        assert!(matches!(
            secrets.attempt(&account, 3).await,
            Ok(Attempt::Counted(_))
        ));
    }

    assert_eq!(
        secrets
            .attempt(&account, 3)
            .await
            .expect("attempting should succeed"),
        Attempt::Exhausted
    );
}

pub async fn attempts_made_at_once_are_counted_one_by_one<S>(secrets: &S)
where
    S: Credentials + Verifications,
{
    let account = AccountId::generate(at(1_000));
    secrets
        .issue(&account, &pending("004217", 1_900))
        .await
        .expect("issuing should succeed");

    let attempts = join_all((0..20).map(|_| secrets.attempt(&account, 5))).await;

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

pub async fn a_new_code_starts_its_attempts_afresh<S>(secrets: &S)
where
    S: Credentials + Verifications,
{
    let account = AccountId::generate(at(1_000));
    secrets
        .issue(&account, &pending("004217", 1_900))
        .await
        .expect("issuing should succeed");
    for _ in 0..2 {
        let _counted = secrets.attempt(&account, 2).await;
    }

    secrets
        .issue(&account, &pending("731905", 2_900))
        .await
        .expect("issuing again should succeed");

    assert_eq!(
        secrets
            .attempt(&account, 2)
            .await
            .expect("attempting should succeed"),
        Attempt::Counted(PendingVerification {
            tried: 1,
            ..pending("731905", 2_900)
        }),
        "a new code replaces the old one, and the guesses at the old one no longer count"
    );
}

pub async fn each_account_has_a_code_of_its_own<S>(secrets: &S)
where
    S: Credentials + Verifications,
{
    let ada = AccountId::generate(at(1_000));
    let bea = AccountId::generate(at(2_000));
    secrets
        .issue(&ada, &pending("111111", 1_900))
        .await
        .expect("issuing should succeed");
    secrets
        .issue(&bea, &pending("222222", 2_900))
        .await
        .expect("issuing should succeed");

    let _counted = secrets.attempt(&ada, 5).await;
    secrets.close(&ada).await.expect("closing should succeed");

    assert_eq!(
        secrets
            .attempt(&bea, 5)
            .await
            .expect("attempting should succeed"),
        Attempt::Counted(PendingVerification {
            tried: 1,
            ..pending("222222", 2_900)
        })
    );
}

pub async fn attempting_or_closing_where_nothing_was_issued_is_harmless<S>(secrets: &S)
where
    S: Credentials + Verifications,
{
    let account = AccountId::generate(at(1_000));

    assert_eq!(
        secrets
            .attempt(&account, 5)
            .await
            .expect("attempting should succeed"),
        Attempt::Nothing
    );
    secrets
        .close(&account)
        .await
        .expect("closing nothing is fine");
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
        $crate::secrets_conformance_case!(
            $workbench,
            a_registered_address_names_its_account_and_password
        );
        $crate::secrets_conformance_case!($workbench, an_address_never_registered_names_nobody);
        $crate::secrets_conformance_case!($workbench, an_address_belongs_to_one_account_only);
        $crate::secrets_conformance_case!($workbench, a_released_address_can_be_registered_again);
        $crate::secrets_conformance_case!(
            $workbench,
            an_address_is_released_only_by_the_account_holding_it
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
    };
}
