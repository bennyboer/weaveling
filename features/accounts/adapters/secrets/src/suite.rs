use accounts_core::{
    AccountId, Credentials, EmailAddress, PasswordHash, PendingVerification, Registered,
    SecretsError, VerificationCode, Verifications,
};
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
        failed: 0,
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

pub async fn an_issued_code_is_pending_until_closed<S>(secrets: &S)
where
    S: Credentials + Verifications,
{
    let account = AccountId::generate(at(1_000));
    secrets
        .issue(&account, &pending("004217", 1_900))
        .await
        .expect("issuing should succeed");

    assert_eq!(
        secrets
            .pending(&account)
            .await
            .expect("reading should succeed"),
        Some(pending("004217", 1_900))
    );

    secrets
        .close(&account)
        .await
        .expect("closing should succeed");

    assert_eq!(
        secrets
            .pending(&account)
            .await
            .expect("reading should succeed"),
        None
    );
}

pub async fn each_failure_is_counted_until_a_new_code_starts_afresh<S>(secrets: &S)
where
    S: Credentials + Verifications,
{
    let account = AccountId::generate(at(1_000));
    secrets
        .issue(&account, &pending("004217", 1_900))
        .await
        .expect("issuing should succeed");

    for _ in 0..2 {
        secrets
            .failed(&account)
            .await
            .expect("counting should succeed");
    }
    assert_eq!(
        secrets
            .pending(&account)
            .await
            .expect("reading should succeed")
            .map(|pending| pending.failed),
        Some(2)
    );

    secrets
        .issue(&account, &pending("731905", 2_900))
        .await
        .expect("issuing again should succeed");
    assert_eq!(
        secrets
            .pending(&account)
            .await
            .expect("reading should succeed"),
        Some(pending("731905", 2_900)),
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

    secrets.failed(&ada).await.expect("counting should succeed");
    secrets.close(&ada).await.expect("closing should succeed");

    assert_eq!(
        secrets.pending(&bea).await.expect("reading should succeed"),
        Some(pending("222222", 2_900))
    );
}

pub async fn failing_or_closing_where_nothing_is_pending_is_harmless<S>(secrets: &S)
where
    S: Credentials + Verifications,
{
    let account = AccountId::generate(at(1_000));

    secrets
        .failed(&account)
        .await
        .expect("counting nothing is fine");
    secrets
        .close(&account)
        .await
        .expect("closing nothing is fine");

    assert_eq!(
        secrets
            .pending(&account)
            .await
            .expect("reading should succeed"),
        None
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
        $crate::secrets_conformance_case!(
            $workbench,
            a_registered_address_names_its_account_and_password
        );
        $crate::secrets_conformance_case!($workbench, an_address_never_registered_names_nobody);
        $crate::secrets_conformance_case!($workbench, an_address_belongs_to_one_account_only);
        $crate::secrets_conformance_case!($workbench, an_issued_code_is_pending_until_closed);
        $crate::secrets_conformance_case!(
            $workbench,
            each_failure_is_counted_until_a_new_code_starts_afresh
        );
        $crate::secrets_conformance_case!($workbench, each_account_has_a_code_of_its_own);
        $crate::secrets_conformance_case!(
            $workbench,
            failing_or_closing_where_nothing_is_pending_is_harmless
        );
    };
}
