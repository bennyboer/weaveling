use std::sync::{Arc, Mutex};
use std::time::Duration;

use appearances_catalog::InMemoryAppearanceCatalog;
use appearances_contract::PlaceDTO;
use appearances_core::{
    AppearanceCatalog, AppearancesCore, IdeaLink, Place, ProjectLink, SceneLink, Subject,
};
use appearances_rest::TellingAppearances;
use async_trait::async_trait;
use authorizing::{Act, Authorizer, AuthorizerError, Readable};
use axum::Extension;
use axum::http::StatusCode;
use axum_test::{TestServer, TestWebSocket};
use eventsourcing::{AccountId, Agent};
use live::Broadcaster;
use serde_json::Value;

const READABLE: &str = "project_ada";
const UNREADABLE: &str = "project_kept";

#[derive(Default)]
struct OneProject {
    down: Mutex<bool>,
}

#[async_trait]
impl Authorizer for OneProject {
    async fn may(&self, _agent: &Agent, act: Act, project: &str) -> Result<bool, AuthorizerError> {
        if *self.down.lock().expect("never poisoned") {
            return Err(AuthorizerError::Unreachable("the list is down".into()));
        }

        Ok(act == Act::Read && project == READABLE)
    }

    async fn readable_by(&self, _agent: &Agent) -> Result<Readable, AuthorizerError> {
        Ok(Readable::Everything)
    }
}

struct Guarded {
    kept: Arc<InMemoryAppearanceCatalog>,
    telling: TellingAppearances,
    authorizer: Arc<OneProject>,
    server: TestServer,
}

fn an_idea(id: &str) -> Subject {
    Subject::Idea(IdeaLink::from(id))
}

async fn guarded() -> Guarded {
    let broadcaster = Broadcaster::new();
    let kept = Arc::new(InMemoryAppearanceCatalog::new());
    let authorizer = Arc::new(OneProject::default());
    let appearances = AppearancesCore::new(kept.clone(), authorizer.clone());
    let server = TestServer::builder().http_transport().build(
        appearances_rest::router(appearances.clone())
            .merge(appearances_rest::live(appearances, broadcaster.clone()))
            .layer(Extension(Agent::Account(AccountId::from("account_ada")))),
    );
    for (idea, project) in [("idea_ada", READABLE), ("idea_kept", UNREADABLE)] {
        kept.remember_project(&an_idea(idea), &ProjectLink::from(project))
            .await
            .expect("remembering should succeed");
        kept.remember(&an_idea(idea), &Place::Scene(SceneLink::from("scene_1")), 1)
            .await
            .expect("remembering should succeed");
    }

    Guarded {
        telling: TellingAppearances::around(kept.clone(), broadcaster),
        kept,
        authorizer,
        server,
    }
}

async fn following(server: &TestServer, idea: &str) -> TestWebSocket {
    server
        .get_websocket(&format!("/appearances/live?idea={idea}"))
        .await
        .into_websocket()
        .await
}

async fn heard_soon(socket: &mut TestWebSocket) -> Option<Value> {
    tokio::time::timeout(Duration::from_millis(300), socket.receive_text())
        .await
        .ok()
        .map(|frame| serde_json::from_str(&frame).expect("a frame is JSON"))
}

#[tokio::test]
async fn where_an_idea_of_a_project_one_may_read_appears_is_told_and_followed() {
    let guarded = guarded().await;
    let mut socket = following(&guarded.server, "idea_ada").await;

    let listed = guarded.server.get("/appearances?idea=idea_ada").await;
    guarded
        .telling
        .remember(
            &an_idea("idea_ada"),
            &Place::Scene(SceneLink::from("scene_2")),
            2,
        )
        .await
        .expect("remembering should succeed");

    listed.assert_status_ok();
    assert_eq!(
        listed.json::<Vec<PlaceDTO>>(),
        vec![PlaceDTO::Scene {
            id: "scene_1".to_owned()
        }]
    );
    assert_eq!(
        heard_soon(&mut socket).await.expect("told while fresh")["of"],
        "idea_ada"
    );
}

#[tokio::test]
async fn an_idea_of_a_project_one_may_not_read_is_not_there() {
    let guarded = guarded().await;

    guarded
        .server
        .get("/appearances?idea=idea_kept")
        .await
        .assert_status(StatusCode::NOT_FOUND);
    guarded
        .server
        .get_websocket("/appearances/live?idea=idea_kept")
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn an_idea_of_no_project_known_yet_appears_nowhere_and_is_told_only_once_it_may_be_read() {
    let guarded = guarded().await;
    let listed = guarded.server.get("/appearances?idea=idea_new").await;
    let mut kept_from = following(&guarded.server, "idea_new").await;
    let mut readable = following(&guarded.server, "idea_later").await;
    let mut orphaned = following(&guarded.server, "idea_orphan").await;

    for (idea, project) in [("idea_new", UNREADABLE), ("idea_later", READABLE)] {
        guarded
            .kept
            .remember_project(&an_idea(idea), &ProjectLink::from(project))
            .await
            .expect("remembering should succeed");
        guarded
            .telling
            .remember(&an_idea(idea), &Place::Scene(SceneLink::from("scene_9")), 1)
            .await
            .expect("remembering should succeed");
    }

    listed.assert_status_ok();
    assert!(listed.json::<Vec<PlaceDTO>>().is_empty());
    assert!(
        heard_soon(&mut readable).await.is_some(),
        "an idea whose project turns out readable is told, so the frames do go out"
    );
    assert!(
        heard_soon(&mut kept_from).await.is_none(),
        "followed before its project was known, an idea of a project kept from one stays untold"
    );
    guarded
        .telling
        .remember(
            &an_idea("idea_orphan"),
            &Place::Scene(SceneLink::from("scene_9")),
            1,
        )
        .await
        .expect("remembering should succeed");
    assert!(
        heard_soon(&mut orphaned).await.is_none(),
        "where an idea of no project known appears is told to nobody, as nobody can be let in to it"
    );
}

#[tokio::test]
async fn a_followed_idea_ends_once_whom_to_tell_cannot_be_known() {
    let guarded = guarded().await;
    let mut socket = following(&guarded.server, "idea_ada").await;

    *guarded.authorizer.down.lock().expect("never poisoned") = true;
    guarded
        .telling
        .remember(
            &an_idea("idea_ada"),
            &Place::Scene(SceneLink::from("scene_2")),
            2,
        )
        .await
        .expect("remembering should succeed");

    assert!(
        tokio::spawn(async move { socket.receive_text().await })
            .await
            .is_err(),
        "a change it cannot judge may be one the follower needed, so the socket ends and its client reloads"
    );
}

#[tokio::test]
async fn whom_to_let_in_not_being_known_is_a_failure_not_a_no() {
    let guarded = guarded().await;
    *guarded.authorizer.down.lock().expect("never poisoned") = true;

    guarded
        .server
        .get("/appearances?idea=idea_ada")
        .await
        .assert_status(StatusCode::INTERNAL_SERVER_ERROR);
    guarded
        .server
        .get_websocket("/appearances/live?idea=idea_ada")
        .await
        .assert_status(StatusCode::INTERNAL_SERVER_ERROR);
}
