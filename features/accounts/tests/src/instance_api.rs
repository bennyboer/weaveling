use std::collections::BTreeMap;

use accounts_contract::{AccountPageDTO, LockRequest, SignInRequest, refused};
use accounts_core::{
    AccountCatalog, AccountId, AccountName, AccountSummary, EmailAddress, EmailAddresses,
};
use axum::http::StatusCode;
use axum::http::header::{COOKIE, SET_COOKIE};
use axum_test::{TestResponse, TestServer};
use eventsourcing::Version;
use serde_json::Value;

use crate::wiring;
use crate::wiring::{HER_PASSWORD, Wired, at};

const ADA: &str = "ada@example.com";
const GRACE: &str = "grace@example.com";

struct Instance {
    wired: Wired,
    server: TestServer,
    ada: AccountId,
    ada_signed_in: String,
    grace_signed_in: String,
}

async fn signed_up(wired: &Wired, email: &str, name: &str) -> AccountId {
    wired
        .accounts
        .sign_up(email, name, HER_PASSWORD)
        .await
        .expect("signing up should succeed");
    wired
        .accounts
        .verify(email, &wired.last_code_sent_to(email))
        .await
        .expect("the code sent should verify");

    wired
        .secrets
        .holder(&EmailAddress::new(email).expect("a plain address"))
        .await
        .expect("reading should succeed")
        .expect("the address should be held")
}

async fn signed_in(server: &TestServer, email: &str) -> String {
    server
        .post("/sessions")
        .json(&SignInRequest {
            email: email.to_owned(),
            password: HER_PASSWORD.to_owned(),
        })
        .await
        .header(SET_COOKIE)
        .to_str()
        .expect("a cookie is plain text")
        .split(';')
        .next()
        .expect("the cookie names the session")
        .to_owned()
}

async fn an_instance_with_ada_and_grace_its_operator() -> Instance {
    let wired = wiring::wired();
    let ada = signed_up(&wired, ADA, "Ada").await;
    wired.clock.set(at(1_700_000_060));
    let grace = signed_up(&wired, GRACE, "Grace").await;
    let code = wired
        .accounts
        .draw_operator_code()
        .await
        .expect("drawing should succeed")
        .expect("with nobody appointed a code is drawn");
    wired
        .accounts
        .become_operator(&grace, code.as_str())
        .await
        .expect("the code drawn makes an operator");
    wired.settle().await;
    let server = TestServer::new(wired.routes.clone().merge(
        accounts_rest::admitting_only_the_signed_in(
            wired.accounts.clone(),
            accounts_rest::guarded_router(wired.accounts.clone()),
        ),
    ));
    let ada_signed_in = signed_in(&server, ADA).await;
    let grace_signed_in = signed_in(&server, GRACE).await;

    Instance {
        wired,
        server,
        ada,
        ada_signed_in,
        grace_signed_in,
    }
}

fn refused_as(response: &TestResponse, status: StatusCode, kind: &str) -> Value {
    response.assert_status(status);
    let told: Value = response.json();
    assert_eq!(told["type"], kind, "{told}");

    told
}

impl Instance {
    async fn listed(&self, cookie: &str) -> TestResponse {
        self.wired.settle().await;

        self.server
            .get("/instance/accounts")
            .add_header(COOKIE, cookie)
            .await
    }

    async fn lock(&self, cookie: &str, account: &str, reason: &str) -> TestResponse {
        self.server
            .put(&format!("/instance/accounts/{account}/lock"))
            .add_header(COOKIE, cookie)
            .json(&LockRequest {
                reason: reason.to_owned(),
            })
            .await
    }

    async fn unlock(&self, cookie: &str, account: &str) -> TestResponse {
        self.server
            .delete(&format!("/instance/accounts/{account}/lock"))
            .add_header(COOKIE, cookie)
            .await
    }
}

#[tokio::test]
async fn the_operator_lists_every_account_newest_first_with_what_it_is() {
    let instance = an_instance_with_ada_and_grace_its_operator().await;

    let response = instance.listed(&instance.grace_signed_in).await;

    response.assert_status_ok();
    let page: AccountPageDTO = response.json();
    assert_eq!(
        page.accounts
            .iter()
            .map(|account| (
                account.email.as_str(),
                account.name.as_str(),
                account.verified,
                account.operator
            ))
            .collect::<Vec<_>>(),
        [(GRACE, "Grace", true, true), (ADA, "Ada", true, false)]
    );
    assert_eq!(page.next, None);
}

#[tokio::test]
async fn an_operator_locks_an_account_for_a_reason_and_unlocks_it_again() {
    let instance = an_instance_with_ada_and_grace_its_operator().await;
    let ada = instance.ada.to_string();

    instance
        .lock(&instance.grace_signed_in, &ada, "Sent spam.")
        .await
        .assert_status(StatusCode::NO_CONTENT);

    let page: AccountPageDTO = instance.listed(&instance.grace_signed_in).await.json();
    let listed = page
        .accounts
        .iter()
        .find(|account| account.id == ada)
        .expect("ada is listed");
    assert_eq!(listed.locks.len(), 1);
    assert_eq!(listed.locks[0].kind, "operator");
    assert_eq!(listed.locks[0].reason, "Sent spam.");
    refused_as(
        &instance
            .server
            .post("/sessions")
            .json(&SignInRequest {
                email: ADA.to_owned(),
                password: HER_PASSWORD.to_owned(),
            })
            .await,
        StatusCode::FORBIDDEN,
        refused::ACCOUNT_LOCKED,
    );

    instance
        .unlock(&instance.grace_signed_in, &ada)
        .await
        .assert_status(StatusCode::NO_CONTENT);
    let page: AccountPageDTO = instance.listed(&instance.grace_signed_in).await.json();
    assert!(page.accounts.iter().all(|account| account.locks.is_empty()));
}

#[tokio::test]
async fn anyone_but_the_operator_is_refused() {
    let instance = an_instance_with_ada_and_grace_its_operator().await;
    let ada = instance.ada.to_string();

    refused_as(
        &instance.listed(&instance.ada_signed_in).await,
        StatusCode::FORBIDDEN,
        refused::NOT_AN_OPERATOR,
    );
    refused_as(
        &instance
            .lock(&instance.ada_signed_in, &ada, "I can, can I?")
            .await,
        StatusCode::FORBIDDEN,
        refused::NOT_AN_OPERATOR,
    );
    refused_as(
        &instance.unlock(&instance.ada_signed_in, &ada).await,
        StatusCode::FORBIDDEN,
        refused::NOT_AN_OPERATOR,
    );
}

#[tokio::test]
async fn nobody_signed_out_reaches_the_instance() {
    let instance = an_instance_with_ada_and_grace_its_operator().await;

    refused_as(
        &instance.server.get("/instance/accounts").await,
        StatusCode::UNAUTHORIZED,
        refused::NOT_SIGNED_IN,
    );
}

#[tokio::test]
async fn a_lock_without_a_fitting_reason_is_refused_by_its_kind() {
    let instance = an_instance_with_ada_and_grace_its_operator().await;
    let ada = instance.ada.to_string();

    refused_as(
        &instance.lock(&instance.grace_signed_in, &ada, "  ").await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::LOCK_REASON_BLANK,
    );
    refused_as(
        &instance
            .lock(&instance.grace_signed_in, &ada, "Sent\u{7}spam.")
            .await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::LOCK_REASON_CONTROL_CHARACTER,
    );
    let told = refused_as(
        &instance
            .lock(&instance.grace_signed_in, &ada, &"a".repeat(501))
            .await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::LOCK_REASON_TOO_LONG,
    );
    assert_eq!(
        (told["max"].clone(), told["actual"].clone()),
        (500.into(), 501.into())
    );
}

#[tokio::test]
async fn the_operator_is_not_locked() {
    let instance = an_instance_with_ada_and_grace_its_operator().await;
    let page: AccountPageDTO = instance.listed(&instance.grace_signed_in).await.json();
    let grace = page
        .accounts
        .iter()
        .find(|account| account.operator)
        .expect("grace is listed")
        .id
        .clone();

    refused_as(
        &instance
            .lock(&instance.grace_signed_in, &grace, "Testing.")
            .await,
        StatusCode::CONFLICT,
        refused::OPERATOR_CANNOT_BE_LOCKED,
    );
}

#[tokio::test]
async fn an_account_that_is_not_there_or_no_account_at_all_is_said_so() {
    let instance = an_instance_with_ada_and_grace_its_operator().await;
    let nobody = AccountId::generate(at(1_000)).to_string();

    refused_as(
        &instance
            .lock(&instance.grace_signed_in, &nobody, "Sent spam.")
            .await,
        StatusCode::NOT_FOUND,
        refused::ACCOUNT_NOT_FOUND,
    );
    refused_as(
        &instance
            .lock(&instance.grace_signed_in, "not-an-id", "Sent spam.")
            .await,
        StatusCode::BAD_REQUEST,
        refused::ACCOUNT_ID_INVALID,
    );
    refused_as(
        &instance
            .server
            .get("/instance/accounts")
            .add_query_param("after", "not-an-id")
            .add_header(COOKIE, &instance.grace_signed_in)
            .await,
        StatusCode::BAD_REQUEST,
        refused::ACCOUNT_ID_INVALID,
    );
}

#[tokio::test]
async fn a_long_list_comes_a_page_at_a_time() {
    let instance = an_instance_with_ada_and_grace_its_operator().await;
    for seconds in 0..accounts_rest::PAGE as i64 {
        instance
            .wired
            .catalog
            .remember(&AccountSummary {
                id: AccountId::generate(at(1_000 + seconds)),
                version: Version::of(1),
                name: AccountName::new("Someone").expect("a plain name"),
                email: EmailAddress::new(&format!("someone{seconds}@example.com"))
                    .expect("a plain address"),
                verified: true,
                operator: false,
                signed_up_at: at(1_000 + seconds),
                locks: BTreeMap::new(),
            })
            .await
            .expect("remembering should succeed");
    }

    let first: AccountPageDTO = instance.listed(&instance.grace_signed_in).await.json();
    let after = first.next.clone().expect("more than a page means a next");
    let second: AccountPageDTO = instance
        .server
        .get("/instance/accounts")
        .add_query_param("after", &after)
        .add_header(COOKIE, &instance.grace_signed_in)
        .await
        .json();

    assert_eq!(first.accounts.len(), accounts_rest::PAGE);
    assert_eq!(second.accounts.len(), 2);
    assert_eq!(second.next, None);
}
