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

async fn five_schemas(fixture: &PostgresFixture) -> Databases {
    let databases = Databases {
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
    let databases = five_schemas(&fixture).await;
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
async fn a_project_written_to_postgres_is_listed_back() {
    let running = a_running_api().await;
    let id = a_project(&running.server, "The Weaver's Apprentice").await;

    let listed = running.server.get("/api/projects").await;
    listed.assert_status_ok();

    let found = listed.json::<Value>();
    let names: Vec<&str> = found
        .as_array()
        .expect("a listing is an array")
        .iter()
        .filter(|project| project["id"] == id.as_str())
        .filter_map(|project| project["name"].as_str())
        .collect();

    assert_eq!(names, vec!["The Weaver's Apprentice"]);

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
    let databases = five_schemas(&fixture).await;

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
            "projects",
            "SELECT count(*) FROM projects",
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
    let databases = five_schemas(&fixture).await;
    let clock = Arc::new(SystemClock);
    let adapters = Adapters::postgres(clock.clone(), &databases);
    let publisher = adapters.dispatcher.clone();
    let server = TestServer::new(app(adapters));

    let relays = Relays::started(
        &databases,
        publisher,
        clock,
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

    relays.stop().await;
    fixture.cleanup().await;
}
