use std::sync::Arc;
use std::time::Duration;

use ::time::OffsetDateTime;
use appearances_catalog::InMemoryAppearanceCatalog;
use appearances_core::{
    AppearanceCatalog, IdeaLink, Place, ProjectLink, SceneLink, SectionLink, Subject,
};
use appearances_rest::{Readers, TellingAppearances};
use authorizing::AllowingEverything;
use axum::Extension;
use axum_test::{TestServer, TestWebSocket};
use eventsourcing::Agent;
use live::Broadcaster;
use serde_json::{Value, json};
use tokio::time;

const LAID: OffsetDateTime = OffsetDateTime::UNIX_EPOCH;

fn an_idea(id: &str) -> Subject {
    Subject::Idea(IdeaLink::from(id))
}

fn a_scene(id: &str) -> Place {
    Place::Scene(SceneLink::from(id))
}

fn a_section(id: &str) -> Place {
    Place::Section(SectionLink::from(id))
}

struct Followed {
    catalog: TellingAppearances,
    sockets: Vec<TestWebSocket>,
    _server: TestServer,
}

async fn followed(ideas: &[&str]) -> Followed {
    let broadcaster = Broadcaster::new();
    let kept = Arc::new(InMemoryAppearanceCatalog::new());
    for idea in ideas {
        kept.remember_project(&an_idea(idea), &ProjectLink::from("project_1"))
            .await
            .expect("remembering should succeed");
    }
    let catalog = TellingAppearances::around(kept.clone(), broadcaster.clone());
    let server = TestServer::builder().http_transport().build(
        appearances_rest::live(
            broadcaster,
            Readers {
                catalog: kept,
                authorizer: Arc::new(AllowingEverything),
            },
        )
        .layer(Extension(Agent::Local)),
    );
    let mut sockets = Vec::new();
    for idea in ideas {
        sockets.push(
            server
                .get_websocket(&format!("/appearances/live?idea={idea}"))
                .await
                .into_websocket()
                .await,
        );
    }

    Followed {
        catalog,
        sockets,
        _server: server,
    }
}

async fn told(socket: &mut TestWebSocket) -> Value {
    let frame = time::timeout(Duration::from_secs(1), socket.receive_text())
        .await
        .expect("the places are told while they are fresh");

    serde_json::from_str(&frame).expect("a frame is JSON")
}

async fn nothing_told(socket: &mut TestWebSocket) -> bool {
    time::timeout(Duration::from_millis(200), socket.receive_text())
        .await
        .is_err()
}

#[tokio::test]
async fn whoever_follows_an_idea_is_told_where_it_appears_as_that_changes() {
    let mut followed = followed(&["idea_1"]).await;

    followed
        .catalog
        .remember(&an_idea("idea_1"), &a_scene("scene_1"), 1)
        .await
        .expect("remembering should succeed");
    let placed = told(&mut followed.sockets[0]).await;
    followed
        .catalog
        .forget(&an_idea("idea_1"), &a_scene("scene_1"), 2)
        .await
        .expect("forgetting should succeed");
    let gone = told(&mut followed.sockets[0]).await;

    assert_eq!(
        (placed["of"].clone(), placed["event"].clone()),
        (
            json!("idea_1"),
            json!({ "places": [{ "type": "scene", "id": "scene_1" }] })
        )
    );
    assert_eq!(gone["event"], json!({ "places": [] }));
    assert!(gone["version"].as_u64() > placed["version"].as_u64());
}

#[tokio::test]
async fn a_late_change_is_told_as_what_is_kept() {
    let mut followed = followed(&["idea_1"]).await;
    followed
        .catalog
        .forget(&an_idea("idea_1"), &a_scene("scene_1"), 2)
        .await
        .expect("forgetting should succeed");
    told(&mut followed.sockets[0]).await;

    followed
        .catalog
        .remember(&an_idea("idea_1"), &a_scene("scene_1"), 1)
        .await
        .expect("remembering should succeed");

    assert_eq!(
        told(&mut followed.sockets[0]).await["event"],
        json!({ "places": [] }),
        "a remembering older than the forgetting leaves the idea nowhere"
    );
}

#[tokio::test]
async fn a_place_forgotten_is_told_to_every_idea_that_appeared_there_and_no_other() {
    let mut followed = followed(&["idea_1", "idea_2", "idea_3"]).await;
    for (idea, place) in [
        ("idea_1", a_section("section_gone")),
        ("idea_1", a_section("section_kept")),
        ("idea_2", a_section("section_gone")),
        ("idea_3", a_section("section_kept")),
    ] {
        followed
            .catalog
            .remember(&an_idea(idea), &place, 1)
            .await
            .expect("remembering should succeed");
    }
    for socket in &mut followed.sockets {
        while !nothing_told(socket).await {}
    }

    followed
        .catalog
        .forget_place(&a_section("section_gone"), LAID)
        .await
        .expect("forgetting should succeed");

    assert_eq!(
        told(&mut followed.sockets[0]).await["event"],
        json!({ "places": [{ "type": "section", "id": "section_kept" }] })
    );
    assert_eq!(
        told(&mut followed.sockets[1]).await["event"],
        json!({ "places": [] })
    );
    assert!(
        nothing_told(&mut followed.sockets[2]).await,
        "an idea that never appeared in the place is not told"
    );
}

#[tokio::test]
async fn a_discarded_idea_is_told_it_appears_nowhere() {
    let mut followed = followed(&["idea_1"]).await;
    followed
        .catalog
        .remember(&an_idea("idea_1"), &a_scene("scene_1"), 1)
        .await
        .expect("remembering should succeed");
    told(&mut followed.sockets[0]).await;

    followed
        .catalog
        .forget_subject(&an_idea("idea_1"), LAID)
        .await
        .expect("forgetting should succeed");

    assert_eq!(
        told(&mut followed.sockets[0]).await["event"],
        json!({ "places": [] })
    );
}

#[tokio::test]
async fn forgetting_old_tombstones_reaches_the_catalog_and_tells_nobody() {
    let mut followed = followed(&["idea_1"]).await;
    followed
        .catalog
        .forget_subject(&an_idea("idea_1"), LAID)
        .await
        .expect("forgetting should succeed");
    told(&mut followed.sockets[0]).await;

    let forgotten = followed
        .catalog
        .forget_tombstones_laid_before(LAID + Duration::from_secs(1))
        .await
        .expect("forgetting tombstones should succeed");

    assert_eq!(forgotten, 1);
    assert!(
        nothing_told(&mut followed.sockets[0]).await,
        "where an idea appears does not change when its tombstone goes"
    );
}
