mod mail_drop;

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use accounts_core::{KeyCeremonies, TotpKey};
use authorizing::{Act, Authorizer};
use axum::Router;
use axum::body;
use axum::body::Body;
use axum::http::header::ORIGIN;
use axum::http::{Request, StatusCode};
use axum_test::{TestServer, WsMessage};
use clock::SystemClock;
use eventsourcing::Agent;
use live_contract::SESSION_ENDED;
use outbox::Cadence;
use serde_json::{Value, json};
use tower::ServiceExt;
use weaveling_service_api::{Accounts, Adapters, Mailing, Relays, ServedClient, SignUp, Storage};

async fn an_app() -> Router {
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
    let response = an_app()
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
    // when: the health check is asked
    let (status, body) = get("/api/service/health").await;

    // then: it answers ok
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "ok");
}

#[tokio::test]
async fn the_projects_feature_is_mounted_under_api() {
    // when: the projects are listed under api
    let (status, body) = get("/api/projects").await;

    // then: an empty list is answered
    assert_eq!(status, StatusCode::OK, "body was {body}");
    assert_eq!(body, "[]");
}

fn a_built_client() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "weaveling-app-client-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("after the epoch")
            .as_nanos()
    ));
    fs::create_dir_all(dir.join("pkg")).expect("the folder should be made");
    fs::write(
        dir.join("page.html"),
        "<html><head><title>Weaveling</title></head><body></body></html>",
    )
    .expect("the page should be written");
    fs::write(dir.join("pkg/weaveling.js"), "export default 1;").expect("a file should be written");

    dir
}

async fn handed_out(client: ServedClient, path: &str) -> (StatusCode, String) {
    let response = weaveling_service_api::serving_the_client(an_app().await, client)
        .oneshot(
            Request::builder()
                .uri(path)
                .body(Body::empty())
                .expect("should build the request"),
        )
        .await
        .expect("the app should respond");
    let status = response.status();
    let body = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("the body should be readable");

    (status, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test]
async fn the_client_is_handed_out_beside_the_api_its_page_at_every_address_of_its_own() {
    // given: a built client, handed out by the server
    let client = ServedClient {
        dir: a_built_client(),
        reloading: None,
    };

    // expect: its files are handed out as built
    assert_eq!(
        handed_out(client.clone(), "/pkg/weaveling.js").await,
        (StatusCode::OK, "export default 1;".to_owned())
    );
    // and: its page at the root and at any address of the client's own, unchanged
    for address in ["/", "/projects/a-book/write", "/account/security"] {
        let (status, page) = handed_out(client.clone(), address).await;
        assert_eq!(status, StatusCode::OK, "{address}");
        assert!(page.contains("<title>Weaveling</title>"), "{address}");
        assert!(!page.contains("<script>"), "{address}");
    }
    // and: the api still answers, and an address under it that is none stays not found
    assert_eq!(
        handed_out(client.clone(), "/api/service/health").await,
        (StatusCode::OK, "ok".to_owned())
    );
    assert_eq!(
        handed_out(client, "/api/no-such-thing").await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn while_cargo_leptos_watches_the_page_reloads_on_changes() {
    // given: a built client, handed out while cargo-leptos watches
    let client = ServedClient {
        dir: a_built_client(),
        reloading: Some(3001),
    };

    // when: the page is asked for
    let (status, page) = handed_out(client, "/outline").await;

    // then: it listens for changes on the port told
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("(3001);</script>"), "{page}");
}

#[tokio::test]
async fn an_address_under_the_api_that_is_none_is_not_found() {
    // when: an address under the api that answers nothing is asked
    let (status, _) = get("/api/no-such-thing").await;

    // then: it is not found
    assert_eq!(status, StatusCode::NOT_FOUND);
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
    // given: a server for each setting of accounts and signing up
    for (accounts, sign_up, said) in [
        (
            Accounts::On,
            SignUp::Open,
            json!({ "accounts": {
                "sign_up": "open",
                "operator_wanted": true,
                "can_set_up_totp": false,
                "can_add_keys": false,
            } }),
        ),
        (
            Accounts::On,
            SignUp::ByInvitation,
            json!({ "accounts": {
                "sign_up": "invitation",
                "operator_wanted": true,
                "can_set_up_totp": false,
                "can_add_keys": false,
            } }),
        ),
        (
            Accounts::Off,
            SignUp::ByInvitation,
            json!({ "accounts": null }),
        ),
    ] {
        let server = TestServer::new(weaveling_service_api::app(
            Adapters::in_memory(accounts, Arc::new(SystemClock))
                .await
                .signing_up(sign_up),
        ));

        // when: its mode is asked
        let answered = server.get("/api/service/mode").await;

        // then: it says whether it keeps accounts and who may sign up
        answered.assert_status_ok();
        assert_eq!(
            answered.json::<Value>(),
            said,
            "without accounts nobody signs up, so the way in is not told"
        );
    }
}

#[tokio::test]
async fn the_service_says_whether_totp_can_be_set_up_and_keys_added() {
    // given: a server holding a TOTP key and told where it is served
    let server = TestServer::new(weaveling_service_api::app(
        Adapters::in_memory(Accounts::On, Arc::new(SystemClock))
            .await
            .sealing_totp_secrets(
                TotpKey::from_hex(
                    "7e57000000000000000000000000000000000000000000000000000000007e57",
                )
                .expect("a test key"),
            )
            .holding_key_ceremonies(
                KeyCeremonies::serving_at("https://weaveling.example.com").expect("a public URL"),
            ),
    ));

    // when: its mode is asked
    let answered = server.get("/api/service/mode").await;

    // then: it says both can be done
    answered.assert_status_ok();
    let accounts = &answered.json::<Value>()["accounts"];
    assert_eq!(accounts["can_set_up_totp"], true);
    assert_eq!(accounts["can_add_keys"], true);
}

#[tokio::test]
async fn a_server_signing_up_by_invitation_refuses_a_plain_sign_up_and_takes_links() {
    // given: a server signing up by invitation
    let server = TestServer::new(weaveling_service_api::app(
        Adapters::in_memory(Accounts::On, Arc::new(SystemClock))
            .await
            .signing_up(SignUp::ByInvitation),
    ));

    // when: one signs up plainly and one by a link
    let plain = server
        .post("/api/accounts")
        .json(&json!({ "email": "ada@example.com", "name": "Ada", "password": "correct horse battery" }))
        .await;
    let by_link = server
        .post("/api/accounts/by-invitation")
        .json(&json!({ "token": "a token nobody was given", "name": "Ada", "password": "correct horse battery" }))
        .await;

    // then: the plain sign up is refused
    plain.assert_status(StatusCode::FORBIDDEN);
    assert_eq!(
        plain.json::<Value>()["type"],
        "signing-up-by-invitation-only"
    );
    // and: a link is weighed for itself
    by_link.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        by_link.json::<Value>()["type"],
        "invitation-invalid",
        "the way in by a link stands open, and only a link of its own lets anyone through"
    );
}

#[tokio::test]
async fn unless_told_otherwise_anyone_may_sign_up() {
    // given: a server with accounts told nothing about signing up
    let server = TestServer::new(weaveling_service_api::app(
        Adapters::in_memory(Accounts::On, Arc::new(SystemClock)).await,
    ));

    // then: anyone may sign up
    assert_eq!(
        server.get("/api/service/mode").await.json::<Value>()["accounts"]["sign_up"],
        "open"
    );
}

#[tokio::test]
async fn with_accounts_on_the_work_waits_behind_signing_in() {
    // given: a server with accounts
    let server = TestServer::new(weaveling_service_api::app(
        Adapters::in_memory(Accounts::On, Arc::new(SystemClock)).await,
    ));

    // when: the work is asked for without signing in
    let refused = server.get("/api/projects").await;
    // then: it is refused as not signed in
    refused.assert_status(StatusCode::UNAUTHORIZED);
    assert_eq!(refused.json::<Value>()["type"], "not-signed-in");
    // and: the health check still answers
    server
        .get("/api/service/health")
        .await
        .assert_status(StatusCode::OK);
    // and: neither the session nor the notifications answer
    server
        .get("/api/sessions/current")
        .await
        .assert_status(StatusCode::UNAUTHORIZED);
    server
        .get("/api/notifications")
        .await
        .assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn with_accounts_on_becoming_the_operator_waits_behind_signing_in_too() {
    // given: a server with accounts
    let server = TestServer::new(weaveling_service_api::app(
        Adapters::in_memory(Accounts::On, Arc::new(SystemClock)).await,
    ));

    // when: one asks to become the operator without signing in
    let refused = server
        .post("/api/operators")
        .json(&json!({ "code": "a guess" }))
        .await;

    // then: it is refused as not signed in
    refused.assert_status(StatusCode::UNAUTHORIZED);
    assert_eq!(refused.json::<Value>()["type"], "not-signed-in");
}

#[tokio::test]
async fn a_code_is_drawn_at_start_only_where_accounts_are_on() {
    // given: a service with accounts and one without
    let on = Adapters::in_memory(Accounts::On, Arc::new(SystemClock)).await;
    let off = Adapters::in_memory(Accounts::Off, Arc::new(SystemClock)).await;

    // then: only the one with accounts draws a code
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
    // given: a server without accounts
    let server = TestServer::new(weaveling_service_api::app(
        Adapters::in_memory(Accounts::Off, Arc::new(SystemClock)).await,
    ));

    // then: the work answers
    server
        .get("/api/projects")
        .await
        .assert_status(StatusCode::OK);
    // and: there are no notifications
    server
        .get("/api/notifications")
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn sign_ups_are_taken_only_where_accounts_are_on() {
    // expect: a sign up is taken only where accounts are on
    assert_eq!(signing_up(Accounts::On).await, StatusCode::ACCEPTED);
    assert_eq!(
        signing_up(Accounts::Off).await,
        StatusCode::NOT_FOUND,
        "whether there are accounts is said by the setting, not by where the work is kept"
    );
}

#[tokio::test]
async fn feature_routes_are_not_reachable_outside_the_api_prefix() {
    // when: a feature is asked for outside the api prefix
    let (status, _) = get("/projects").await;

    // then: it is not found
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn an_unknown_path_answers_404() {
    // when: an unknown path is asked for
    let (status, _) = get("/api/nope").await;

    // then: it is not found
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_scenes_feature_is_mounted_under_api() {
    // when: a scene is started under api
    let (status, body) = post_json("/api/scenes", r#"{"project":"project_1"}"#).await;

    // then: it is created
    assert_eq!(status, StatusCode::CREATED, "body was {body}");
    assert!(body.contains("scene_"), "body was {body}");
}

#[tokio::test]
async fn the_refusals_are_mounted_under_the_service() {
    // when: the refusals are listed under the service
    let (status, body) = get("/api/service/refusals").await;

    // then: an empty list is answered
    assert_eq!(status, StatusCode::OK, "body was {body}");
    assert_eq!(body, "[]");
}

#[tokio::test]
async fn there_is_no_socket_for_one_scene_any_more() {
    // when: a socket for one scene is asked for
    let (status, _) = get("/api/scenes/sync/weaveling").await;

    // then: it is not found
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "every scene travels on the one socket at /api/scenes/sync"
    );
}

#[tokio::test]
async fn the_one_socket_is_under_api() {
    // when: the one socket is asked for without a socket
    let (status, body) = get("/api/live").await;

    // then: it asks for an upgrade
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body.contains("upgrade"),
        "the live channel refuses a request that is not a socket: {body}"
    );
}

#[tokio::test]
async fn the_socket_is_found_under_the_feature_it_belongs_to() {
    // when: a socket is asked for outside its feature
    let (status, _) = get("/api/sync").await;

    // then: it is not found
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "every route starts with its feature, the scenes' socket too"
    );
}

#[tokio::test]
async fn the_two_features_do_not_shadow_each_other() {
    // when: the projects are listed and a scene started
    let (projects, _) = get("/api/projects").await;
    let (scenes, _) = post_json("/api/scenes", r#"{"project":"project_1"}"#).await;

    // then: each answers
    assert_eq!(projects, StatusCode::OK);
    assert_eq!(scenes, StatusCode::CREATED);
}

#[tokio::test]
async fn the_ideas_feature_is_mounted_under_api() {
    // when: an idea is captured under api
    let (status, body) = post_json(
        "/api/ideas",
        r#"{"project":"project_1","title":"The Loom"}"#,
    )
    .await;

    // then: it is created
    assert_eq!(status, StatusCode::CREATED, "body was {body}");
    assert!(body.contains("idea_"), "body was {body}");
}

#[tokio::test]
async fn the_ideas_listing_is_mounted_under_api() {
    // when: the ideas are listed under api
    let (status, body) = get("/api/ideas?project=project_1").await;

    // then: an empty list is answered
    assert_eq!(status, StatusCode::OK, "body was {body}");
    assert_eq!(body, "[]");
}

#[tokio::test]
async fn every_feature_answers_without_shadowing_the_others() {
    // when: each feature is asked
    let (projects, _) = get("/api/projects").await;
    let (scenes, _) = post_json("/api/scenes", r#"{"project":"project_1"}"#).await;
    let (ideas, _) = get("/api/ideas?project=project_1").await;

    // then: each answers
    assert_eq!(projects, StatusCode::OK);
    assert_eq!(scenes, StatusCode::CREATED);
    assert_eq!(ideas, StatusCode::OK);
}

#[tokio::test]
async fn the_live_scenes_are_settled_before_the_service_stops() {
    // given: a service in memory
    let adapters = Adapters::in_memory(Accounts::Off, Arc::new(SystemClock)).await;

    // then: its live scenes are settled before it stops
    assert_eq!(
        adapters.settling().len(),
        1,
        "scenes holds edits and counts in memory, and stopping without settling them loses them"
    );
}

#[tokio::test]
async fn the_socket_for_many_scenes_is_mounted_under_api() {
    // when: the socket for many scenes is asked for without a socket
    let (status, body) = get("/api/scenes/sync").await;

    // then: it asks for an upgrade
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body.contains("upgrade"),
        "the socket itself refuses a request that is not a socket, rather than a scene route          reading `sync` as a scene's id: {body}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn in_memory_a_captured_idea_travels_all_the_way_to_its_catalog() {
    // given: a project on a relayed server in memory
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

    // when: an idea is captured in it
    server
        .post("/api/ideas")
        .json(&json!({ "project": project, "title": "A girl in a wood" }))
        .await
        .assert_status(StatusCode::CREATED);

    // then: it reaches the catalog of ideas
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
    // given: a server with accounts mailing into a drop
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

    // when: an address signs up
    server
        .post("/api/accounts")
        .json(&json!({ "email": "ada@example.com", "name": "Ada", "password": "correct horse battery" }))
        .await
        .assert_status(StatusCode::ACCEPTED);

    // then: one mail with a code is left there for it
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
    // given: an operator signed in on a server signing up by invitation
    let signed_in =
        an_operator_signed_in_on_a_server_signing_up("first-operator", SignUp::ByInvitation).await;

    // when: its mode is asked
    let told: Value = signed_in.server.get("/api/service/mode").await.json();

    // then: no operator is wanted any more
    assert_eq!(
        told,
        json!({ "accounts": {
            "sign_up": "invitation",
            "operator_wanted": false,
            "can_set_up_totp": false,
            "can_add_keys": false,
        } }),
        "with an operator there, nobody is asked to set the instance up any more"
    );
    // and: an operator code signs nobody up
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
    // given: an operator signed in on a server
    let signed_in = an_operator_signed_in_on_a_server("instance").await;
    let server = &signed_in.server;

    // then: her account reaches the list of accounts as operator
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
    // given: an operator signed in on a server
    let signed_in = an_operator_signed_in_on_a_server("inviting").await;

    // when: she invites an address from a page
    signed_in
        .server
        .post("/api/invitations")
        .add_header(ORIGIN, "https://weaveling.example.com")
        .json(&json!({ "email": "grace@example.com" }))
        .await
        .assert_status(StatusCode::CREATED);

    // then: one link to that page is mailed to the address
    mail_drop::invitation_token_mailed_to(&signed_in.drop, "grace@example.com")
        .await
        .expect("the link should have been mailed, once the issue was heard");
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
    // given: an invitation mailed by an operator
    let signed_in = an_operator_signed_in_on_a_server("invited").await;
    signed_in
        .server
        .post("/api/invitations")
        .add_header(ORIGIN, "https://weaveling.example.com")
        .json(&json!({ "email": "grace@example.com" }))
        .await
        .assert_status(StatusCode::CREATED);
    let token = mail_drop::invitation_token_mailed_to(&signed_in.drop, "grace@example.com")
        .await
        .expect("the link should have been mailed");

    // when: its link is followed to sign up
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

    // then: the account is made
    assert_eq!(
        status,
        StatusCode::CREATED,
        "accounts hear of the invitation only through the outbox, its relay and their own listener"
    );
    // and: it may sign in
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
    // given: an operator signed in on a server
    let signed_in = an_operator_signed_in_on_a_server("listed").await;
    // when: she invites an address
    signed_in
        .server
        .post("/api/invitations")
        .add_header(ORIGIN, "https://weaveling.example.com")
        .json(&json!({ "email": "grace@example.com" }))
        .await
        .assert_status(StatusCode::CREATED);

    // then: the invitation is listed open
    let open = listed_once(&signed_in.server, |listed| {
        listed["invitations"]
            .as_array()
            .is_some_and(|all| !all.is_empty())
    })
    .await;
    assert_eq!(open["invitations"][0]["email"], "grace@example.com");
    assert_eq!(open["invitations"][0]["status"], "open");

    // when: its link is followed to sign up
    let token = mail_drop::invitation_token_mailed_to(&signed_in.drop, "grace@example.com")
        .await
        .expect("the link should have been mailed");
    signed_in
        .server
        .post("/api/accounts/by-invitation")
        .do_not_save_cookies()
        .json(&json!({ "token": token, "name": "Grace", "password": "another long password" }))
        .await
        .assert_status(StatusCode::CREATED);

    // then: it is listed no more
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
    // given: an operator signed in on a server
    let signed_in = an_operator_signed_in_on_a_server("declined").await;

    // when: she invites her own address
    signed_in
        .server
        .post("/api/invitations")
        .add_header(ORIGIN, "https://weaveling.example.com")
        .json(&json!({ "email": "ada@example.com" }))
        .await
        .assert_status(StatusCode::CREATED);

    // then: the invitation is listed declined, and why
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
    // given: a server with accounts and one without
    for (accounts, status) in [
        (Accounts::On, StatusCode::UNAUTHORIZED),
        (Accounts::Off, StatusCode::NOT_FOUND),
    ] {
        let server = TestServer::new(weaveling_service_api::app(
            Adapters::in_memory(accounts, Arc::new(SystemClock)).await,
        ));

        // then: making an invitation asks for signing in, or is not found without accounts
        server
            .post("/api/invitations")
            .add_header(ORIGIN, "https://weaveling.example.com")
            .json(&json!({ "email": "grace@example.com" }))
            .await
            .assert_status(status);
    }
}

#[tokio::test]
async fn an_invitation_is_turned_down_by_its_link_without_signing_in_where_there_are_accounts() {
    // given: a server with accounts and one without
    for (accounts, status) in [
        (Accounts::On, StatusCode::UNPROCESSABLE_ENTITY),
        (Accounts::Off, StatusCode::NOT_FOUND),
    ] {
        let server = TestServer::new(weaveling_service_api::app(
            Adapters::in_memory(accounts, Arc::new(SystemClock)).await,
        ));

        // then: turning down by a link is weighed without signing in, or not found without accounts
        server
            .post("/api/invitations/decline")
            .json(&json!({ "token": "a link nobody was given" }))
            .await
            .assert_status(status);
    }
}

#[tokio::test]
async fn the_server_does_the_invitations_and_notifications_chores_only_where_there_are_accounts() {
    // given: a service with accounts and one without
    for (accounts, retiring) in [(Accounts::On, true), (Accounts::Off, false)] {
        let adapters = Adapters::in_memory(accounts, Arc::new(SystemClock)).await;

        // when: its chores are named
        let named: Vec<&str> = adapters
            .chores()
            .iter()
            .map(|chore| chore.named())
            .collect();

        // then: only with accounts does it retire invitations and forget notices and hearings
        assert_eq!(named.contains(&"retire-due-invitations"), retiring);
        assert_eq!(named.contains(&"forget-old-notices"), retiring);
        assert_eq!(named.contains(&"forget-old-account-hearings"), retiring);
        assert_eq!(named.contains(&"sweep-deleted-accounts"), retiring);
        assert_eq!(named.contains(&"forget-old-account-tombstones"), retiring);
        assert_eq!(named.contains(&"forget-old-account-tries"), retiring);
        assert_eq!(named.contains(&"forget-old-invitation-tries"), retiring);
        // and: it forgets what dead letters told either way
        assert!(named.contains(&"forget-old-dead-letters"));
        // and: it forgets appearance tombstones either way
        assert!(
            named.contains(&"forget-old-appearance-tombstones"),
            "tombstones are laid with or without accounts"
        );
    }
}

#[tokio::test]
async fn without_accounts_the_one_local_person_may_do_everything_in_any_project() {
    // given: the authorizer of a service without accounts
    let authorizer = Adapters::in_memory(Accounts::Off, Arc::new(SystemClock))
        .await
        .authorizer();

    // then: the local person may do anything in any project
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
    // given: an operator signed in on a server
    let signed_in = an_operator_signed_in_on_a_server("authorizing").await;
    let ada: Value = signed_in.server.get("/api/sessions/current").await.json();
    let ada = Agent::Account(
        ada["account"]
            .as_str()
            .expect("the session names its account")
            .into(),
    );
    // when: she starts a project
    let started: Value = signed_in
        .server
        .post("/api/projects")
        .json(&json!({ "name": "Guarded" }))
        .await
        .json();
    let project = started["id"].as_str().expect("a project has an id");

    // then: she may manage it
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
    // and: an account not in it may not read it
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
    // given: an operator signed in on a server
    let signed_in = an_operator_signed_in_on_a_server("listing").await;
    let server = &signed_in.server;
    // when: she starts a project
    let started: Value = server
        .post("/api/projects")
        .json(&json!({ "name": "Ada's own" }))
        .await
        .json();

    // then: she lists it alone
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

    // when: another account signs in
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

    // then: it lists none
    let theirs: Value = server.get("/api/projects").await.json();
    assert_eq!(
        theirs,
        json!([]),
        "Grace is in no project, so she sees none, Ada's least of all"
    );

    signed_in.end().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn on_a_server_one_socket_tells_an_account_of_an_invitation_to_its_address_and_of_its_project()
 {
    // given: an operator following the invitations to her address
    let signed_in = an_operator_signed_in_on_a_server("one-socket").await;
    let server = &signed_in.server;
    let ada: Value = server.get("/api/sessions/current").await.json();
    let mut socket = server
        .get_websocket("/api/live")
        .await
        .into_websocket()
        .await;
    let mut known = false;
    for _ in 0..50 {
        socket
            .send_json(&json!({ "do": "subscribe", "id": "bell", "topic": "invitations", "about": { "mine": "true" } }))
            .await;
        let told = tokio::time::timeout(Duration::from_millis(200), socket.receive_json::<Value>())
            .await
            .expect("told whether it is followed");
        known = told["told"] == "following";
        if known {
            break;
        }
    }
    assert!(
        known,
        "invitations hear her sign-up through the outbox, and know her address"
    );
    // and: following her projects
    socket
        .send_json(&json!({ "do": "subscribe", "id": "projects", "topic": "members", "about": { "mine": "true" } }))
        .await;
    let followed: Value = tokio::time::timeout(Duration::from_secs(1), socket.receive_json())
        .await
        .expect("told it is followed");
    assert_eq!(followed, json!({ "told": "following", "id": "projects" }));

    // when: she starts a project
    let started: Value = server
        .post("/api/projects")
        .json(&json!({ "name": "Shared on one socket" }))
        .await
        .json();
    // then: its founding is told on the socket
    let founded: Value = serde_json::from_str(
        &tokio::time::timeout(Duration::from_secs(5), socket.receive_text())
            .await
            .expect("the founding is told while it is fresh"),
    )
    .expect("told as JSON");
    assert_eq!(founded["id"], "projects");
    assert_eq!(founded["frame"]["of"], started["id"]);
    assert_eq!(founded["frame"]["event"]["name"], "FOUNDED");

    // when: she is invited into it
    let issued: Value = server
        .post("/api/invitations")
        .add_header(ORIGIN, "https://weaveling.example.com")
        .json(&json!({
            "email": ada["email"],
            "scope": { "kind": "project", "project": started["id"], "role": "reader" },
        }))
        .await
        .json();
    // then: the invitation is told on the same socket
    let invited: Value = serde_json::from_str(
        &tokio::time::timeout(Duration::from_secs(5), socket.receive_text())
            .await
            .expect("the invitation is told while it is fresh"),
    )
    .expect("told as JSON");
    assert_eq!(invited["id"], "bell");
    assert_eq!(invited["frame"]["of"], issued["id"]);

    signed_in.end().await;
}

#[tokio::test]
async fn the_one_socket_knows_every_topic_a_project_is_followed_by() {
    // given: a socket to a server
    let server = TestServer::builder()
        .http_transport()
        .build(weaveling_service_api::app(
            Adapters::in_memory(Accounts::Off, Arc::new(SystemClock)).await,
        ));
    let mut socket = server
        .get_websocket("/api/live")
        .await
        .into_websocket()
        .await;

    // when: every topic a project is followed by is subscribed to, and one nobody registered
    for (topic, about) in [
        ("projects", json!({ "mine": "true" })),
        ("projects", json!({ "project": "project_1" })),
        ("boards", json!({ "board": "board_1" })),
        ("outlines", json!({ "outline": "outline_1" })),
        ("ideas", json!({ "project": "project_1" })),
        ("appearances", json!({ "idea": "idea_1" })),
        ("scenes", json!({ "project": "project_1" })),
        ("quotation-restylings", json!({ "project": "project_1" })),
        ("word-counts", json!({ "project": "project_1" })),
        ("nobody-registered-this", json!({})),
    ] {
        socket
            .send_json(&json!({ "do": "subscribe", "id": topic, "topic": topic, "about": about }))
            .await;
    }

    // then: only the one nobody registered is unknown
    let mut unknown = Vec::new();
    while let Ok(told) =
        tokio::time::timeout(Duration::from_millis(500), socket.receive_json::<Value>()).await
    {
        if told["why"] == "unknown-topic" {
            unknown.push(told["id"].clone());
        }
    }
    assert_eq!(
        unknown,
        vec![json!("nobody-registered-this")],
        "every other is known"
    );
}

#[tokio::test]
async fn the_one_socket_is_there_only_behind_signing_in_where_there_are_accounts() {
    // given: a server with accounts and one without
    for (accounts, status) in [
        (Accounts::On, StatusCode::UNAUTHORIZED),
        (Accounts::Off, StatusCode::SWITCHING_PROTOCOLS),
    ] {
        let server = TestServer::builder()
            .http_transport()
            .build(weaveling_service_api::app(
                Adapters::in_memory(accounts, Arc::new(SystemClock)).await,
            ));

        // then: the socket asks for signing in where there are accounts and opens where there are none
        assert_eq!(
            server.get_websocket("/api/live").await.status_code(),
            status
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn on_a_server_a_project_is_not_there_for_an_account_not_in_it() {
    // given: a project its owner reads
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

    // when: another account signs in
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

    // then: it neither reads, renames nor deletes the project
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
    // and: neither lists nor captures its ideas
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
    // and: neither follows the project nor its ideas
    let mut socket = server
        .get_websocket("/api/live")
        .await
        .into_websocket()
        .await;
    for topic in ["projects", "ideas"] {
        socket
            .send_json(&json!({ "do": "subscribe", "id": topic, "topic": topic, "about": { "project": project } }))
            .await;
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), socket.receive_json::<Value>())
                .await
                .expect("told at once"),
            json!({ "told": "ended", "id": topic, "why": "not-there" })
        );
    }

    signed_in.end().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn on_a_server_signing_out_closes_the_one_socket_and_the_editing_socket_as_ended() {
    // given: an operator signed in on both sockets
    let signed_in = an_operator_signed_in_on_a_server("signing-out").await;
    let mut live = signed_in
        .server
        .get_websocket("/api/live")
        .await
        .into_websocket()
        .await;
    let mut editing = signed_in
        .server
        .get_websocket("/api/scenes/sync")
        .await
        .into_websocket()
        .await;

    // when: she signs out
    signed_in
        .server
        .delete("/api/sessions/current")
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: both close as ended
    for socket in [&mut live, &mut editing] {
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(1), socket.receive_message()).await,
            Ok(WsMessage::Close(Some(frame))) if u16::from(frame.code) == SESSION_ENDED
        ));
    }
    signed_in.end().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn on_a_server_a_member_whose_role_was_changed_finds_it_in_their_notifications() {
    // given: an operator managing a project
    let signed_in = an_operator_signed_in_on_a_server("notified").await;
    let server = &signed_in.server;
    let ada: Value = server.get("/api/sessions/current").await.json();
    let ada = ada["account"]
        .as_str()
        .expect("the session names its account")
        .to_owned();
    let started: Value = server
        .post("/api/projects")
        .json(&json!({ "name": "Shared" }))
        .await
        .json();
    let project = started["id"].as_str().expect("an id").to_owned();
    for _ in 0..1_000 {
        if signed_in
            .authorizer
            .may(&Agent::Account(ada.as_str().into()), Act::Manage, &project)
            .await
            .expect("asking should succeed")
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    // and: an account that joined it as author
    server
        .post("/api/invitations")
        .add_header(ORIGIN, "https://weaveling.example.com")
        .json(&json!({ "email": "grace@example.com", "scope": { "kind": "project", "project": project, "role": "author" } }))
        .await
        .assert_status(StatusCode::CREATED);
    let token = mail_drop::invitation_token_mailed_to(&signed_in.drop, "grace@example.com")
        .await
        .expect("the link should have been mailed");
    server
        .post("/api/accounts/by-invitation")
        .do_not_save_cookies()
        .json(&json!({ "token": token, "name": "Grace", "password": "another long password" }))
        .await
        .assert_status(StatusCode::CREATED);
    let mut grace = None;
    for _ in 0..1_000 {
        let members: Value = server
            .get(&format!("/api/members?project={project}"))
            .await
            .json();
        grace = members
            .as_array()
            .and_then(|all| all.iter().find(|member| member["role"] == "author"))
            .and_then(|member| member["account"].as_str())
            .map(str::to_owned);
        if grace.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let grace = grace.expect("Grace joins the project she was invited into");

    // when: its role is changed to reader
    server
        .patch(&format!("/api/members/{project}/accounts/{grace}"))
        .json(&json!({ "role": "reader" }))
        .await
        .assert_status_ok();
    // and: it signs in
    server
        .post("/api/sessions")
        .json(&json!({ "email": "grace@example.com", "password": "another long password" }))
        .await
        .assert_status_ok();

    // then: it finds the change in its notifications
    let mut notices = Value::Null;
    for _ in 0..1_000 {
        notices = server.get("/api/notifications").await.json();
        if notices[0]["names"]
            .as_object()
            .is_some_and(|names| names.len() == 2)
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        notices[0]["about"],
        json!({ "kind": "role-changed", "project": project, "role": "reader", "by": ada }),
        "the change is relayed, noted for Grace, listed, and found on coming in"
    );
    // and: the notice names who changed it and the project
    assert_eq!(
        notices[0]["names"],
        json!({ ada: "Ada", project: "Shared" }),
        "who changed it and the project, by the names notifications heard"
    );

    signed_in.end().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn on_a_server_an_account_follows_its_notifications_on_the_one_socket() {
    // given: a socket of a signed in account
    let signed_in = an_operator_signed_in_on_a_server("notifications-live").await;
    let mut socket = signed_in
        .server
        .get_websocket("/api/live")
        .await
        .into_websocket()
        .await;

    // when: it follows its notifications
    socket
        .send_json(&json!({ "do": "subscribe", "id": "bell", "topic": "notifications", "about": { "mine": "true" } }))
        .await;

    // then: it is told it follows them
    let told: Value = tokio::time::timeout(Duration::from_secs(1), socket.receive_json())
        .await
        .expect("told whether it is followed");
    assert_eq!(told, json!({ "told": "following", "id": "bell" }));
    // and: nothing else is told yet
    assert!(
        tokio::time::timeout(Duration::from_millis(300), socket.receive_json::<Value>())
            .await
            .is_err(),
        "followed, neither unknown nor not there, and nothing to tell yet"
    );
    signed_in.end().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn with_accounts_on_only_an_operator_reaches_the_refusals() {
    // given: an operator signed in on a server
    let signed_in = an_operator_signed_in_on_a_server("refusals").await;
    let server = &signed_in.server;

    // expect: the operator is answered the refusals
    let listed = server.get("/api/service/refusals").await;
    listed.assert_status_ok();
    assert_eq!(listed.text(), "[]");
    // and: nobody signed in is refused, for each of the refusals' routes
    for refused in [
        server
            .get("/api/service/refusals")
            .do_not_save_cookies()
            .clear_cookies()
            .await,
        server
            .post("/api/service/refusals/1/acknowledge")
            .do_not_save_cookies()
            .clear_cookies()
            .await,
        server
            .post("/api/service/refusals/1/retry")
            .do_not_save_cookies()
            .clear_cookies()
            .await,
    ] {
        refused.assert_status(StatusCode::UNAUTHORIZED);
    }

    // when: another account signs up and signs in
    server
        .post("/api/accounts")
        .do_not_save_cookies()
        .json(&json!({ "email": "grace@example.com", "name": "Grace", "password": "another long password" }))
        .await
        .assert_status(StatusCode::ACCEPTED);
    server
        .post("/api/accounts/verification")
        .do_not_save_cookies()
        .json(&json!({ "email": "grace@example.com", "code": mail_drop::code_dropped_for(&signed_in.drop, "grace@example.com") }))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    let grace = server
        .post("/api/sessions")
        .do_not_save_cookies()
        .json(&json!({ "email": "grace@example.com", "password": "another long password" }))
        .await;
    grace.assert_status_ok();
    let grace_cookie = grace
        .header("set-cookie")
        .to_str()
        .expect("a cookie is plain text")
        .split(';')
        .next()
        .expect("the cookie names the session")
        .to_owned();

    // then: she is refused the refusals, as no operator
    let refused = server
        .get("/api/service/refusals")
        .do_not_save_cookies()
        .clear_cookies()
        .add_header("cookie", grace_cookie.clone())
        .await;
    refused.assert_status(StatusCode::FORBIDDEN);
    assert_eq!(refused.json::<Value>()["type"], "not-an-operator");

    // when: each follows the refusals live
    let followed = |cookie: Option<String>| async move {
        let asked = server.get_websocket("/api/live");
        let asked = match cookie {
            Some(cookie) => asked
                .do_not_save_cookies()
                .clear_cookies()
                .add_header("cookie", cookie),
            None => asked,
        };
        let mut socket = asked.await.into_websocket().await;
        socket
            .send_json(
                &json!({ "do": "subscribe", "id": "refusals", "topic": "refusals", "about": {} }),
            )
            .await;

        tokio::time::timeout(Duration::from_secs(1), socket.receive_json::<Value>())
            .await
            .expect("told whether it is followed")
    };

    // then: the operator follows them
    assert_eq!(
        followed(None).await,
        json!({ "told": "following", "id": "refusals" })
    );
    // and: she is refused, as for something not there
    assert_eq!(
        followed(Some(grace_cookie.clone())).await,
        json!({ "told": "ended", "id": "refusals", "why": "not-there" })
    );

    signed_in.end().await;
}
