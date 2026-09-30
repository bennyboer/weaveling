#![cfg(feature = "postgres")]

use std::sync::Arc;

use axum_test::TestServer;
use clock::SystemClock;
use serde_json::{Value, json};
use test_harness::PostgresFixture;
use weaveling_service_api::{Adapters, Databases, app};

struct Running {
    fixture: PostgresFixture,
    databases: Databases,
    server: TestServer,
}

async fn a_schema_each(fixture: &PostgresFixture) -> Databases {
    let databases = Databases {
        messaging: fixture.create_schema("messaging").await,
        projects: fixture.create_schema("projects").await,
        pieces: fixture.create_schema("pieces").await,
        boards: fixture.create_schema("boards").await,
        outline: fixture.create_schema("outline").await,
        passages: fixture.create_schema("passages").await,
    };
    databases
        .lay_out()
        .await
        .expect("every feature's schema should lay down");

    databases
}

async fn a_running_api() -> Running {
    let fixture = PostgresFixture::setup().await;
    let databases = a_schema_each(&fixture).await;
    let server = TestServer::new(app(Adapters::postgres(Arc::new(SystemClock), &databases)));

    Running {
        fixture,
        databases,
        server,
    }
}

impl Running {
    async fn cleanup(self) {
        self.fixture.cleanup().await;
    }
}

async fn a_project(server: &TestServer, named: &str) -> String {
    let made = server
        .post("/api/projects")
        .json(&json!({ "name": named }))
        .await;
    made.assert_status(axum::http::StatusCode::CREATED);

    made.json::<Value>()["id"]
        .as_str()
        .expect("a created project should carry an id")
        .to_owned()
}

#[tokio::test]
async fn a_project_written_to_postgres_is_read_back_and_left_waiting_to_be_announced() {
    let running = a_running_api().await;
    let id = a_project(&running.server, "The Weaver's Apprentice").await;

    let found = running.server.get(&format!("/api/projects/{id}")).await;
    found.assert_status_ok();
    assert_eq!(
        found.json::<Value>()["name"].as_str(),
        Some("The Weaver's Apprentice"),
        "reading an aggregate replays its stream, so this needs no projection"
    );

    let waiting: Vec<String> =
        sqlx::query_scalar("SELECT routing_key FROM outbox WHERE published_at IS NULL")
            .fetch_all(&running.databases.projects)
            .await
            .expect("reading the outbox should succeed");

    assert_eq!(
        waiting,
        vec!["project.started".to_owned()],
        "on PostgreSQL the store enqueues and a relay publishes, so the message waits here"
    );

    running.cleanup().await;
}

#[tokio::test]
async fn a_piece_captured_against_postgres_is_written_and_left_waiting_to_be_announced() {
    let running = a_running_api().await;
    let project = a_project(&running.server, "Capturing").await;

    let captured = running
        .server
        .post("/api/pieces")
        .json(&json!({ "project": project, "title": "A girl in a wood" }))
        .await;
    captured.assert_status(axum::http::StatusCode::CREATED);
    let id = captured.json::<Value>()["id"]
        .as_str()
        .expect("a captured piece should carry an id")
        .to_owned();

    let found = running.server.get(&format!("/api/pieces/{id}")).await;
    found.assert_status_ok();
    assert_eq!(
        found.json::<Value>()["title"].as_str(),
        Some("A girl in a wood"),
        "reading an aggregate replays its stream, so this needs no projection"
    );

    let waiting: Vec<String> =
        sqlx::query_scalar("SELECT routing_key FROM outbox WHERE published_at IS NULL")
            .fetch_all(&running.databases.pieces)
            .await
            .expect("reading the outbox should succeed");

    assert_eq!(
        waiting,
        vec!["piece.captured".to_owned()],
        "on PostgreSQL the store enqueues and a relay publishes, so the message waits here"
    );

    running.cleanup().await;
}

#[tokio::test]
async fn what_was_written_survives_a_second_api_built_on_the_same_databases() {
    let fixture = PostgresFixture::setup().await;
    let databases = a_schema_each(&fixture).await;

    let first = TestServer::new(app(Adapters::postgres(Arc::new(SystemClock), &databases)));
    let id = a_project(&first, "Outliving").await;
    drop(first);

    let second = TestServer::new(app(Adapters::postgres(Arc::new(SystemClock), &databases)));
    let found = second.get(&format!("/api/projects/{id}")).await;

    found.assert_status_ok();
    assert_eq!(
        found.json::<Value>()["name"].as_str(),
        Some("Outliving"),
        "an in-memory store would have forgotten this the moment the first server went away"
    );

    fixture.cleanup().await;
}

#[tokio::test]
async fn every_feature_keeps_its_rows_where_it_was_told_to() {
    let running = a_running_api().await;
    let project = a_project(&running.server, "Apart").await;

    running
        .server
        .post("/api/pieces")
        .json(&json!({ "project": project, "title": "A girl in a wood" }))
        .await
        .assert_status(axum::http::StatusCode::CREATED);

    for (feature, table, counting, pool) in [
        (
            "projects",
            "events",
            "SELECT count(*) FROM events",
            &running.databases.projects,
        ),
        (
            "projects",
            "outbox",
            "SELECT count(*) FROM outbox",
            &running.databases.projects,
        ),
        (
            "pieces",
            "events",
            "SELECT count(*) FROM events",
            &running.databases.pieces,
        ),
        (
            "pieces",
            "outbox",
            "SELECT count(*) FROM outbox",
            &running.databases.pieces,
        ),
    ] {
        let held: i64 = sqlx::query_scalar(counting)
            .fetch_one(pool)
            .await
            .expect("counting should succeed");

        assert!(held > 0, "{feature} should have written into its {table}");
    }

    let elsewhere: i64 = sqlx::query_scalar("SELECT count(*) FROM events")
        .fetch_one(&running.databases.boards)
        .await
        .expect("counting should succeed");

    assert_eq!(
        elsewhere, 0,
        "capturing a piece must not write into another feature's database"
    );

    running.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_relay_carries_what_was_captured_all_the_way_to_its_catalog() {
    use eventsourcing::Cadence;
    use weaveling_service_api::Relays;

    let fixture = PostgresFixture::setup().await;
    let databases = a_schema_each(&fixture).await;
    let clock = Arc::new(SystemClock);
    let adapters = Adapters::postgres(clock, &databases);
    let outboxes = adapters.outboxes();
    let consuming = adapters.consuming();
    let server = TestServer::new(app(adapters));

    let relays = Relays::started(
        outboxes,
        consuming,
        Cadence {
            deliver_every: std::time::Duration::from_millis(10),
            sweep_every: std::time::Duration::from_secs(3_600),
            deliver_at_most: 16,
            sweep_at_most: 16,
            kept_for: time::Duration::days(90),
        },
    );

    let project = a_project(&server, "Relaying").await;
    server
        .post("/api/pieces")
        .json(&json!({ "project": project, "title": "A girl in a wood" }))
        .await
        .assert_status(axum::http::StatusCode::CREATED);

    let mut titles = Vec::new();
    for _ in 0..200 {
        titles = server
            .get(&format!("/api/pieces?project={project}"))
            .await
            .json::<Value>()
            .as_array()
            .expect("a listing is an array")
            .iter()
            .filter_map(|piece| piece["title"].as_str().map(ToOwned::to_owned))
            .collect();

        if !titles.is_empty() {
            break;
        }

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    assert_eq!(
        titles,
        vec!["A girl in a wood".to_owned()],
        "the catalog is a projection, so this is the whole chain: append, outbox, relay, listener"
    );

    let mut names = Vec::new();
    for _ in 0..200 {
        names = server
            .get("/api/projects")
            .await
            .json::<Value>()
            .as_array()
            .expect("a listing is an array")
            .iter()
            .filter(|listed| listed["id"] == project.as_str())
            .filter_map(|listed| listed["name"].as_str().map(ToOwned::to_owned))
            .collect();

        if !names.is_empty() {
            break;
        }

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    assert_eq!(
        names,
        vec!["Relaying".to_owned()],
        "projects travel the same chain as everything else now, each on its own database"
    );

    relays.stop().await;
    fixture.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn deleting_a_project_sweeps_away_everything_it_held() {
    use eventsourcing::Cadence;
    use weaveling_service_api::Relays;

    let fixture = PostgresFixture::setup().await;
    let databases = a_schema_each(&fixture).await;
    let clock = Arc::new(SystemClock);
    let adapters = Adapters::postgres(clock, &databases);
    let outboxes = adapters.outboxes();
    let consuming = adapters.consuming();
    let server = TestServer::new(app(adapters));

    let relays = Relays::started(
        outboxes,
        consuming,
        Cadence {
            deliver_every: std::time::Duration::from_millis(10),
            sweep_every: std::time::Duration::from_secs(3_600),
            deliver_at_most: 16,
            sweep_at_most: 16,
            kept_for: time::Duration::days(90),
        },
    );

    let project = a_project(&server, "Doomed").await;
    let captured = server
        .post("/api/pieces")
        .json(&json!({ "project": project, "title": "A girl in a wood" }))
        .await;
    captured.assert_status(axum::http::StatusCode::CREATED);
    let piece = captured.json::<Value>()["id"]
        .as_str()
        .expect("a captured piece carries an id")
        .to_owned();

    let made = server.post("/api/passages").await;
    made.assert_status(axum::http::StatusCode::CREATED);
    let passage = made.json::<Value>()["id"]
        .as_str()
        .expect("a passage carries an id")
        .to_owned();
    server
        .put(&format!("/api/pieces/{piece}/passage"))
        .json(&json!({ "passage": passage }))
        .await
        .assert_status_ok();
    server
        .get(&format!("/api/passages/{passage}"))
        .await
        .assert_status_ok();

    server
        .post("/api/boards")
        .json(&json!({ "project": project }))
        .await
        .assert_status(axum::http::StatusCode::OK);
    server
        .post("/api/outlines")
        .json(&json!({ "project": project }))
        .await
        .assert_status(axum::http::StatusCode::OK);

    assert!(
        until(|| async {
            !listed(&server, &format!("/api/pieces?project={project}"))
                .await
                .is_empty()
        })
        .await,
        "the piece has to be catalogued before deleting can be shown to sweep it away"
    );

    server
        .delete(&format!("/api/projects/{project}"))
        .await
        .assert_status(axum::http::StatusCode::NO_CONTENT);

    assert!(
        until(|| async {
            listed(&server, &format!("/api/pieces?project={project}"))
                .await
                .is_empty()
        })
        .await,
        "a deleted project must not leave its pieces behind; with a durable store an orphan \
         outlives the session rather than dying with the process"
    );

    assert!(
        until(|| async {
            let boards: i64 = sqlx::query_scalar("SELECT count(*) FROM board_summaries")
                .fetch_one(&databases.boards)
                .await
                .expect("counting should succeed");
            let outlines: i64 = sqlx::query_scalar("SELECT count(*) FROM outline_summaries")
                .fetch_one(&databases.outline)
                .await
                .expect("counting should succeed");

            (boards, outlines) == (0, 0)
        })
        .await,
        "the board and the outline go with the project, each disposing its own"
    );

    assert!(
        until(|| async {
            server
                .get(&format!("/api/passages/{passage}"))
                .await
                .status_code()
                == axum::http::StatusCode::NOT_FOUND
        })
        .await,
        "the CRDT store holds the only copy of the prose, so a deleted project has to reach          it — the piece's own discard is what carries the passage across the seam"
    );

    let stuck: Vec<(String, String)> = sqlx::query_as("SELECT listener, why FROM dead_letters")
        .fetch_all(&databases.messaging)
        .await
        .expect("reading the dead letters should succeed");

    assert!(
        stuck.is_empty(),
        "a cascade that dead-letters has half finished, which is worse than not starting: {stuck:?}"
    );

    relays.stop().await;
    fixture.cleanup().await;
}

async fn listed(server: &TestServer, at: &str) -> Vec<String> {
    server
        .get(at)
        .await
        .json::<Value>()
        .as_array()
        .expect("a listing is an array")
        .iter()
        .filter_map(|held| held["id"].as_str().map(ToOwned::to_owned))
        .collect()
}

async fn until<F, Fut>(settled: F) -> bool
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    for _ in 0..200 {
        if settled().await {
            return true;
        }

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    false
}
