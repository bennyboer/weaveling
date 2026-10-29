use std::fs;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use axum::http::StatusCode;
use axum::http::header::ORIGIN;
use axum_test::TestServer;
use clock::SystemClock;
use mailing::LoggingMailer;
use outbox::Cadence;
use serde_json::{Value, json};
use weaveling_service_api::{Accounts, Adapters, Mailing, Relays, Storage};

pub const DATABASES: [&str; 11] = [
    "messaging",
    "accounts",
    "invitations",
    "members",
    "projects",
    "ideas",
    "boards",
    "outline",
    "scenes",
    "appearances",
    "statistics",
];

#[async_trait]
pub trait Backend: Sized + Send + Sync {
    async fn setup() -> Self;

    fn storage(&self) -> Storage;

    async fn read(&self, database: &str, query: &'static str) -> Vec<String>;

    async fn count(&self, database: &str, query: &'static str) -> i64;

    async fn cleanup(self);
}

pub struct Journey<B: Backend> {
    pub backend: B,
    pub server: TestServer,
    relays: Option<Relays>,
    drop: Option<PathBuf>,
}

pub async fn a_server_on<B: Backend>(backend: &B) -> TestServer {
    TestServer::new(weaveling_service_api::app(
        adapters_on(backend, Accounts::Off).await,
    ))
}

async fn adapters_on<B: Backend>(backend: &B, accounts: Accounts) -> Adapters {
    Adapters::assembled(
        backend.storage(),
        accounts,
        Arc::new(LoggingMailer),
        Arc::new(SystemClock),
    )
    .await
    .expect("every feature's schema should lay down")
}

impl<B: Backend> Journey<B> {
    pub async fn unrelayed() -> Self {
        Self::unrelayed_with(Accounts::Off).await
    }

    pub async fn with_accounts() -> Self {
        Self::unrelayed_with(Accounts::On).await
    }

    async fn unrelayed_with(accounts: Accounts) -> Self {
        let backend = B::setup().await;
        let server = TestServer::new(weaveling_service_api::app(
            adapters_on(&backend, accounts).await,
        ));

        Self {
            backend,
            server,
            relays: None,
            drop: None,
        }
    }

    pub async fn relayed() -> Self {
        let backend = B::setup().await;
        let adapters = adapters_on(&backend, Accounts::Off).await;
        let relays = Relays::started(
            adapters.outboxes(),
            adapters.consuming(),
            Cadence {
                deliver_every: Duration::from_millis(10),
                ..Cadence::default()
            },
        );

        Self {
            backend,
            server: TestServer::new(weaveling_service_api::app(adapters)),
            relays: Some(relays),
            drop: None,
        }
    }

    pub async fn signed_in(named: &str) -> Self {
        let backend = B::setup().await;
        let drop = std::env::temp_dir().join(format!(
            "weaveling-journey-{named}-mail-drop-{}",
            std::process::id()
        ));
        let _gone = fs::remove_dir_all(&drop);
        let adapters = Adapters::assembled(
            backend.storage(),
            Accounts::On,
            Mailing::Dropped { into: drop.clone() }.mailer(),
            Arc::new(SystemClock),
        )
        .await
        .expect("every feature's schema should lay down");
        let relays = Relays::started(
            adapters.outboxes(),
            adapters.consuming(),
            Cadence {
                deliver_every: Duration::from_millis(10),
                ..Cadence::default()
            },
        );
        let mut server = TestServer::new(weaveling_service_api::app(adapters));
        server.save_cookies();
        server
            .post("/api/accounts")
            .json(&json!({ "email": "ada@example.com", "name": "Ada", "password": "correct horse battery" }))
            .await
            .assert_status(StatusCode::ACCEPTED);
        server
            .post("/api/accounts/verification")
            .json(&json!({ "email": "ada@example.com", "code": crate::mail_drop::code_dropped_for(&drop, "ada@example.com") }))
            .await
            .assert_status(StatusCode::NO_CONTENT);
        server
            .post("/api/sessions")
            .json(&json!({ "email": "ada@example.com", "password": "correct horse battery" }))
            .await
            .assert_status_ok();

        Self {
            backend,
            server,
            relays: Some(relays),
            drop: Some(drop),
        }
    }

    pub async fn end(self) {
        if let Some(relays) = self.relays {
            relays.stop().await;
        }
        if let Some(drop) = self.drop {
            let _gone = fs::remove_dir_all(&drop);
        }
        self.backend.cleanup().await;
    }

    pub async fn posted(&self, path: &str, body: Value, answered: StatusCode) -> Value {
        let response = self.server.post(path).json(&body).await;
        response.assert_status(answered);

        response.json()
    }

    pub async fn project(&self, named: &str) -> String {
        id_in(
            &self
                .posted(
                    "/api/projects",
                    json!({ "name": named }),
                    StatusCode::CREATED,
                )
                .await,
            "id",
        )
    }

    pub async fn idea(&self, project: &str, titled: &str) -> String {
        let captured = self
            .posted(
                "/api/ideas",
                json!({ "project": project, "title": titled }),
                StatusCode::CREATED,
            )
            .await;

        id_in(&captured, "id")
    }

    pub async fn scene(&self, project: &str) -> String {
        id_in(
            &self
                .posted(
                    "/api/scenes",
                    json!({ "project": project }),
                    StatusCode::CREATED,
                )
                .await,
            "id",
        )
    }

    pub async fn outline(&self, project: &str) -> String {
        id_in(
            &self
                .posted(
                    "/api/outlines",
                    json!({ "project": project }),
                    StatusCode::OK,
                )
                .await,
            "id",
        )
    }

    pub async fn section(&self, outline: &str, titled: &str) -> String {
        let added = self
            .posted(
                &format!("/api/outlines/{outline}/sections"),
                json!({ "title": titled, "under": null, "after": null }),
                StatusCode::CREATED,
            )
            .await;

        id_in(&added, "section")
    }

    pub async fn attach(&self, outline: &str, kind: &str, id: &str, section: &str) {
        self.posted(
            &format!("/api/outlines/{outline}/attachments"),
            json!({ "attachment": { "kind": kind, "id": id }, "section": section, "after": null }),
            StatusCode::OK,
        )
        .await;
    }

    pub async fn signed_in_as_another(&self, email: &str) {
        let drop = self
            .drop
            .as_ref()
            .expect("only a journey signed in has a mail drop");
        self.server
            .post("/api/accounts")
            .json(
                &json!({ "email": email, "name": "Another", "password": "another long password" }),
            )
            .await
            .assert_status(StatusCode::ACCEPTED);
        self.server
            .post("/api/accounts/verification")
            .json(
                &json!({ "email": email, "code": crate::mail_drop::code_dropped_for(drop, email) }),
            )
            .await
            .assert_status(StatusCode::NO_CONTENT);
        self.server
            .post("/api/sessions")
            .json(&json!({ "email": email, "password": "another long password" }))
            .await
            .assert_status_ok();
    }

    pub async fn link(&self, scene: &str, idea: &str) {
        self.posted(
            &format!("/api/scenes/{scene}/ideas"),
            json!({ "idea": idea }),
            StatusCode::OK,
        )
        .await;
    }

    pub async fn delete(&self, path: &str) {
        self.server
            .delete(path)
            .await
            .assert_status(StatusCode::NO_CONTENT);
    }

    pub async fn read(&self, path: &str) -> Value {
        self.server.get(path).await.json()
    }

    pub async fn listed(&self, path: &str) -> Vec<String> {
        self.read(path)
            .await
            .as_array()
            .expect("a listing is an array")
            .iter()
            .filter_map(|held| held["id"].as_str().map(ToOwned::to_owned))
            .collect()
    }
}

pub fn id_in(answer: &Value, key: &str) -> String {
    answer[key]
        .as_str()
        .unwrap_or_else(|| panic!("the answer should carry its {key}"))
        .to_owned()
}

pub async fn until<F, Fut>(settled: F) -> bool
where
    F: Fn() -> Fut,
    Fut: Future<Output = bool>,
{
    for _ in 0..1_000 {
        if settled().await {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    false
}

const WAITING: &str = "SELECT routing_key FROM outbox WHERE published_at IS NULL ORDER BY entry";
const RELAYED: &str =
    "SELECT routing_key FROM outbox WHERE published_at IS NOT NULL ORDER BY entry";

pub async fn a_project_is_read_back_and_left_waiting_to_be_announced<B: Backend>() {
    let journey = Journey::<B>::unrelayed().await;

    let id = journey.project("The Weaver's Apprentice").await;

    assert_eq!(
        journey.read(&format!("/api/projects/{id}")).await["name"],
        "The Weaver's Apprentice",
        "reading an aggregate replays its stream, so this needs no projection"
    );
    assert_eq!(
        journey.backend.read("projects", WAITING).await,
        ["project.started"],
        "the store enqueues and a relay publishes, and with no relay running the message waits here"
    );
    journey.end().await;
}

pub async fn a_captured_idea_is_read_back_and_left_waiting_to_be_announced<B: Backend>() {
    let journey = Journey::<B>::unrelayed().await;
    let project = journey.project("Capturing").await;

    let id = journey.idea(&project, "A girl in a wood").await;

    assert_eq!(
        journey.read(&format!("/api/ideas/{id}")).await["title"],
        "A girl in a wood",
        "reading an aggregate replays its stream, so this needs no projection"
    );
    assert_eq!(
        journey.backend.read("ideas", WAITING).await,
        ["idea.captured"]
    );
    journey.end().await;
}

pub async fn what_was_written_survives_a_second_api_built_on_the_same_databases<B: Backend>() {
    let backend = B::setup().await;
    let first = a_server_on(&backend).await;
    let id = id_in(
        &first
            .post("/api/projects")
            .json(&json!({ "name": "Outliving" }))
            .await
            .json(),
        "id",
    );
    drop(first);

    let second = a_server_on(&backend).await;

    assert_eq!(
        second
            .get(&format!("/api/projects/{id}"))
            .await
            .json::<Value>()["name"],
        "Outliving",
        "an in-memory store would have forgotten this the moment the first server went away"
    );
    backend.cleanup().await;
}

pub async fn every_feature_keeps_its_rows_where_it_was_told_to<B: Backend>() {
    let journey = Journey::<B>::unrelayed().await;
    let project = journey.project("Apart").await;

    journey.idea(&project, "A girl in a wood").await;

    for (feature, counting) in [
        ("projects", "SELECT count(*) FROM events"),
        ("projects", "SELECT count(*) FROM outbox"),
        ("ideas", "SELECT count(*) FROM events"),
        ("ideas", "SELECT count(*) FROM outbox"),
    ] {
        let held = journey.backend.count(feature, counting).await;
        assert!(held > 0, "{feature} should have written: {counting}");
    }
    assert_eq!(
        journey
            .backend
            .count("boards", "SELECT count(*) FROM events")
            .await,
        0,
        "capturing an idea must not write into another feature's database"
    );
    journey.end().await;
}

pub async fn a_relay_carries_what_was_captured_all_the_way_to_its_catalog<B: Backend>() {
    let journey = Journey::<B>::relayed().await;
    let project = journey.project("Relaying").await;

    journey.idea(&project, "A girl in a wood").await;

    assert!(
        until(|| async {
            journey.read(&format!("/api/ideas?project={project}")).await[0]["title"]
                == "A girl in a wood"
        })
        .await,
        "the catalog is a projection, so this is the whole chain: append, outbox, relay, listener"
    );
    assert!(
        until(|| async {
            let projects = journey.read("/api/projects").await;
            projects.as_array().is_some_and(|all| {
                all.iter()
                    .any(|listed| listed["id"] == project.as_str() && listed["name"] == "Relaying")
            })
        })
        .await,
        "projects travel the same chain as everything else, each on its own database"
    );
    journey.end().await;
}

pub async fn deleting_a_project_sweeps_away_everything_it_held<B: Backend>() {
    let journey = Journey::<B>::relayed().await;
    let project = journey.project("Doomed").await;
    journey.idea(&project, "A girl in a wood").await;
    let scene = journey.scene(&project).await;
    journey
        .posted("/api/boards", json!({ "project": project }), StatusCode::OK)
        .await;
    journey.outline(&project).await;
    let ideas = format!("/api/ideas?project={project}");
    assert!(
        until(|| async { !journey.listed(&ideas).await.is_empty() }).await,
        "the idea has to be catalogued before deleting can be shown to sweep it away"
    );

    journey.delete(&format!("/api/projects/{project}")).await;

    assert!(
        until(|| async { journey.listed(&ideas).await.is_empty() }).await,
        "a deleted project must not leave its ideas behind; with a durable store an orphan \
         outlives the session rather than dying with the process"
    );
    assert!(
        until(|| async {
            journey
                .backend
                .count("boards", "SELECT count(*) FROM board_summaries")
                .await
                == 0
                && journey
                    .backend
                    .count("outline", "SELECT count(*) FROM outline_summaries")
                    .await
                    == 0
        })
        .await,
        "the board and the outline go with the project, each disposing its own"
    );
    assert!(
        until(|| async {
            journey
                .server
                .get(&format!("/api/scenes/{scene}"))
                .await
                .status_code()
                == StatusCode::NOT_FOUND
        })
        .await,
        "the CRDT store holds the only copy of the prose, and nothing points at this scene \
         but its own project column, so the project's sweep is the only thing that can reach it"
    );
    let stuck = journey
        .backend
        .read(
            "messaging",
            "SELECT listener || ': ' || why FROM dead_letters",
        )
        .await;
    assert!(
        stuck.is_empty(),
        "a cascade that dead-letters has half finished, which is worse than not starting: {stuck:?}"
    );
    journey.end().await;
}

pub async fn discarding_an_idea_unlinks_it_from_every_scene<B: Backend>() {
    let journey = Journey::<B>::relayed().await;
    let project = journey.project("Linking").await;
    let idea = journey.idea(&project, "A girl in a wood").await;
    let scene = journey.scene(&project).await;
    journey.link(&scene, &idea).await;

    journey.delete(&format!("/api/ideas/{idea}")).await;

    assert!(
        until(|| async {
            journey.read(&format!("/api/scenes/{scene}")).await["ideas"]
                .as_array()
                .is_some_and(Vec::is_empty)
        })
        .await,
        "the scene owns the link, so only a scenes listener hearing the discard can drop it"
    );
    journey.end().await;
}

pub async fn deleting_a_scene_takes_it_out_of_the_book<B: Backend>() {
    let journey = Journey::<B>::relayed().await;
    let project = journey.project("Unplacing").await;
    let outline = journey.outline(&project).await;
    let section = journey.section(&outline, "Part One").await;
    let scene = journey.scene(&project).await;
    journey.attach(&outline, "scene", &scene, &section).await;

    journey.delete(&format!("/api/scenes/{scene}")).await;

    assert!(
        until(|| async {
            journey.read(&format!("/api/outlines/{outline}")).await["sections"][0]["attachments"]
                .as_array()
                .is_some_and(Vec::is_empty)
        })
        .await,
        "the outline owns the placement, so only an outline listener hearing the deletion can let go"
    );
    journey.end().await;
}

pub async fn linking_an_idea_is_announced_and_relayed<B: Backend>() {
    let journey = Journey::<B>::relayed().await;
    let project = journey.project("Announcing").await;
    let scene = journey.scene(&project).await;

    journey.link(&scene, "idea_1").await;

    assert!(
        until(|| async {
            journey.backend.read("scenes", RELAYED).await == ["scene.created", "scene.idea.linked"]
        })
        .await,
        "the link is written and announced in one transaction, and the scenes outbox is relayed like every other"
    );
    journey.end().await;
}

pub async fn an_idea_appears_wherever_it_was_noted_or_linked_until_discarded<B: Backend>() {
    let journey = Journey::<B>::relayed().await;
    let project = journey.project("Appearing").await;
    let idea = journey.idea(&project, "A girl in a wood").await;
    let outline = journey.outline(&project).await;
    let section = journey.section(&outline, "Chapter one").await;
    journey.attach(&outline, "idea", &idea, &section).await;
    let scene = journey.scene(&project).await;

    journey.link(&scene, &idea).await;

    let appearances = format!("/api/appearances?idea={idea}");
    assert!(
        until(|| async {
            let mut places: Vec<Value> =
                serde_json::from_value(journey.read(&appearances).await).unwrap_or_default();
            places.sort_by_key(|place| place["type"].to_string());

            places
                == [
                    json!({ "type": "scene", "id": scene }),
                    json!({ "type": "section", "id": section }),
                ]
        })
        .await,
        "appearances hear both the outline and scenes, each through its own outbox"
    );

    journey.delete(&format!("/api/ideas/{idea}")).await;

    assert!(
        until(|| async {
            journey
                .read(&appearances)
                .await
                .as_array()
                .is_some_and(Vec::is_empty)
        })
        .await,
        "a discarded idea appears nowhere, however many places it was in"
    );
    journey.end().await;
}

pub async fn whoever_starts_a_project_owns_it<B: Backend>() {
    let journey = Journey::<B>::signed_in("owning").await;

    let project = journey.project("Owned").await;

    let owned = vec![format!("{project} owner")];
    let (backend, owned) = (&journey.backend, &owned);
    assert!(
        until(|| async move {
            backend
                .read("members", "SELECT project || ' ' || role FROM memberships")
                .await
                == *owned
        })
        .await,
        "the start is relayed, the members are founded by its listener, and the list hears of them"
    );
    journey.end().await;
}

pub async fn an_idea_s_appearances_are_there_only_for_whoever_may_read_its_project<B: Backend>() {
    let journey = Journey::<B>::signed_in("appearing").await;
    let project = journey.project("Appearing").await;
    let server = &journey.server;
    let at = format!("/api/projects/{project}");
    assert!(
        until(|| async { server.get(&at).await.status_code() == StatusCode::OK }).await,
        "its owner may read the project once the members have heard of her"
    );
    let idea = journey.idea(&project, "A girl in a wood").await;
    let scene = journey.scene(&project).await;

    journey.link(&scene, &idea).await;

    let asked = format!("/api/appearances?idea={idea}");
    let placed = json!([{ "type": "scene", "id": scene }]);
    let (asked, placed) = (&asked, &placed);
    assert!(
        until(|| async move { server.get(asked).await.json::<Value>() == *placed }).await,
        "the link is relayed with its project, and the appearances keep both"
    );
    journey.signed_in_as_another("grace@example.com").await;
    server.get(asked).await.assert_status(StatusCode::NOT_FOUND);
    journey.end().await;
}

pub async fn a_project_s_board_is_there_only_for_whoever_may_read_its_project<B: Backend>() {
    let journey = Journey::<B>::signed_in("pinning").await;
    let project = journey.project("Pinned").await;
    let server = &journey.server;
    let at = format!("/api/projects/{project}");
    assert!(
        until(|| async { server.get(&at).await.status_code() == StatusCode::OK }).await,
        "its owner may read the project once the members have heard of her"
    );
    let opened = journey
        .posted("/api/boards", json!({ "project": project }), StatusCode::OK)
        .await;
    let board = opened["id"].as_str().expect("a board has an id").to_owned();

    journey.signed_in_as_another("grace@example.com").await;

    server
        .post("/api/boards")
        .json(&json!({ "project": project }))
        .await
        .assert_status(StatusCode::NOT_FOUND);
    server
        .get(&format!("/api/boards/{board}"))
        .await
        .assert_status(StatusCode::NOT_FOUND);
    journey.end().await;
}

pub async fn a_project_s_outline_is_there_only_for_whoever_may_read_its_project<B: Backend>() {
    let journey = Journey::<B>::signed_in("outlining").await;
    let project = journey.project("Outlined").await;
    let server = &journey.server;
    let at = format!("/api/projects/{project}");
    assert!(
        until(|| async { server.get(&at).await.status_code() == StatusCode::OK }).await,
        "its owner may read the project once the members have heard of her"
    );
    let outline = journey.outline(&project).await;

    journey.signed_in_as_another("grace@example.com").await;

    server
        .post("/api/outlines")
        .json(&json!({ "project": project }))
        .await
        .assert_status(StatusCode::NOT_FOUND);
    server
        .get(&format!("/api/outlines/{outline}"))
        .await
        .assert_status(StatusCode::NOT_FOUND);
    journey.end().await;
}

pub async fn a_project_s_scenes_are_there_only_for_whoever_may_read_its_project<B: Backend>() {
    let journey = Journey::<B>::signed_in("writing").await;
    let project = journey.project("Written").await;
    let server = &journey.server;
    let at = format!("/api/projects/{project}");
    assert!(
        until(|| async { server.get(&at).await.status_code() == StatusCode::OK }).await,
        "its owner may read the project once the members have heard of her"
    );
    let scene = journey.scene(&project).await;

    journey.signed_in_as_another("grace@example.com").await;

    for path in [
        format!("/api/scenes?project={project}"),
        format!("/api/scenes/{scene}"),
    ] {
        server.get(&path).await.assert_status(StatusCode::NOT_FOUND);
    }
    server
        .post("/api/scenes")
        .json(&json!({ "project": project }))
        .await
        .assert_status(StatusCode::NOT_FOUND);
    journey.end().await;
}

pub async fn a_project_s_members_are_listed_for_whoever_may_read_it<B: Backend>() {
    let journey = Journey::<B>::signed_in("members").await;
    let project = journey.project("Shared").await;
    let server = &journey.server;
    let at = format!("/api/members?project={project}");
    assert!(
        until(|| async { server.get(&at).await.status_code() == StatusCode::OK }).await,
        "its owner may list the members once they have heard of her"
    );

    let listed: Value = server.get(&at).await.json();
    assert_eq!(listed.as_array().map(Vec::len), Some(1));
    assert_eq!(listed[0]["role"], "owner");

    journey.signed_in_as_another("grace@example.com").await;

    server.get(&at).await.assert_status(StatusCode::NOT_FOUND);
    journey.end().await;
}

pub async fn an_account_invited_into_a_project_joins_it_by_taking_the_invitation_up<B: Backend>() {
    let journey = Journey::<B>::signed_in("joining").await;
    let project = journey.project("Shared").await;
    let server = &journey.server;
    let at = format!("/api/members?project={project}");
    assert!(
        until(|| async { server.get(&at).await.status_code() == StatusCode::OK }).await,
        "its owner may invite once the members have heard of her"
    );
    server
        .post("/api/invitations")
        .add_header(ORIGIN, "https://weaveling.example.com")
        .json(&json!({
            "email": "grace@example.com",
            "scope": { "kind": "project", "project": project, "role": "author" },
        }))
        .await
        .assert_status(StatusCode::CREATED);
    let drop = journey
        .drop
        .as_ref()
        .expect("a journey signed in has a mail drop");
    let token = crate::mail_drop::invitation_token_dropped_for(drop, "grace@example.com")
        .expect("the link should have been mailed");
    journey.signed_in_as_another("grace@example.com").await;

    assert!(
        until(|| async {
            server
                .post("/api/accounts/current/invitations/accept")
                .json(&json!({ "token": token }))
                .await
                .status_code()
                == StatusCode::NO_CONTENT
        })
        .await,
        "accounts hear of the invitation only through the outbox, its relay and their own listener"
    );
    assert!(
        until(|| async {
            let listed: Value = server.get(&at).await.json();
            listed
                .as_array()
                .is_some_and(|members| members.iter().any(|member| member["role"] == "author"))
        })
        .await,
        "invitations accept what accounts took up, and members let her join as she was invited"
    );
    let listed: Value = server.get(&at).await.json();
    assert_eq!(
        listed,
        json!([
            { "account": listed[0]["account"], "role": "owner", "name": "Ada" },
            { "account": listed[1]["account"], "role": "author", "name": "Another" },
        ]),
        "members heard both sign-ups, so she sees whom she writes with by name, but no address"
    );
    journey.end().await;
}

pub async fn a_deleted_project_s_owner_neither_lists_its_members_nor_invites_into_it<B: Backend>() {
    let journey = Journey::<B>::signed_in("disbanding").await;
    let project = journey.project("Gone").await;
    let server = &journey.server;
    let at = format!("/api/members?project={project}");
    assert!(
        until(|| async { server.get(&at).await.status_code() == StatusCode::OK }).await,
        "its owner may list the members once they have heard of her"
    );

    journey.delete(&format!("/api/projects/{project}")).await;

    assert!(
        until(|| async { server.get(&at).await.status_code() == StatusCode::NOT_FOUND }).await,
        "the members hear the project deleted and let everyone go"
    );
    server
        .post("/api/invitations")
        .add_header(ORIGIN, "https://weaveling.example.com")
        .json(&json!({
            "email": "grace@example.com",
            "scope": { "kind": "project", "project": project, "role": "author" },
        }))
        .await
        .assert_status(StatusCode::NOT_FOUND);
    journey.end().await;
}

pub async fn an_account_is_told_in_the_application_what_it_is_invited_into_and_joins_there<
    B: Backend,
>() {
    let journey = Journey::<B>::signed_in("told").await;
    let project = journey.project("Shared").await;
    let server = &journey.server;
    let members = format!("/api/members?project={project}");
    assert!(
        until(|| async { server.get(&members).await.status_code() == StatusCode::OK }).await,
        "its owner may invite once the members have heard of her"
    );
    server
        .post("/api/invitations")
        .add_header(ORIGIN, "https://weaveling.example.com")
        .json(&json!({
            "email": "grace@example.com",
            "scope": { "kind": "project", "project": project, "role": "author" },
        }))
        .await
        .assert_status(StatusCode::CREATED);

    journey.signed_in_as_another("grace@example.com").await;

    assert!(
        until(|| async {
            let mine: Value = server.get("/api/invitations/mine").await.json();
            mine[0]["project_name"] == "Shared" && mine[0]["invited_by"] == "Ada"
        })
        .await,
        "invitations hear of her sign-up, the project's name and who invited, all through the outbox"
    );
    let mine: Value = server.get("/api/invitations/mine").await.json();
    assert_eq!(mine.as_array().map(Vec::len), Some(1));
    assert_eq!(mine[0]["project"], project);
    assert_eq!(mine[0]["role"], "author");

    server
        .post(&format!(
            "/api/accounts/current/invitations/{}/accept",
            mine[0]["id"].as_str().unwrap_or_default()
        ))
        .await
        .assert_status(StatusCode::NO_CONTENT);

    assert!(
        until(|| async {
            let listed: Value = server.get(&members).await.json();
            listed
                .as_array()
                .is_some_and(|members| members.iter().any(|member| member["role"] == "author"))
        })
        .await,
        "taken up in the application, no link followed, she joins as she was invited"
    );
    assert!(
        until(|| async {
            server
                .get("/api/invitations/mine")
                .await
                .json::<Value>()
                .as_array()
                .is_some_and(Vec::is_empty)
        })
        .await,
        "and is invited to it no more"
    );
    journey.end().await;
}

pub async fn an_account_turns_down_in_the_application_what_it_is_invited_into<B: Backend>() {
    let journey = Journey::<B>::signed_in("turned-down").await;
    let project = journey.project("Shared").await;
    let server = &journey.server;
    let members = format!("/api/members?project={project}");
    assert!(
        until(|| async { server.get(&members).await.status_code() == StatusCode::OK }).await,
        "its owner may invite once the members have heard of her"
    );
    server
        .post("/api/invitations")
        .add_header(ORIGIN, "https://weaveling.example.com")
        .json(&json!({
            "email": "grace@example.com",
            "scope": { "kind": "project", "project": project, "role": "author" },
        }))
        .await
        .assert_status(StatusCode::CREATED);

    journey.signed_in_as_another("grace@example.com").await;
    assert!(
        until(|| async {
            server
                .get("/api/invitations/mine")
                .await
                .json::<Value>()
                .as_array()
                .is_some_and(|mine| mine.len() == 1)
        })
        .await,
        "invitations hear of her sign-up"
    );
    let mine: Value = server.get("/api/invitations/mine").await.json();
    server
        .post(&format!(
            "/api/accounts/current/invitations/{}/decline",
            mine[0]["id"].as_str().unwrap_or_default()
        ))
        .await
        .assert_status(StatusCode::NO_CONTENT);

    assert!(
        until(|| async {
            server
                .get("/api/invitations/mine")
                .await
                .json::<Value>()
                .as_array()
                .is_some_and(Vec::is_empty)
        })
        .await,
        "turned down, she is invited to it no more"
    );
    server
        .post("/api/sessions")
        .json(&json!({ "email": "ada@example.com", "password": "correct horse battery" }))
        .await
        .assert_status_ok();
    let invited = format!("/api/invitations?project={project}");
    let listed: Value = server.get(&invited).await.json();
    assert_eq!(listed["invitations"][0]["email"], "grace@example.com");
    assert_eq!(listed["invitations"][0]["status"], "declined");
    assert_eq!(
        listed["invitations"][0]["declined_for"], "turned-down",
        "the owner sees why"
    );
    assert!(
        !server
            .get(&members)
            .await
            .json::<Value>()
            .as_array()
            .is_some_and(|members| members.iter().any(|member| member["role"] == "author")),
        "and nobody joined"
    );
    journey.end().await;
}

#[macro_export]
macro_rules! journeys {
    ($backend:ty) => {
        $crate::journeys!($backend:
            a_project_is_read_back_and_left_waiting_to_be_announced,
            a_captured_idea_is_read_back_and_left_waiting_to_be_announced,
            what_was_written_survives_a_second_api_built_on_the_same_databases,
            every_feature_keeps_its_rows_where_it_was_told_to,
            a_relay_carries_what_was_captured_all_the_way_to_its_catalog,
            deleting_a_project_sweeps_away_everything_it_held,
            discarding_an_idea_unlinks_it_from_every_scene,
            deleting_a_scene_takes_it_out_of_the_book,
            linking_an_idea_is_announced_and_relayed,
            an_idea_appears_wherever_it_was_noted_or_linked_until_discarded,
            whoever_starts_a_project_owns_it,
            an_idea_s_appearances_are_there_only_for_whoever_may_read_its_project,
            a_project_s_board_is_there_only_for_whoever_may_read_its_project,
            a_project_s_outline_is_there_only_for_whoever_may_read_its_project,
            a_project_s_scenes_are_there_only_for_whoever_may_read_its_project,
            a_project_s_members_are_listed_for_whoever_may_read_it,
            an_account_invited_into_a_project_joins_it_by_taking_the_invitation_up,
            a_deleted_project_s_owner_neither_lists_its_members_nor_invites_into_it,
            an_account_is_told_in_the_application_what_it_is_invited_into_and_joins_there,
            an_account_turns_down_in_the_application_what_it_is_invited_into,
        );
    };
    ($backend:ty: $($journey:ident),+ $(,)?) => {
        $(
            #[tokio::test(flavor = "multi_thread")]
            async fn $journey() {
                journeys::$journey::<$backend>().await;
            }
        )+
    };
}
