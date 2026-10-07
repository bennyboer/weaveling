#![cfg(feature = "postgres")]

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use axum_test::TestServer;
use clock::{Clock, SystemClock};
use serde_json::{Value, json};
use sqlx::PgPool;
use test_harness::PostgresFixture;
use weaveling_service_api::{Adapters, Storage, Unprepared, app};
use wiring::Databases;

const DATABASES: [&str; 7] = [
    "messaging",
    "projects",
    "ideas",
    "boards",
    "outline",
    "passages",
    "appearances",
];

#[derive(Clone)]
struct SchemaEach {
    pools: Arc<HashMap<&'static str, PgPool>>,
}

impl SchemaEach {
    fn of(&self, database: &str) -> &PgPool {
        self.pools
            .get(database)
            .unwrap_or_else(|| panic!("the fixture should have made a {database} schema"))
    }
}

#[async_trait]
impl Databases for SchemaEach {
    async fn ready(&self, feature: &str) -> Result<PgPool, Unprepared> {
        self.pools
            .get(feature)
            .cloned()
            .ok_or_else(|| Unprepared::Unreachable {
                database: feature.to_owned(),
                why: "the fixture made no schema for it".to_owned(),
            })
    }
}

struct Running {
    fixture: PostgresFixture,
    databases: SchemaEach,
    server: TestServer,
}

async fn a_schema_each(fixture: &PostgresFixture) -> SchemaEach {
    let mut pools = HashMap::new();
    for database in DATABASES {
        pools.insert(database, fixture.create_schema(database).await);
    }

    SchemaEach {
        pools: Arc::new(pools),
    }
}

async fn adapters_on(databases: &SchemaEach, clock: Arc<dyn Clock>) -> Adapters {
    Adapters::assembled(Storage::Postgres(Arc::new(databases.clone())), clock)
        .await
        .expect("every feature's schema should lay down")
}

async fn a_running_api() -> Running {
    let fixture = PostgresFixture::setup().await;
    let databases = a_schema_each(&fixture).await;
    let server = TestServer::new(app(adapters_on(&databases, Arc::new(SystemClock)).await));

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
            .fetch_all(running.databases.of("projects"))
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
async fn an_idea_captured_against_postgres_is_written_and_left_waiting_to_be_announced() {
    let running = a_running_api().await;
    let project = a_project(&running.server, "Capturing").await;

    let captured = running
        .server
        .post("/api/ideas")
        .json(&json!({ "project": project, "title": "A girl in a wood" }))
        .await;
    captured.assert_status(axum::http::StatusCode::CREATED);
    let id = captured.json::<Value>()["id"]
        .as_str()
        .expect("a captured idea should carry an id")
        .to_owned();

    let found = running.server.get(&format!("/api/ideas/{id}")).await;
    found.assert_status_ok();
    assert_eq!(
        found.json::<Value>()["title"].as_str(),
        Some("A girl in a wood"),
        "reading an aggregate replays its stream, so this needs no projection"
    );

    let waiting: Vec<String> =
        sqlx::query_scalar("SELECT routing_key FROM outbox WHERE published_at IS NULL")
            .fetch_all(running.databases.of("ideas"))
            .await
            .expect("reading the outbox should succeed");

    assert_eq!(
        waiting,
        vec!["idea.captured".to_owned()],
        "on PostgreSQL the store enqueues and a relay publishes, so the message waits here"
    );

    running.cleanup().await;
}

#[tokio::test]
async fn what_was_written_survives_a_second_api_built_on_the_same_databases() {
    let fixture = PostgresFixture::setup().await;
    let databases = a_schema_each(&fixture).await;

    let first = TestServer::new(app(adapters_on(&databases, Arc::new(SystemClock)).await));
    let id = a_project(&first, "Outliving").await;
    drop(first);

    let second = TestServer::new(app(adapters_on(&databases, Arc::new(SystemClock)).await));
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
        .post("/api/ideas")
        .json(&json!({ "project": project, "title": "A girl in a wood" }))
        .await
        .assert_status(axum::http::StatusCode::CREATED);

    for (feature, table, counting, pool) in [
        (
            "projects",
            "events",
            "SELECT count(*) FROM events",
            running.databases.of("projects"),
        ),
        (
            "projects",
            "outbox",
            "SELECT count(*) FROM outbox",
            running.databases.of("projects"),
        ),
        (
            "ideas",
            "events",
            "SELECT count(*) FROM events",
            running.databases.of("ideas"),
        ),
        (
            "ideas",
            "outbox",
            "SELECT count(*) FROM outbox",
            running.databases.of("ideas"),
        ),
    ] {
        let held: i64 = sqlx::query_scalar(counting)
            .fetch_one(pool)
            .await
            .expect("counting should succeed");

        assert!(held > 0, "{feature} should have written into its {table}");
    }

    let elsewhere: i64 = sqlx::query_scalar("SELECT count(*) FROM events")
        .fetch_one(running.databases.of("boards"))
        .await
        .expect("counting should succeed");

    assert_eq!(
        elsewhere, 0,
        "capturing an idea must not write into another feature's database"
    );

    running.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_relay_carries_what_was_captured_all_the_way_to_its_catalog() {
    use outbox::Cadence;
    use weaveling_service_api::Relays;

    let fixture = PostgresFixture::setup().await;
    let databases = a_schema_each(&fixture).await;
    let clock = Arc::new(SystemClock);
    let adapters = adapters_on(&databases, clock).await;
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
        .post("/api/ideas")
        .json(&json!({ "project": project, "title": "A girl in a wood" }))
        .await
        .assert_status(axum::http::StatusCode::CREATED);

    let mut titles = Vec::new();
    for _ in 0..200 {
        titles = server
            .get(&format!("/api/ideas?project={project}"))
            .await
            .json::<Value>()
            .as_array()
            .expect("a listing is an array")
            .iter()
            .filter_map(|idea| idea["title"].as_str().map(ToOwned::to_owned))
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
    use outbox::Cadence;
    use weaveling_service_api::Relays;

    let fixture = PostgresFixture::setup().await;
    let databases = a_schema_each(&fixture).await;
    let clock = Arc::new(SystemClock);
    let adapters = adapters_on(&databases, clock).await;
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
        .post("/api/ideas")
        .json(&json!({ "project": project, "title": "A girl in a wood" }))
        .await;
    captured.assert_status(axum::http::StatusCode::CREATED);

    let made = server
        .post("/api/passages")
        .json(&json!({ "project": project }))
        .await;
    made.assert_status(axum::http::StatusCode::CREATED);
    let passage = made.json::<Value>()["id"]
        .as_str()
        .expect("a passage carries an id")
        .to_owned();
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
            !listed(&server, &format!("/api/ideas?project={project}"))
                .await
                .is_empty()
        })
        .await,
        "the idea has to be catalogued before deleting can be shown to sweep it away"
    );

    server
        .delete(&format!("/api/projects/{project}"))
        .await
        .assert_status(axum::http::StatusCode::NO_CONTENT);

    assert!(
        until(|| async {
            listed(&server, &format!("/api/ideas?project={project}"))
                .await
                .is_empty()
        })
        .await,
        "a deleted project must not leave its ideas behind; with a durable store an orphan \
         outlives the session rather than dying with the process"
    );

    assert!(
        until(|| async {
            let boards: i64 = sqlx::query_scalar("SELECT count(*) FROM board_summaries")
                .fetch_one(databases.of("boards"))
                .await
                .expect("counting should succeed");
            let outlines: i64 = sqlx::query_scalar("SELECT count(*) FROM outline_summaries")
                .fetch_one(databases.of("outline"))
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
        "the CRDT store holds the only copy of the prose, and nothing points at this passage \
         but its own project column, so the project's sweep is the only thing that can reach it"
    );

    let stuck: Vec<(String, String)> = sqlx::query_as("SELECT listener, why FROM dead_letters")
        .fetch_all(databases.of("messaging"))
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

#[tokio::test(flavor = "multi_thread")]
async fn discarding_an_idea_unlinks_it_from_every_passage() {
    use outbox::Cadence;
    use weaveling_service_api::Relays;

    let fixture = PostgresFixture::setup().await;
    let databases = a_schema_each(&fixture).await;
    let clock = Arc::new(SystemClock);
    let adapters = adapters_on(&databases, clock).await;
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

    let project = a_project(&server, "Linking").await;
    let captured = server
        .post("/api/ideas")
        .json(&json!({ "project": project, "title": "A girl in a wood" }))
        .await;
    captured.assert_status(axum::http::StatusCode::CREATED);
    let idea = captured.json::<Value>()["id"]
        .as_str()
        .expect("a captured idea carries an id")
        .to_owned();
    let made = server
        .post("/api/passages")
        .json(&json!({ "project": project }))
        .await;
    made.assert_status(axum::http::StatusCode::CREATED);
    let passage = made.json::<Value>()["id"]
        .as_str()
        .expect("a passage carries an id")
        .to_owned();
    server
        .post(&format!("/api/passages/{passage}/ideas"))
        .json(&json!({ "idea": idea }))
        .await
        .assert_status_ok();

    server
        .delete(&format!("/api/ideas/{idea}"))
        .await
        .assert_status(axum::http::StatusCode::NO_CONTENT);

    assert!(
        until(|| async {
            server
                .get(&format!("/api/passages/{passage}"))
                .await
                .json::<Value>()["ideas"]
                .as_array()
                .is_some_and(Vec::is_empty)
        })
        .await,
        "the passage owns the link, so only a passages listener hearing the discard can drop it"
    );
    server
        .get(&format!("/api/passages/{passage}"))
        .await
        .assert_status_ok();

    relays.stop().await;
    fixture.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn linking_an_idea_is_announced_and_relayed() {
    use outbox::Cadence;
    use weaveling_service_api::Relays;

    let fixture = PostgresFixture::setup().await;
    let databases = a_schema_each(&fixture).await;
    let clock = Arc::new(SystemClock);
    let adapters = adapters_on(&databases, clock).await;
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

    let project = a_project(&server, "Announcing").await;
    let made = server
        .post("/api/passages")
        .json(&json!({ "project": project }))
        .await;
    made.assert_status(axum::http::StatusCode::CREATED);
    let passage = made.json::<Value>()["id"]
        .as_str()
        .expect("a passage carries an id")
        .to_owned();

    server
        .post(&format!("/api/passages/{passage}/ideas"))
        .json(&json!({ "idea": "idea_1" }))
        .await
        .assert_status_ok();

    assert!(
        until(|| async {
            let relayed: Vec<String> = sqlx::query_scalar(
                "SELECT routing_key FROM outbox WHERE published_at IS NOT NULL ORDER BY entry",
            )
            .fetch_all(databases.of("passages"))
            .await
            .expect("reading the passages outbox should succeed");

            relayed == vec!["passage.idea.linked".to_owned()]
        })
        .await,
        "the link is written and announced in one transaction, and the passages outbox is relayed like every other"
    );

    relays.stop().await;
    fixture.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_idea_appears_wherever_it_was_noted_or_linked_until_discarded() {
    use outbox::Cadence;
    use weaveling_service_api::Relays;

    let fixture = PostgresFixture::setup().await;
    let databases = a_schema_each(&fixture).await;
    let adapters = adapters_on(&databases, Arc::new(SystemClock)).await;
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

    let project = a_project(&server, "Appearing").await;
    let captured = server
        .post("/api/ideas")
        .json(&json!({ "project": project, "title": "A girl in a wood" }))
        .await;
    captured.assert_status(axum::http::StatusCode::CREATED);
    let idea = captured.json::<Value>()["id"]
        .as_str()
        .expect("a captured idea carries an id")
        .to_owned();
    let opened = server
        .post("/api/outlines")
        .json(&json!({ "project": project }))
        .await;
    opened.assert_status_ok();
    let outline = opened.json::<Value>()["id"]
        .as_str()
        .expect("an outline carries an id")
        .to_owned();
    let added = server
        .post(&format!("/api/outlines/{outline}/sections"))
        .json(&json!({ "under": null, "after": null, "title": "Chapter one" }))
        .await;
    added.assert_status(axum::http::StatusCode::CREATED);
    let section = added.json::<Value>()["section"]
        .as_str()
        .expect("an added section carries an id")
        .to_owned();
    server
        .post(&format!("/api/outlines/{outline}/attachments"))
        .json(&json!({
            "attachment": { "kind": "idea", "id": idea },
            "section": section,
            "after": null,
        }))
        .await
        .assert_status_ok();
    let made = server
        .post("/api/passages")
        .json(&json!({ "project": project }))
        .await;
    made.assert_status(axum::http::StatusCode::CREATED);
    let passage = made.json::<Value>()["id"]
        .as_str()
        .expect("a passage carries an id")
        .to_owned();
    server
        .post(&format!("/api/passages/{passage}/ideas"))
        .json(&json!({ "idea": idea }))
        .await
        .assert_status_ok();

    let appearances = format!("/api/appearances?idea={idea}");
    assert!(
        until(|| async {
            let mut places: Vec<Value> = server.get(&appearances).await.json();
            places.sort_by_key(|place| place["type"].to_string());

            places
                == vec![
                    json!({ "type": "passage", "id": passage }),
                    json!({ "type": "section", "id": section }),
                ]
        })
        .await,
        "appearances hear both the outline and passages, each through its own outbox"
    );

    server
        .delete(&format!("/api/ideas/{idea}"))
        .await
        .assert_status(axum::http::StatusCode::NO_CONTENT);

    assert!(
        until(|| async {
            server
                .get(&appearances)
                .await
                .json::<Vec<Value>>()
                .is_empty()
        })
        .await,
        "a discarded idea appears nowhere, however many places it was in"
    );

    relays.stop().await;
    fixture.cleanup().await;
}
