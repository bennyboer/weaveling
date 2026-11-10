use accounts_contract::{
    AccountPageDTO, AccountSummaryDTO, DeletionDTO, PasswordRequest, SignInRequest, SignedInDTO,
    deletion_asked_by, refused, stage,
};
use accounts_core::{AccountId, InProject, KEPT_BEFORE_DELETION};
use axum::http::StatusCode;
use axum::http::header::{COOKIE, SET_COOKIE};
use axum_test::TestResponse;
use clock::text;
use eventsourcing::Version;

use crate::totp_bench::{Served, cookie_set_by, refused_as, served};
use crate::wiring::{ADA, HER_PASSWORD};

impl Served {
    async fn ask_deletion(&self, cookie: &str, password: &str) -> TestResponse {
        self.server
            .post("/accounts/current/deletion")
            .add_header(COOKIE, cookie.to_owned())
            .json(&PasswordRequest {
                password: password.to_owned(),
            })
            .await
    }

    async fn keep(&self, cookie: &str) -> TestResponse {
        self.server
            .delete("/accounts/current/deletion")
            .add_header(COOKIE, cookie.to_owned())
            .await
    }

    async fn pending_session(&self) -> TestResponse {
        self.server
            .post("/sessions")
            .json(&SignInRequest {
                email: ADA.to_owned(),
                password: HER_PASSWORD.to_owned(),
            })
            .await
    }

    async fn marked_by(&self, cookie: &str, account: &AccountId) -> TestResponse {
        self.server
            .put(&format!("/instance/accounts/{account}/deletion"))
            .add_header(COOKIE, cookie.to_owned())
            .await
    }

    async fn kept_by(&self, cookie: &str, account: &AccountId) -> TestResponse {
        self.server
            .delete(&format!("/instance/accounts/{account}/deletion"))
            .add_header(COOKIE, cookie.to_owned())
            .await
    }

    async fn listed_for(&self, cookie: &str, account: &AccountId) -> AccountSummaryDTO {
        self.wired.settle().await;
        let page: AccountPageDTO = self
            .server
            .get("/instance/accounts")
            .add_header(COOKIE, cookie.to_owned())
            .await
            .json();

        page.accounts
            .into_iter()
            .find(|listed| listed.id == account.to_string())
            .expect("the account is listed")
    }

    async fn holding(&self, project: &str, account: &AccountId, version: u64, held: InProject) {
        self.wired
            .accounts
            .remember_holding(None, project, account, Version::of(version), Some(held))
            .await
            .expect("remembering should succeed");
    }
}

fn thirty_days_on() -> String {
    text::serialize(clock::at(1_700_000_000) + KEPT_BEFORE_DELETION)
}

#[tokio::test]
async fn asking_with_her_password_tells_until_when_she_is_kept_and_signs_her_out() {
    // given: ada signed in
    let served = served(|wired| wired.accounts.clone()).await;
    let cookie = served.signed_in().await;

    // when: she asks for her account to be deleted with her password
    let asked = served.ask_deletion(&cookie, HER_PASSWORD).await;

    // then: she is told until when it is kept, asked by her
    asked.assert_status_ok();
    assert_eq!(
        asked.json::<DeletionDTO>(),
        DeletionDTO {
            until: thirty_days_on(),
            by: deletion_asked_by::HOLDER.to_owned(),
        }
    );
    // and: her cookie is let go of
    assert!(
        asked
            .header(SET_COOKIE)
            .to_str()
            .expect("a cookie is plain text")
            .contains("Max-Age=0")
    );
    // and: her session no longer signs her in
    refused_as(
        &served.current(&cookie).await,
        StatusCode::UNAUTHORIZED,
        refused::NOT_SIGNED_IN,
    );
}

#[tokio::test]
async fn asking_with_a_wrong_password_is_refused_and_keeps_her_signed_in() {
    // given: ada signed in
    let served = served(|wired| wired.accounts.clone()).await;
    let cookie = served.signed_in().await;

    // when: she asks with a wrong password
    let refused = served.ask_deletion(&cookie, "not her password").await;

    // then: it is refused as the wrong password
    refused_as(
        &refused,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::PASSWORD_WRONG,
    );
    // and: she is still signed in
    served.current(&cookie).await.assert_status_ok();
}

#[tokio::test]
async fn asking_while_the_last_owner_of_a_shared_project_names_the_projects_to_hand_over() {
    // given: ada signed in, the last owner of a project grace is in
    let served = served(|wired| wired.accounts.clone()).await;
    let grace = AccountId::generate(clock::at(2_000));
    served
        .holding("project_shared", &served.ada, 1, InProject::Owner)
        .await;
    served
        .holding("project_shared", &grace, 2, InProject::Member)
        .await;
    let cookie = served.signed_in().await;

    // when: she asks for her account to be deleted
    let refused = served.ask_deletion(&cookie, HER_PASSWORD).await;

    // then: it is refused, naming the project to hand over
    refused_as(
        &refused,
        StatusCode::CONFLICT,
        refused::PROJECTS_TO_HAND_OVER,
    );
    assert_eq!(
        refused.json::<serde_json::Value>()["projects"],
        serde_json::json!(["project_shared"])
    );
}

#[tokio::test]
async fn signed_in_while_pending_she_is_told_until_when_and_keeps_her_account_signed_in_anew() {
    // given: ada's account asked to be deleted
    let served = served(|wired| wired.accounts.clone()).await;
    let cookie = served.signed_in().await;
    served
        .ask_deletion(&cookie, HER_PASSWORD)
        .await
        .assert_status_ok();

    // when: she signs in again
    let pending = cookie_set_by(&served.pending_session().await);

    // then: her session may only keep the account, and says until when
    let current: SignedInDTO = served.current(&pending).await.json();
    assert_eq!(current.stage, stage::DELETION_PENDING);
    assert_eq!(
        current.deletion,
        Some(DeletionDTO {
            until: thirty_days_on(),
            by: deletion_asked_by::HOLDER.to_owned(),
        })
    );

    // when: she keeps her account
    let kept = served.keep(&pending).await;

    // then: she is signed in, with no deletion pending
    kept.assert_status_ok();
    let signed_in: SignedInDTO = kept.json();
    assert_eq!(signed_in.stage, stage::SIGNED_IN);
    assert_eq!(signed_in.deletion, None);
    // and: under a new session, the pending one over
    let anew = cookie_set_by(&kept);
    assert_ne!(anew, pending);
    served.current(&anew).await.assert_status_ok();
    refused_as(
        &served.current(&pending).await,
        StatusCode::UNAUTHORIZED,
        refused::NOT_SIGNED_IN,
    );
}

#[tokio::test]
async fn keeping_is_refused_with_no_deletion_pending_or_no_session_at_all() {
    // given: ada signed in, nothing pending
    let served = served(|wired| wired.accounts.clone()).await;
    let cookie = served.signed_in().await;

    // expect: keeping her account is refused as nothing pending
    refused_as(
        &served.keep(&cookie).await,
        StatusCode::CONFLICT,
        refused::NO_DELETION_PENDING,
    );
    // and: without a session, as nobody signed in
    refused_as(
        &served.server.delete("/accounts/current/deletion").await,
        StatusCode::UNAUTHORIZED,
        refused::NOT_SIGNED_IN,
    );
}

#[tokio::test]
async fn the_holder_cannot_keep_an_account_the_operator_marked() {
    // given: ada's account marked for deletion by grace, the operator
    let served = served(|wired| wired.accounts.clone()).await;
    let (grace, _) = served.grace_the_operator().await;
    let marked = served
        .wired
        .accounts
        .mark_for_deletion(&grace, &served.ada, None)
        .await
        .expect("an operator marks an account");
    // and: ada signed in while it is pending
    let pending = cookie_set_by(&served.pending_session().await);

    // then: her session says the operator asked
    let current: SignedInDTO = served.current(&pending).await.json();
    assert_eq!(
        current.deletion,
        Some(DeletionDTO {
            until: text::serialize(marked.until),
            by: deletion_asked_by::OPERATOR.to_owned(),
        })
    );
    // and: keeping it is refused as the operator's to call off
    refused_as(
        &served.keep(&pending).await,
        StatusCode::FORBIDDEN,
        refused::DELETION_ASKED_BY_THE_OPERATOR,
    );
}

#[tokio::test]
async fn the_operator_marks_an_account_which_is_listed_so_until_she_calls_it_off() {
    // given: ada signed in, and grace the operator
    let served = served(|wired| wired.accounts.clone()).await;
    let ada_s = served.signed_in().await;
    let (_, grace_s) = served.grace_the_operator().await;

    // when: grace marks ada's account for deletion
    let marked = served.marked_by(&grace_s, &served.ada).await;

    // then: she is told until when it is kept, asked by the operator
    marked.assert_status_ok();
    let deletion: DeletionDTO = marked.json();
    assert_eq!(deletion.by, deletion_asked_by::OPERATOR);
    // and: the list tells it so
    assert_eq!(
        served.listed_for(&grace_s, &served.ada).await.deletion,
        Some(deletion.clone())
    );
    // and: ada is signed out
    refused_as(
        &served.current(&ada_s).await,
        StatusCode::UNAUTHORIZED,
        refused::NOT_SIGNED_IN,
    );
    // and: marking again answers the same day
    assert_eq!(
        served
            .marked_by(&grace_s, &served.ada)
            .await
            .json::<DeletionDTO>(),
        deletion
    );

    // when: grace calls it off
    let kept = served.kept_by(&grace_s, &served.ada).await;

    // then: it is kept, and listed with no deletion
    kept.assert_status(StatusCode::NO_CONTENT);
    assert_eq!(
        served.listed_for(&grace_s, &served.ada).await.deletion,
        None
    );
}

#[tokio::test]
async fn the_operator_calls_off_what_the_holder_asked_too() {
    // given: ada's account asked to be deleted by her
    let served = served(|wired| wired.accounts.clone()).await;
    let (_, grace_s) = served.grace_the_operator().await;
    let ada_s = served.signed_in().await;
    served
        .ask_deletion(&ada_s, HER_PASSWORD)
        .await
        .assert_status_ok();
    // and: listed so, asked by her
    assert_eq!(
        served
            .listed_for(&grace_s, &served.ada)
            .await
            .deletion
            .map(|deletion| deletion.by),
        Some(deletion_asked_by::HOLDER.to_owned())
    );

    // when: grace calls it off
    served
        .kept_by(&grace_s, &served.ada)
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: ada signs in all the way again
    let anew = cookie_set_by(&served.pending_session().await);
    let current: SignedInDTO = served.current(&anew).await.json();
    assert_eq!(current.stage, stage::SIGNED_IN);
}

#[tokio::test]
async fn only_an_operator_marks_or_keeps_and_not_the_last_owner_of_a_shared_project() {
    // given: ada signed in, and grace the operator
    let served = served(|wired| wired.accounts.clone()).await;
    let ada_s = served.signed_in().await;
    let (grace, grace_s) = served.grace_the_operator().await;

    // expect: ada may neither mark nor keep grace's account
    refused_as(
        &served.marked_by(&ada_s, &grace).await,
        StatusCode::FORBIDDEN,
        refused::NOT_AN_OPERATOR,
    );
    refused_as(
        &served.kept_by(&ada_s, &grace).await,
        StatusCode::FORBIDDEN,
        refused::NOT_AN_OPERATOR,
    );
    // and: grace keeping an account nobody asked to delete is no failure, and changes nothing
    served
        .kept_by(&grace_s, &served.ada)
        .await
        .assert_status(StatusCode::NO_CONTENT);
    assert_eq!(served.listed_for(&grace_s, &served.ada).await.deletion, None);
    // and: nor mark ada while the last owner of a project another is in
    served
        .holding("project_shared", &served.ada, 1, InProject::Owner)
        .await;
    served
        .holding("project_shared", &grace, 2, InProject::Member)
        .await;
    refused_as(
        &served.marked_by(&grace_s, &served.ada).await,
        StatusCode::CONFLICT,
        refused::PROJECTS_TO_HAND_OVER,
    );
}

#[tokio::test]
async fn marking_or_keeping_from_a_version_the_account_moved_past_is_refused() {
    // given: grace the operator, and ada's account moved on since it stood at its first version
    let served = served(|wired| wired.accounts.clone()).await;
    let (_, grace_s) = served.grace_the_operator().await;

    // expect: marking it from its first version is refused as outdated
    refused_as(
        &served
            .server
            .put(&format!("/instance/accounts/{}/deletion", served.ada))
            .add_header(COOKIE, grace_s.clone())
            .add_header("if-match", "\"1\"")
            .await,
        StatusCode::PRECONDITION_FAILED,
        "outdated",
    );
    // and: once marked, keeping it from its first version too
    served
        .marked_by(&grace_s, &served.ada)
        .await
        .assert_status_ok();
    refused_as(
        &served
            .server
            .delete(&format!("/instance/accounts/{}/deletion", served.ada))
            .add_header(COOKIE, grace_s.clone())
            .add_header("if-match", "\"1\"")
            .await,
        StatusCode::PRECONDITION_FAILED,
        "outdated",
    );
}

#[tokio::test]
async fn an_account_swept_is_there_no_more_to_mark() {
    // given: ada's account asked to be deleted and swept on its day
    let served = served(|wired| wired.accounts.clone()).await;
    let (_, grace_s) = served.grace_the_operator().await;
    let ada_s = served.signed_in().await;
    let deletion: DeletionDTO = served.ask_deletion(&ada_s, HER_PASSWORD).await.json();
    served
        .wired
        .clock
        .set(text::read(&deletion.until).expect("a day the server told"));
    served
        .wired
        .chores
        .iter()
        .find(|chore| chore.named() == "sweep-deleted-accounts")
        .expect("accounts sweep those deleted")
        .carry_out()
        .await
        .expect("sweeping should succeed");

    // expect: marking it again finds no such account
    refused_as(
        &served.marked_by(&grace_s, &served.ada).await,
        StatusCode::NOT_FOUND,
        refused::ACCOUNT_NOT_FOUND,
    );
}
