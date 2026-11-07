use std::collections::BTreeMap;

use accounts_contract::{AccountPageDTO, LockRequest, SignInRequest, refused};
use accounts_core::{
    AccountCatalog, AccountId, AccountName, AccountSummary, EmailAddress, EmailAddresses, KIND,
};
use axum::http::StatusCode;
use axum::http::header::{COOKIE, SET_COOKIE};
use axum_test::{TestRequest, TestResponse, TestServer};
use eventsourcing::{AggregateId, EventStore, Version};
use serde_json::Value;
use transacting::{InMemoryTransactions, Transactions};

use crate::wiring;
use crate::wiring::{HER_PASSWORD, Wired};

const ADA: &str = "ada@example.com";
const GRACE: &str = "grace@example.com";

struct Instance {
    wired: Wired,
    server: TestServer,
    ada: AccountId,
    ada_signed_in: String,
    grace_signed_in: String,
}

async fn a_verified_account(wired: &Wired, email: &str, name: &str) -> AccountId {
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

async fn a_session_cookie(server: &TestServer, email: &str) -> String {
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
    let ada = a_verified_account(&wired, ADA, "Ada").await;
    wired.clock.set(clock::at(1_700_000_060));
    let grace = a_verified_account(&wired, GRACE, "Grace").await;
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
    let ada_signed_in = a_session_cookie(&server, ADA).await;
    let grace_signed_in = a_session_cookie(&server, GRACE).await;

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
    // given: an instance with two accounts, one its operator
    let instance = an_instance_with_ada_and_grace_its_operator().await;

    // when: the operator lists the accounts
    let response = instance.listed(&instance.grace_signed_in).await;

    // then: every account is listed newest first with what it is
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
    // given: an instance with two accounts, one its operator
    let instance = an_instance_with_ada_and_grace_its_operator().await;
    let ada = instance.ada.to_string();

    // when: the operator locks the other account for a reason
    instance
        .lock(&instance.grace_signed_in, &ada, "Sent spam.")
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: the account is listed with that lock
    let page: AccountPageDTO = instance.listed(&instance.grace_signed_in).await.json();
    let listed = page
        .accounts
        .iter()
        .find(|account| account.id == ada)
        .expect("ada is listed");
    assert_eq!(listed.locks.len(), 1);
    assert_eq!(listed.locks[0].kind, "operator");
    assert_eq!(listed.locks[0].reason, "Sent spam.");
    // and: it cannot sign in
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

    // when: the operator unlocks it
    instance
        .unlock(&instance.grace_signed_in, &ada)
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: no account is listed locked
    let page: AccountPageDTO = instance.listed(&instance.grace_signed_in).await.json();
    assert!(page.accounts.iter().all(|account| account.locks.is_empty()));
}

#[tokio::test]
async fn anyone_but_the_operator_is_refused() {
    // given: an instance with two accounts, one its operator
    let instance = an_instance_with_ada_and_grace_its_operator().await;
    let ada = instance.ada.to_string();

    // then: the other account may not list the accounts
    refused_as(
        &instance.listed(&instance.ada_signed_in).await,
        StatusCode::FORBIDDEN,
        refused::NOT_AN_OPERATOR,
    );
    // and: may not lock one
    refused_as(
        &instance
            .lock(&instance.ada_signed_in, &ada, "I can, can I?")
            .await,
        StatusCode::FORBIDDEN,
        refused::NOT_AN_OPERATOR,
    );
    // and: may not unlock one
    refused_as(
        &instance.unlock(&instance.ada_signed_in, &ada).await,
        StatusCode::FORBIDDEN,
        refused::NOT_AN_OPERATOR,
    );
}

#[tokio::test]
async fn nobody_signed_out_reaches_the_instance() {
    // given: an instance with two accounts, one its operator
    let instance = an_instance_with_ada_and_grace_its_operator().await;

    // then: the accounts are not listed without a session
    refused_as(
        &instance.server.get("/instance/accounts").await,
        StatusCode::UNAUTHORIZED,
        refused::NOT_SIGNED_IN,
    );
}

#[tokio::test]
async fn a_lock_without_a_fitting_reason_is_refused_by_its_kind() {
    // given: an instance with two accounts, one its operator
    let instance = an_instance_with_ada_and_grace_its_operator().await;
    let ada = instance.ada.to_string();

    // then: a lock with a blank reason is refused
    refused_as(
        &instance.lock(&instance.grace_signed_in, &ada, "  ").await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::LOCK_REASON_BLANK,
    );
    // and: one with a control character in its reason
    refused_as(
        &instance
            .lock(&instance.grace_signed_in, &ada, "Sent\u{7}spam.")
            .await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::LOCK_REASON_CONTROL_CHARACTER,
    );
    // and: one with too long a reason, telling how long it may be
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
    // given: an instance with two accounts, one its operator
    let instance = an_instance_with_ada_and_grace_its_operator().await;
    let page: AccountPageDTO = instance.listed(&instance.grace_signed_in).await.json();
    let grace = page
        .accounts
        .iter()
        .find(|account| account.operator)
        .expect("grace is listed")
        .id
        .clone();

    // then: locking the operator is refused
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
    // given: an instance with two accounts, one its operator
    let instance = an_instance_with_ada_and_grace_its_operator().await;
    // and: an account id nobody holds
    let nobody = AccountId::generate(clock::at(1_000)).to_string();

    // then: locking it is refused as not found
    refused_as(
        &instance
            .lock(&instance.grace_signed_in, &nobody, "Sent spam.")
            .await,
        StatusCode::NOT_FOUND,
        refused::ACCOUNT_NOT_FOUND,
    );
    // and: locking something that is no account id is refused as invalid
    refused_as(
        &instance
            .lock(&instance.grace_signed_in, "not-an-id", "Sent spam.")
            .await,
        StatusCode::BAD_REQUEST,
        refused::ACCOUNT_ID_INVALID,
    );
    // and: so is listing after it
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

async fn remembered(instance: &Instance, summary: &AccountSummary) {
    let mut transaction = InMemoryTransactions::new()
        .begin()
        .await
        .expect("beginning should succeed");
    instance
        .wired
        .catalog
        .remember(&mut transaction, summary)
        .await
        .expect("remembering should succeed");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");
}

#[tokio::test]
async fn a_long_list_comes_a_page_at_a_time() {
    // given: an instance with two accounts, one its operator
    let instance = an_instance_with_ada_and_grace_its_operator().await;
    // and: a page more of older accounts
    for seconds in 0..accounts_rest::PAGE as i64 {
        remembered(
            &instance,
            &AccountSummary {
                id: AccountId::generate(clock::at(1_000 + seconds)),
                version: Version::of(1),
                name: AccountName::new("Someone").expect("a plain name"),
                email: EmailAddress::new(&format!("someone{seconds}@example.com"))
                    .expect("a plain address"),
                verified: true,
                operator: false,
                signed_up_at: clock::at(1_000 + seconds),
                locks: BTreeMap::new(),
                second_factor: false,
            },
        )
        .await;
    }

    // when: the accounts are listed, and listed again after the first page
    let first: AccountPageDTO = instance.listed(&instance.grace_signed_in).await.json();
    let after = first.next.clone().expect("more than a page means a next");
    let second: AccountPageDTO = instance
        .server
        .get("/instance/accounts")
        .add_query_param("after", &after)
        .add_header(COOKIE, &instance.grace_signed_in)
        .await
        .json();

    // then: the first page is full and the second holds the rest with no next
    assert_eq!(first.accounts.len(), accounts_rest::PAGE);
    assert_eq!(second.accounts.len(), 2);
    assert_eq!(second.next, None);
}

#[tokio::test]
async fn a_list_that_fills_one_page_exactly_promises_no_next() {
    // given: an instance with two accounts, one its operator
    let instance = an_instance_with_ada_and_grace_its_operator().await;
    // and: just enough older accounts to fill one page
    for seconds in 0..accounts_rest::PAGE as i64 - 2 {
        remembered(
            &instance,
            &AccountSummary {
                id: AccountId::generate(clock::at(1_000 + seconds)),
                version: Version::of(1),
                name: AccountName::new("Someone").expect("a plain name"),
                email: EmailAddress::new(&format!("someone{seconds}@example.com"))
                    .expect("a plain address"),
                verified: true,
                operator: false,
                signed_up_at: clock::at(1_000 + seconds),
                locks: BTreeMap::new(),
                second_factor: false,
            },
        )
        .await;
    }

    // when: the accounts are listed
    let page: AccountPageDTO = instance.listed(&instance.grace_signed_in).await.json();

    // then: the one page is full and promises no next
    assert_eq!(
        page.accounts.len(),
        accounts_rest::PAGE,
        "Ada and Grace fill it"
    );
    assert_eq!(page.next, None);
}

impl Instance {
    async fn appoint(&self, cookie: &str, account: &str) -> TestResponse {
        self.server
            .put(&format!("/instance/accounts/{account}/operator"))
            .add_header(COOKIE, cookie)
            .await
    }

    async fn dismiss(&self, cookie: &str, account: &str) -> TestResponse {
        self.server
            .delete(&format!("/instance/accounts/{account}/operator"))
            .add_header(COOKIE, cookie)
            .await
    }

    async fn is_listed_as_operator(&self, account: &str) -> bool {
        let page: AccountPageDTO = self.listed(&self.grace_signed_in).await.json();

        page.accounts
            .iter()
            .find(|listed| listed.id == account)
            .expect("the account is listed")
            .operator
    }

    async fn grace(&self) -> String {
        let page: AccountPageDTO = self.listed(&self.grace_signed_in).await.json();

        page.accounts
            .iter()
            .find(|listed| listed.email == GRACE)
            .expect("grace is listed")
            .id
            .clone()
    }
}

#[tokio::test]
async fn the_operator_makes_another_operator_and_takes_the_role_away_again() {
    // given: an instance with two accounts, one its operator
    let instance = an_instance_with_ada_and_grace_its_operator().await;
    let ada = instance.ada.to_string();

    // when: the operator appoints the other account
    instance
        .appoint(&instance.grace_signed_in, &ada)
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: it is listed as an operator
    assert!(instance.is_listed_as_operator(&ada).await);

    // when: the operator dismisses it again
    instance
        .dismiss(&instance.grace_signed_in, &ada)
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: it is no longer listed as an operator
    assert!(!instance.is_listed_as_operator(&ada).await);
}

#[tokio::test]
async fn the_last_operator_keeps_the_role() {
    // given: an instance whose only operator is one of two accounts
    let instance = an_instance_with_ada_and_grace_its_operator().await;
    let grace = instance.grace().await;

    // then: dismissing the operator is refused
    refused_as(
        &instance.dismiss(&instance.grace_signed_in, &grace).await,
        StatusCode::CONFLICT,
        refused::LAST_OPERATOR,
    );
    // and: the operator is still listed as one
    assert!(instance.is_listed_as_operator(&grace).await);
}

#[tokio::test]
async fn anyone_but_an_operator_is_refused_the_role_of_others() {
    // given: an instance with two accounts, one its operator
    let instance = an_instance_with_ada_and_grace_its_operator().await;
    let (ada, grace) = (instance.ada.to_string(), instance.grace().await);

    // then: the other account may not appoint
    refused_as(
        &instance.appoint(&instance.ada_signed_in, &ada).await,
        StatusCode::FORBIDDEN,
        refused::NOT_AN_OPERATOR,
    );
    // and: may not dismiss
    refused_as(
        &instance.dismiss(&instance.ada_signed_in, &grace).await,
        StatusCode::FORBIDDEN,
        refused::NOT_AN_OPERATOR,
    );
}

#[tokio::test]
async fn an_account_unconfirmed_locked_or_missing_is_not_made_an_operator() {
    // given: an instance with two accounts, one its operator
    let instance = an_instance_with_ada_and_grace_its_operator().await;
    let ada = instance.ada.to_string();
    // and: an account whose address is unconfirmed
    instance
        .wired
        .accounts
        .sign_up("hedy@example.com", "Hedy", HER_PASSWORD)
        .await
        .expect("signing up should succeed");
    let hedy = instance
        .wired
        .secrets
        .holder(&EmailAddress::new("hedy@example.com").expect("a plain address"))
        .await
        .expect("reading should succeed")
        .expect("the address should be held")
        .to_string();

    // then: the unconfirmed account is not appointed
    refused_as(
        &instance.appoint(&instance.grace_signed_in, &hedy).await,
        StatusCode::CONFLICT,
        refused::OPERATOR_ADDRESS_NOT_VERIFIED,
    );

    // when: another account is locked
    instance
        .lock(&instance.grace_signed_in, &ada, "Sent spam.")
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: the locked account is not appointed
    refused_as(
        &instance.appoint(&instance.grace_signed_in, &ada).await,
        StatusCode::CONFLICT,
        refused::LOCKED_CANNOT_BECOME_OPERATOR,
    );
    // and: an account nobody holds is not found
    refused_as(
        &instance
            .appoint(
                &instance.grace_signed_in,
                &AccountId::generate(clock::at(1_000)).to_string(),
            )
            .await,
        StatusCode::NOT_FOUND,
        refused::ACCOUNT_NOT_FOUND,
    );
}

#[derive(Debug, Clone, Copy)]
enum OperatorAct {
    Lock,
    Unlock,
    Appoint,
    Dismiss,
}

const EVERY_OPERATOR_ACT: [OperatorAct; 4] = [
    OperatorAct::Lock,
    OperatorAct::Unlock,
    OperatorAct::Appoint,
    OperatorAct::Dismiss,
];

impl Instance {
    fn grace_on_ada(&self, act: OperatorAct) -> TestRequest {
        let ada = self.ada.to_string();
        let asked = match act {
            OperatorAct::Lock => self
                .server
                .put(&format!("/instance/accounts/{ada}/lock"))
                .json(&LockRequest {
                    reason: "Sent spam.".to_owned(),
                }),
            OperatorAct::Unlock => self
                .server
                .delete(&format!("/instance/accounts/{ada}/lock")),
            OperatorAct::Appoint => self
                .server
                .put(&format!("/instance/accounts/{ada}/operator")),
            OperatorAct::Dismiss => self
                .server
                .delete(&format!("/instance/accounts/{ada}/operator")),
        };

        asked.add_header(COOKIE, &self.grace_signed_in)
    }

    async fn ada_s_version(&self) -> usize {
        self.wired
            .store
            .read_from(&AggregateId::from(&self.ada), KIND, Version::of(1))
            .await
            .expect("reading should succeed")
            .len()
    }
}

#[tokio::test]
async fn every_operator_act_from_the_version_the_operator_saw_lands() {
    // given: an instance with two accounts, one its operator
    let instance = an_instance_with_ada_and_grace_its_operator().await;
    let mut seen = instance.ada_s_version().await;

    for act in EVERY_OPERATOR_ACT {
        // when: the operator acts on the other account from the version it stands at
        let response = instance
            .grace_on_ada(act)
            .add_header("if-match", format!("\"{seen}\""))
            .await;

        // then: the act lands, moving the account one version on
        response.assert_status(StatusCode::NO_CONTENT);
        assert_eq!(instance.ada_s_version().await, seen + 1, "{act:?}");
        seen += 1;
    }
    // and: the account ends unlocked and no operator
    let page: AccountPageDTO = instance.listed(&instance.grace_signed_in).await.json();
    let ada = page
        .accounts
        .iter()
        .find(|listed| listed.id == instance.ada.to_string())
        .expect("ada is listed");
    assert!(ada.locks.is_empty());
    assert!(!ada.operator);
}

#[tokio::test]
async fn every_operator_act_from_a_version_the_account_has_moved_past_is_refused_and_changes_nothing()
 {
    // given: an instance with two accounts, one its operator
    let instance = an_instance_with_ada_and_grace_its_operator().await;
    let stale = instance.ada_s_version().await;
    // and: the other account locked, unlocked and appointed since
    for act in [OperatorAct::Lock, OperatorAct::Unlock, OperatorAct::Appoint] {
        instance
            .grace_on_ada(act)
            .await
            .assert_status(StatusCode::NO_CONTENT);
    }
    let current = instance.ada_s_version().await;

    for act in EVERY_OPERATOR_ACT {
        // when: the operator acts on it from the version it saw first
        let response = instance
            .grace_on_ada(act)
            .add_header("if-match", format!("\"{stale}\""))
            .await;

        // then: the act is refused as outdated
        refused_as(&response, StatusCode::PRECONDITION_FAILED, "outdated");
        // and: the account stays at the version it stood at
        assert_eq!(instance.ada_s_version().await, current, "{act:?}");
    }
    // and: it is still listed as an operator, and still appointed as one
    assert!(
        instance
            .is_listed_as_operator(&instance.ada.to_string())
            .await
    );
    let grace = instance.grace().await;
    instance
        .unlock(&instance.ada_signed_in, &grace)
        .await
        .assert_status(StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn every_operator_act_naming_no_version_lands_whatever_the_account_stands_at() {
    // given: an instance with two accounts, one its operator
    let instance = an_instance_with_ada_and_grace_its_operator().await;
    let before = instance.ada_s_version().await;

    for act in EVERY_OPERATOR_ACT {
        // when: the operator acts on the other account naming no version
        let response = instance.grace_on_ada(act).await;

        // then: the act lands
        response.assert_status(StatusCode::NO_CONTENT);
    }
    // and: each moved the account one version on
    assert_eq!(instance.ada_s_version().await, before + 4);
}
