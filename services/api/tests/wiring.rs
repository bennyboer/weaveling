mod mail_drop;

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use authorizing::{Act, Authorizer};
use axum::Router;
use axum::body;
use axum::body::Body;
use axum::http::header::ORIGIN;
use axum::http::{Request, StatusCode};
use axum_test::TestServer;
use clock::SystemClock;
use eventsourcing::Agent;
use outbox::Cadence;
use serde_json::{Value, json};
use tower::ServiceExt;
use weaveling_service_api::{Accounts, Adapters, Mailing, Relays, SignUp, Storage};

async fn new_app() -> Router {
    weaveling_service_api::app(Adapters::in_memory(Accounts::Off, Arc::new(SystemClock)).await)
}

async fn get(path: &str) -> (StatusCode, String) {
    reply(
        Request::builder()
            .uri(path)
            .body(Body::empty())
            .expect("should build the request"),
    )
    .await
}

async fn post_json(path: &str, body: &str) -> (StatusCode, String) {
    reply(
        Request::builder()
            .method("POST")
            .uri(path)
            .header("content-type", "application/json")
            .body(Body::from(body.to_owned()))
            .expect("should build the request"),
    )
    .await
}

async fn reply(request: Request<Body>) -> (StatusCode, String) {
    let response = new_app()
        .await
        .oneshot(request)
        .await
        .expect("the app should respond");
    let status = response.status();
    let body = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("the body should be readable");

    (status, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test]
async fn the_health_check_answers_ok() {
    let (status, body) = get("/api/service/health").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "ok");
}

#[tokio::test]
async fn the_projects_feature_is_mounted_under_api() {
    let (status, body) = get("/api/projects").await;

    assert_eq!(status, StatusCode::OK, "body was {body}");
    assert_eq!(body, "[]");
}

async fn signing_up(accounts: Accounts) -> StatusCode {
    let server = TestServer::new(weaveling_service_api::app(
        Adapters::in_memory(accounts, Arc::new(SystemClock)).await,
    ));

    server
        .post("/api/accounts")
        .json(&json!({ "email": "ada@example.com", "name": "Ada", "password": "correct horse battery" }))
        .await
        .status_code()
}

#[tokio::test]
async fn the_service_says_whether_it_keeps_accounts_and_who_may_sign_up() {
    for (accounts, sign_up, said) in [
        (
            Accounts::On,
            SignUp::Open,
            json!({ "accounts": true, "sign_up": "open", "operator_wanted": true }),
        ),
        (
            Accounts::On,
            SignUp::ByInvitation,
            json!({ "accounts": true, "sign_up": "invitation", "operator_wanted": true }),
        ),
        (
            Accounts::Off,
            SignUp::ByInvitation,
            json!({ "accounts": false }),
        ),
    ] {
        let server = TestServer::new(weaveling_service_api::app(
            Adapters::in_memory(accounts, Arc::new(SystemClock))
                .await
                .signing_up(sign_up),
        ));

        let answered = server.get("/api/service/mode").await;

        answered.assert_status_ok();
        assert_eq!(
            answered.json::<Value>(),
            said,
            "without accounts nobody signs up, so the way in is not told"
        );
    }
}

#[tokio::test]
async fn a_server_signing_up_by_invitation_refuses_a_plain_sign_up_and_takes_links() {
    let server = TestServer::new(weaveling_service_api::app(
        Adapters::in_memory(Accounts::On, Arc::new(SystemClock))
            .await
            .signing_up(SignUp::ByInvitation),
    ));

    let plain = server
        .post("/api/accounts")
        .json(&json!({ "email": "ada@example.com", "name": "Ada", "password": "correct horse battery" }))
        .await;
    let by_link = server
        .post("/api/accounts/by-invitation")
        .json(&json!({ "token": "a token nobody was given", "name": "Ada", "password": "correct horse battery" }))
        .await;

    plain.assert_status(StatusCode::FORBIDDEN);
    assert_eq!(
        plain.json::<Value>()["type"],
        "signing-up-by-invitation-only"
    );
    by_link.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        by_link.json::<Value>()["type"],
        "invitation-invalid",
        "the way in by a link stands open, and only a link of its own lets anyone through"
    );
}

#[tokio::test]
async fn unless_told_otherwise_anyone_may_sign_up() {
    let server = TestServer::new(weaveling_service_api::app(
        Adapters::in_memory(Accounts::On, Arc::new(SystemClock)).await,
    ));

    assert_eq!(
        server.get("/api/service/mode").await.json::<Value>()["sign_up"],
        "open"
    );
}

#[tokio::test]
async fn with_accounts_on_the_work_waits_behind_signing_in() {
    let server = TestServer::new(weaveling_service_api::app(
        Adapters::in_memory(Accounts::On, Arc::new(SystemClock)).await,
    ));

    let refused = server.get("/api/projects").await;
    refused.assert_status(StatusCode::UNAUTHORIZED);
    assert_eq!(refused.json::<Value>()["type"], "not-signed-in");
    server
        .get("/api/service/health")
        .await
        .assert_status(StatusCode::OK);
    server
        .get("/api/sessions/current")
        .await
        .assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn with_accounts_on_becoming_the_operator_waits_behind_signing_in_too() {
    let server = TestServer::new(weaveling_service_api::app(
        Adapters::in_memory(Accounts::On, Arc::new(SystemClock)).await,
    ));

    let refused = server
        .post("/api/operators")
        .json(&json!({ "code": "a guess" }))
        .await;

    refused.assert_status(StatusCode::UNAUTHORIZED);
    assert_eq!(refused.json::<Value>()["type"], "not-signed-in");
}

#[tokio::test]
async fn a_code_is_drawn_at_start_only_where_accounts_are_on() {
    let on = Adapters::in_memory(Accounts::On, Arc::new(SystemClock)).await;
    let off = Adapters::in_memory(Accounts::Off, Arc::new(SystemClock)).await;

    assert!(
        on.draw_operator_code()
            .await
            .expect("drawing should succeed")
            .is_some(),
        "a server with accounts and nobody appointed needs a way to an operator"
    );
    assert!(
        off.draw_operator_code()
            .await
            .expect("there is nothing to draw")
            .is_none(),
        "in local mode there is nobody to make an operator"
    );
}

#[tokio::test]
async fn with_accounts_off_the_work_is_done_by_the_one_local_person() {
    let server = TestServer::new(weaveling_service_api::app(
        Adapters::in_memory(Accounts::Off, Arc::new(SystemClock)).await,
    ));

    server
        .get("/api/projects")
        .await
        .assert_status(StatusCode::OK);
}

#[tokio::test]
async fn sign_ups_are_taken_only_where_accounts_are_on() {
    assert_eq!(signing_up(Accounts::On).await, StatusCode::ACCEPTED);
    assert_eq!(
        signing_up(Accounts::Off).await,
        StatusCode::NOT_FOUND,
        "whether there are accounts is said by the setting, not by where the work is kept"
    );
}

#[tokio::test]
async fn feature_routes_are_not_reachable_outside_the_api_prefix() {
    let (status, _) = get("/projects").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn an_unknown_path_answers_404() {
    let (status, _) = get("/api/nope").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_scenes_feature_is_mounted_under_api() {
    let (status, body) = post_json("/api/scenes", r#"{"project":"project_1"}"#).await;

    assert_eq!(status, StatusCode::CREATED, "body was {body}");
    assert!(body.contains("scene_"), "body was {body}");
}

#[tokio::test]
async fn the_refusals_are_mounted_under_the_service() {
    let (status, body) = get("/api/service/refusals").await;

    assert_eq!(status, StatusCode::OK, "body was {body}");
    assert_eq!(body, "[]");
}

#[tokio::test]
async fn there_is_no_socket_for_one_scene_any_more() {
    let (status, _) = get("/api/scenes/sync/weaveling").await;

    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "every scene travels on the one socket at /api/scenes/sync"
    );
}

#[tokio::test]
async fn the_outline_is_followed_live_under_api() {
    let (status, body) = get("/api/outlines/live?outline=outline_031VkO0hnpeQZUiAB7nDma").await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body.contains("upgrade"),
        "the live channel refuses a request that is not a socket: {body}"
    );
}

#[tokio::test]
async fn the_socket_is_found_under_the_feature_it_belongs_to() {
    let (status, _) = get("/api/sync").await;

    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "every route starts with its feature, the scenes' socket too"
    );
}

#[tokio::test]
async fn the_two_features_do_not_shadow_each_other() {
    let (projects, _) = get("/api/projects").await;
    let (scenes, _) = post_json("/api/scenes", r#"{"project":"project_1"}"#).await;

    assert_eq!(projects, StatusCode::OK);
    assert_eq!(scenes, StatusCode::CREATED);
}

#[tokio::test]
async fn the_ideas_feature_is_mounted_under_api() {
    let (status, body) = post_json(
        "/api/ideas",
        r#"{"project":"project_1","title":"The Loom"}"#,
    )
    .await;

    assert_eq!(status, StatusCode::CREATED, "body was {body}");
    assert!(body.contains("idea_"), "body was {body}");
}

#[tokio::test]
async fn the_ideas_listing_is_mounted_under_api() {
    let (status, body) = get("/api/ideas?project=project_1").await;

    assert_eq!(status, StatusCode::OK, "body was {body}");
    assert_eq!(body, "[]");
}

#[tokio::test]
async fn every_feature_answers_without_shadowing_the_others() {
    let (projects, _) = get("/api/projects").await;
    let (scenes, _) = post_json("/api/scenes", r#"{"project":"project_1"}"#).await;
    let (ideas, _) = get("/api/ideas?project=project_1").await;

    assert_eq!(projects, StatusCode::OK);
    assert_eq!(scenes, StatusCode::CREATED);
    assert_eq!(ideas, StatusCode::OK);
}

#[tokio::test]
async fn the_live_scenes_are_settled_before_the_service_stops() {
    let adapters = Adapters::in_memory(Accounts::Off, Arc::new(SystemClock)).await;

    assert_eq!(
        adapters.settling().len(),
        1,
        "scenes holds edits and counts in memory, and stopping without settling them loses them"
    );
}

#[tokio::test]
async fn the_socket_for_many_scenes_is_mounted_under_api() {
    let (status, body) = get("/api/scenes/sync").await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body.contains("upgrade"),
        "the socket itself refuses a request that is not a socket, rather than a scene route          reading `sync` as a scene's id: {body}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn in_memory_a_captured_idea_travels_all_the_way_to_its_catalog() {
    let adapters = Adapters::in_memory(Accounts::Off, Arc::new(SystemClock)).await;
    let relays = Relays::started(
        adapters.outboxes(),
        adapters.consuming(),
        Cadence {
            deliver_every: Duration::from_millis(10),
            ..Cadence::default()
        },
    );
    let server = TestServer::new(weaveling_service_api::app(adapters));
    let project = server
        .post("/api/projects")
        .json(&json!({ "name": "Kept in mind" }))
        .await
        .json::<Value>()["id"]
        .as_str()
        .expect("a project is made with an id")
        .to_owned();

    server
        .post("/api/ideas")
        .json(&json!({ "project": project, "title": "A girl in a wood" }))
        .await
        .assert_status(StatusCode::CREATED);

    let mut listed = Value::Null;
    for _ in 0..1_000 {
        listed = server
            .get(&format!("/api/ideas?project={project}"))
            .await
            .json();
        if listed.as_array().is_some_and(|ideas| !ideas.is_empty()) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        listed[0]["title"], "A girl in a wood",
        "in memory too, the catalog only hears of an idea through the outbox and its relay"
    );

    relays.stop().await;
}

#[tokio::test]
async fn a_server_with_a_mail_drop_leaves_each_code_there() {
    let drop = std::env::temp_dir().join(format!("weaveling-app-mail-drop-{}", std::process::id()));
    let _gone = fs::remove_dir_all(&drop);
    let mailing = Mailing::Dropped { into: drop.clone() };
    let server = TestServer::new(weaveling_service_api::app(
        Adapters::assembled(
            Storage::InMemory,
            Accounts::On,
            mailing.mailer(),
            Arc::new(SystemClock),
        )
        .await
        .expect("nothing kept in memory needs preparing"),
    ));

    server
        .post("/api/accounts")
        .json(&json!({ "email": "ada@example.com", "name": "Ada", "password": "correct horse battery" }))
        .await
        .assert_status(StatusCode::ACCEPTED);

    let dropped: Vec<Value> = fs::read_dir(&drop)
        .expect("the drop should have been made")
        .map(|entry| {
            serde_json::from_slice(&fs::read(entry.expect("an entry").path()).expect("readable"))
                .expect("a mail as JSON")
        })
        .collect();
    assert_eq!(dropped.len(), 1);
    assert_eq!(dropped[0]["to"], "ada@example.com");
    assert!(
        dropped[0]["body"].as_str().is_some_and(|body| body
            .split_whitespace()
            .any(|word| word.len() == 6 && word.bytes().all(|digit| digit.is_ascii_digit()))),
        "{}",
        dropped[0]
    );
    let _gone = fs::remove_dir_all(&drop);
}

struct OperatorSignedIn {
    server: TestServer,
    relays: Relays,
    drop: PathBuf,
    authorizer: Arc<dyn Authorizer>,
}

impl OperatorSignedIn {
    async fn end(self) {
        self.relays.stop().await;
        let _gone = fs::remove_dir_all(&self.drop);
    }
}

async fn an_operator_signed_in_on_a_server(named: &str) -> OperatorSignedIn {
    an_operator_signed_in_on_a_server_signing_up(named, SignUp::Open).await
}

async fn an_operator_signed_in_on_a_server_signing_up(
    named: &str,
    sign_up: SignUp,
) -> OperatorSignedIn {
    let drop = std::env::temp_dir().join(format!(
        "weaveling-{named}-mail-drop-{}",
        std::process::id()
    ));
    let _gone = fs::remove_dir_all(&drop);
    let adapters = Adapters::assembled(
        Storage::InMemory,
        Accounts::On,
        Mailing::Dropped { into: drop.clone() }.mailer(),
        Arc::new(SystemClock),
    )
    .await
    .expect("nothing kept in memory needs preparing")
    .signing_up(sign_up);
    let operator_code = adapters
        .draw_operator_code()
        .await
        .expect("drawing should succeed")
        .expect("with nobody appointed a code is drawn");
    let relays = Relays::started(
        adapters.outboxes(),
        adapters.consuming(),
        Cadence {
            deliver_every: Duration::from_millis(10),
            ..Cadence::default()
        },
    );
    let authorizer = adapters.authorizer();
    let server = TestServer::builder()
        .http_transport()
        .save_cookies()
        .build(weaveling_service_api::app(adapters));
    server
        .post("/api/accounts")
        .json(&json!({ "email": "ada@example.com", "name": "Ada", "password": "correct horse battery", "operator_code": operator_code.as_str() }))
        .await
        .assert_status(StatusCode::ACCEPTED);
    server
        .post("/api/accounts/verification")
        .json(&json!({ "email": "ada@example.com", "code": mail_drop::code_dropped_for(&drop, "ada@example.com") }))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    server
        .post("/api/sessions")
        .json(&json!({ "email": "ada@example.com", "password": "correct horse battery" }))
        .await
        .assert_status_ok();
    server
        .post("/api/operators")
        .json(&json!({ "code": operator_code.as_str() }))
        .await
        .assert_status(StatusCode::NO_CONTENT);

    OperatorSignedIn {
        server,
        relays,
        drop,
        authorizer,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_signing_up_by_invitation_gets_its_first_operator_by_the_logged_code() {
    let signed_in =
        an_operator_signed_in_on_a_server_signing_up("first-operator", SignUp::ByInvitation).await;

    let told: Value = signed_in.server.get("/api/service/mode").await.json();

    assert_eq!(
        told,
        json!({ "accounts": true, "sign_up": "invitation" }),
        "with an operator there, nobody is asked to set the instance up any more"
    );
    signed_in
        .server
        .post("/api/accounts")
        .do_not_save_cookies()
        .json(&json!({ "email": "grace@example.com", "name": "Grace", "password": "another long password", "operator_code": "0123456789abcdef0123456789abcdef" }))
        .await
        .assert_status(StatusCode::FORBIDDEN);

    signed_in.end().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_account_signed_up_on_the_server_reaches_the_operators_list() {
    let signed_in = an_operator_signed_in_on_a_server("instance").await;
    let server = &signed_in.server;

    let mut listed = Value::Null;
    for _ in 0..1_000 {
        listed = server.get("/api/instance/accounts").await.json();
        if listed["accounts"][0]["operator"] == true {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(listed["accounts"][0]["email"], "ada@example.com");
    assert_eq!(
        listed["accounts"][0]["operator"], true,
        "the list only hears of an account through the outbox, its relay and the accounts' own listener"
    );

    signed_in.end().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_operator_signed_in_on_the_server_invites_and_the_link_goes_to_the_page_it_came_from() {
    let signed_in = an_operator_signed_in_on_a_server("inviting").await;

    signed_in
        .server
        .post("/api/invitations")
        .add_header(ORIGIN, "https://weaveling.example.com")
        .json(&json!({ "email": "grace@example.com" }))
        .await
        .assert_status(StatusCode::CREATED);

    let mailed: Vec<String> = fs::read_dir(&signed_in.drop)
        .expect("the drop should have been made")
        .map(|entry| fs::read_to_string(entry.expect("an entry").path()).expect("readable"))
        .filter(|mail| mail.contains("grace@example.com"))
        .collect();
    assert_eq!(mailed.len(), 1);
    assert!(
        mailed[0].contains("https://weaveling.example.com/invitation#"),
        "{}",
        mailed[0]
    );

    signed_in.end().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn whoever_follows_an_invitations_link_signs_up_and_in_at_once() {
    let signed_in = an_operator_signed_in_on_a_server("invited").await;
    signed_in
        .server
        .post("/api/invitations")
        .add_header(ORIGIN, "https://weaveling.example.com")
        .json(&json!({ "email": "grace@example.com" }))
        .await
        .assert_status(StatusCode::CREATED);
    let token = fs::read_dir(&signed_in.drop)
        .expect("the drop should have been made")
        .map(|entry| fs::read_to_string(entry.expect("an entry").path()).expect("readable"))
        .find(|mail| mail.contains("grace@example.com"))
        .and_then(|mail| {
            mail.split_once("/invitation#").map(|(_, rest)| {
                rest.chars()
                    .take_while(char::is_ascii_hexdigit)
                    .collect::<String>()
            })
        })
        .expect("the link should have been mailed");

    let mut status = StatusCode::IM_A_TEAPOT;
    for _ in 0..1_000 {
        status = signed_in
            .server
            .post("/api/accounts/by-invitation")
            .do_not_save_cookies()
            .json(&json!({ "token": token, "name": "Grace", "password": "another long password" }))
            .await
            .status_code();
        if status != StatusCode::UNPROCESSABLE_ENTITY {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    assert_eq!(
        status,
        StatusCode::CREATED,
        "accounts hear of the invitation only through the outbox, its relay and their own listener"
    );
    signed_in
        .server
        .post("/api/sessions")
        .do_not_save_cookies()
        .json(&json!({ "email": "grace@example.com", "password": "another long password" }))
        .await
        .assert_status_ok();

    signed_in.end().await;
}

async fn listed_once(server: &TestServer, settled: impl Fn(&Value) -> bool) -> Value {
    let mut listed = Value::Null;
    for _ in 0..1_000 {
        listed = server.get("/api/invitations").await.json();
        if settled(&listed) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    listed
}

#[tokio::test(flavor = "multi_thread")]
async fn an_invitation_is_listed_open_until_whoever_was_invited_signs_up() {
    let signed_in = an_operator_signed_in_on_a_server("listed").await;
    signed_in
        .server
        .post("/api/invitations")
        .add_header(ORIGIN, "https://weaveling.example.com")
        .json(&json!({ "email": "grace@example.com" }))
        .await
        .assert_status(StatusCode::CREATED);

    let open = listed_once(&signed_in.server, |listed| {
        listed["invitations"]
            .as_array()
            .is_some_and(|all| !all.is_empty())
    })
    .await;
    assert_eq!(open["invitations"][0]["email"], "grace@example.com");
    assert_eq!(open["invitations"][0]["status"], "open");

    let token = fs::read_dir(&signed_in.drop)
        .expect("the drop should have been made")
        .map(|entry| fs::read_to_string(entry.expect("an entry").path()).expect("readable"))
        .find(|mail| mail.contains("grace@example.com"))
        .and_then(|mail| {
            mail.split_once("/invitation#").map(|(_, rest)| {
                rest.chars()
                    .take_while(char::is_ascii_hexdigit)
                    .collect::<String>()
            })
        })
        .expect("the link should have been mailed");
    signed_in
        .server
        .post("/api/accounts/by-invitation")
        .do_not_save_cookies()
        .json(&json!({ "token": token, "name": "Grace", "password": "another long password" }))
        .await
        .assert_status(StatusCode::CREATED);

    let taken_up = listed_once(&signed_in.server, |listed| {
        listed["invitations"].as_array().is_some_and(Vec::is_empty)
    })
    .await;
    assert_eq!(
        taken_up["invitations"],
        json!([]),
        "accounts tell invitations it was taken up, and the list hears it from the invitation"
    );

    signed_in.end().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_invitation_to_an_address_that_has_an_account_is_listed_as_declined_and_why() {
    let signed_in = an_operator_signed_in_on_a_server("declined").await;

    signed_in
        .server
        .post("/api/invitations")
        .add_header(ORIGIN, "https://weaveling.example.com")
        .json(&json!({ "email": "ada@example.com" }))
        .await
        .assert_status(StatusCode::CREATED);

    let declined = listed_once(&signed_in.server, |listed| {
        listed["invitations"][0]["status"] == "declined"
    })
    .await;
    assert_eq!(declined["invitations"][0]["email"], "ada@example.com");
    assert_eq!(declined["invitations"][0]["status"], "declined");
    assert_eq!(
        declined["invitations"][0]["declined_for"], "address-has-account",
        "the operator's own address has an account, so accounts decline and invitations hear why"
    );

    signed_in.end().await;
}

#[tokio::test]
async fn invitations_are_made_only_where_there_are_accounts_and_only_behind_signing_in() {
    for (accounts, status) in [
        (Accounts::On, StatusCode::UNAUTHORIZED),
        (Accounts::Off, StatusCode::NOT_FOUND),
    ] {
        let server = TestServer::new(weaveling_service_api::app(
            Adapters::in_memory(accounts, Arc::new(SystemClock)).await,
        ));

        server
            .post("/api/invitations")
            .add_header(ORIGIN, "https://weaveling.example.com")
            .json(&json!({ "email": "grace@example.com" }))
            .await
            .assert_status(status);
    }
}

#[tokio::test]
async fn the_server_does_the_invitations_chores_only_where_there_are_accounts() {
    for (accounts, chores) in [
        (Accounts::On, vec!["retire-due-invitations"]),
        (Accounts::Off, vec![]),
    ] {
        let adapters = Adapters::in_memory(accounts, Arc::new(SystemClock)).await;

        assert_eq!(
            adapters
                .chores()
                .iter()
                .map(|chore| chore.named())
                .collect::<Vec<_>>(),
            chores
        );
    }
}

#[tokio::test]
async fn without_accounts_the_one_local_person_may_do_everything_in_any_project() {
    let authorizer = Adapters::in_memory(Accounts::Off, Arc::new(SystemClock))
        .await
        .authorizer();

    for act in [Act::Read, Act::Write, Act::Manage] {
        assert!(
            authorizer
                .may(&Agent::Local, act, "project_1")
                .await
                .expect("nothing to ask")
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn on_a_server_whoever_starts_a_project_may_manage_it_and_nobody_else_may_read_it() {
    let signed_in = an_operator_signed_in_on_a_server("authorizing").await;
    let ada: Value = signed_in.server.get("/api/sessions/current").await.json();
    let ada = Agent::Account(
        ada["account"]
            .as_str()
            .expect("the session names its account")
            .into(),
    );
    let started: Value = signed_in
        .server
        .post("/api/projects")
        .json(&json!({ "name": "Guarded" }))
        .await
        .json();
    let project = started["id"].as_str().expect("a project has an id");

    let mut managing = false;
    for _ in 0..1_000 {
        managing = signed_in
            .authorizer
            .may(&ada, Act::Manage, project)
            .await
            .expect("asking should succeed");
        if managing {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    assert!(
        managing,
        "the start is relayed, its members founded and listed, and the authorizer asks the list"
    );
    assert!(
        !signed_in
            .authorizer
            .may(&Agent::Account("account_grace".into()), Act::Read, project)
            .await
            .expect("asking should succeed")
    );
    signed_in.end().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn on_a_server_each_account_lists_only_the_projects_it_is_in() {
    let signed_in = an_operator_signed_in_on_a_server("listing").await;
    let server = &signed_in.server;
    let started: Value = server
        .post("/api/projects")
        .json(&json!({ "name": "Ada's own" }))
        .await
        .json();

    let mut listed = Value::Null;
    for _ in 0..1_000 {
        listed = server.get("/api/projects").await.json();
        if listed[0]["id"] == started["id"] {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(listed[0]["id"], started["id"]);
    assert_eq!(listed.as_array().map(Vec::len), Some(1));

    server
        .post("/api/accounts")
        .json(&json!({ "email": "grace@example.com", "name": "Grace", "password": "another long password" }))
        .await
        .assert_status(StatusCode::ACCEPTED);
    server
        .post("/api/accounts/verification")
        .json(&json!({ "email": "grace@example.com", "code": mail_drop::code_dropped_for(&signed_in.drop, "grace@example.com") }))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    server
        .post("/api/sessions")
        .json(&json!({ "email": "grace@example.com", "password": "another long password" }))
        .await
        .assert_status_ok();

    let theirs: Value = server.get("/api/projects").await.json();
    assert_eq!(
        theirs,
        json!([]),
        "Grace is in no project, so she sees none, Ada's least of all"
    );

    signed_in.end().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn on_a_server_whoever_starts_a_project_is_told_by_the_members_that_it_is_theirs() {
    let signed_in = an_operator_signed_in_on_a_server("following").await;
    let server = &signed_in.server;
    let ada: Value = server.get("/api/sessions/current").await.json();
    let mut socket = server
        .get_websocket("/api/members/live")
        .await
        .into_websocket()
        .await;

    let started: Value = server
        .post("/api/projects")
        .json(&json!({ "name": "Followed" }))
        .await
        .json();

    let mut founded = Value::Null;
    for _ in 0..10 {
        let frame: Value = serde_json::from_str(
            &tokio::time::timeout(Duration::from_secs(5), socket.receive_text())
                .await
                .expect("the change is told while it is fresh"),
        )
        .expect("a frame is JSON");
        if frame["event"]["name"] == "FOUNDED" {
            founded = frame;
            break;
        }
    }
    assert_eq!(founded["of"], started["id"]);
    assert_eq!(
        founded["event"]["owner"], ada["account"],
        "the members are founded by their own feature and told on their own channel, to her"
    );

    signed_in.end().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn on_a_server_a_project_is_not_there_for_an_account_not_in_it() {
    let signed_in = an_operator_signed_in_on_a_server("guarding").await;
    let server = &signed_in.server;
    let started: Value = server
        .post("/api/projects")
        .json(&json!({ "name": "Ada's own" }))
        .await
        .json();
    let at = format!("/api/projects/{}", started["id"].as_str().expect("an id"));

    let mut answered = StatusCode::NOT_FOUND;
    for _ in 0..1_000 {
        answered = server.get(&at).await.status_code();
        if answered == StatusCode::OK {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        answered,
        StatusCode::OK,
        "its owner reads it once the members have heard of her"
    );

    server
        .post("/api/accounts")
        .json(&json!({ "email": "grace@example.com", "name": "Grace", "password": "another long password" }))
        .await
        .assert_status(StatusCode::ACCEPTED);
    server
        .post("/api/accounts/verification")
        .json(&json!({ "email": "grace@example.com", "code": mail_drop::code_dropped_for(&signed_in.drop, "grace@example.com") }))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    server
        .post("/api/sessions")
        .json(&json!({ "email": "grace@example.com", "password": "another long password" }))
        .await
        .assert_status_ok();

    server.get(&at).await.assert_status(StatusCode::NOT_FOUND);
    server
        .patch(&at)
        .json(&json!({ "name": "Grace's now" }))
        .await
        .assert_status(StatusCode::NOT_FOUND);
    server
        .delete(&at)
        .await
        .assert_status(StatusCode::NOT_FOUND);
    server
        .get_websocket(&format!(
            "/api/projects/live?project={}",
            started["id"].as_str().expect("an id")
        ))
        .await
        .assert_status(StatusCode::NOT_FOUND);
    let project = started["id"].as_str().expect("an id");
    server
        .get(&format!("/api/ideas?project={project}"))
        .await
        .assert_status(StatusCode::NOT_FOUND);
    server
        .post("/api/ideas")
        .json(&json!({ "project": project, "title": "Grace's idea" }))
        .await
        .assert_status(StatusCode::NOT_FOUND);
    server
        .get_websocket(&format!("/api/ideas/live?project={project}"))
        .await
        .assert_status(StatusCode::NOT_FOUND);

    signed_in.end().await;
}
