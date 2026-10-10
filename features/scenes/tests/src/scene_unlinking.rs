use std::sync::Arc;

use clock::FixedClock;
use ideas_contract::DISCARDED;
use messaging::{Listener, Message, RoutingKey, Subscription};
use scenes_core::{IdeaLink, SceneId, SceneService};
use scenes_messaging::UnlinkOnDiscard;
use scenes_store::InMemorySceneStore;
use serde_json::json;
use time::{Duration, OffsetDateTime};

const A_PROJECT: &str = "project_1";

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

struct Wired {
    scenes: SceneService,
    unlink: UnlinkOnDiscard,
}

fn a_workbench() -> Wired {
    let scenes = SceneService::new(
        Arc::new(InMemorySceneStore::new()),
        Arc::new(FixedClock::new(at(1_000))),
    );

    Wired {
        unlink: UnlinkOnDiscard::new(scenes.clone()),
        scenes,
    }
}

impl Wired {
    async fn a_scene_linked_to(&self, ideas: &[&str]) -> SceneId {
        let id = self
            .scenes
            .create(A_PROJECT)
            .await
            .expect("creating a scene should succeed")
            .id();

        for idea in ideas {
            self.scenes
                .link(&id.to_string(), idea)
                .await
                .expect("linking should succeed");
        }

        id
    }

    async fn ideas_of(&self, scene: SceneId) -> Vec<IdeaLink> {
        self.scenes
            .open(&scene.to_string())
            .await
            .expect("the scene should still be there")
            .ideas()
            .to_vec()
    }
}

fn discarded(idea: &str) -> Message {
    Message::opening(
        RoutingKey::parse(DISCARDED).expect("a declared routing key is fine"),
        json!({
            "event": { "version": 0, "name": "DISCARDED" },
            "aggregate": { "id": idea, "kind": "idea", "version": 2 },
            "agent": { "kind": "system", "id": null },
            "occurred_at": "1970-01-01T00:16:40Z",
        }),
        at(1_000),
    )
}

#[tokio::test]
async fn a_discarded_idea_is_unlinked_from_every_scene() {
    let wired = a_workbench();
    let one = wired.a_scene_linked_to(&["idea_gone", "idea_kept"]).await;
    let other = wired.a_scene_linked_to(&["idea_gone"]).await;

    wired
        .unlink
        .handle(&discarded("idea_gone"))
        .await
        .expect("unlinking should succeed");

    assert_eq!(wired.ideas_of(one).await, vec![IdeaLink::from("idea_kept")]);
    assert!(
        wired.ideas_of(other).await.is_empty(),
        "a link to an idea nobody kept is a reference to nothing"
    );
}

#[tokio::test]
async fn a_discarded_idea_takes_no_scene_with_it() {
    let wired = a_workbench();
    let scene = wired.a_scene_linked_to(&["idea_gone"]).await;

    wired
        .unlink
        .handle(&discarded("idea_gone"))
        .await
        .expect("unlinking should succeed");

    assert!(
        wired.scenes.open(&scene.to_string()).await.is_ok(),
        "ideas are disposable and scenes are the book: losing the idea drops the link, never the text"
    );
}

#[tokio::test]
async fn hearing_the_same_discard_twice_is_harmless() {
    let wired = a_workbench();
    wired.a_scene_linked_to(&["idea_gone"]).await;

    for _ in 0..2 {
        wired
            .unlink
            .handle(&discarded("idea_gone"))
            .await
            .expect("a redelivery is not a failure");
    }
}

#[tokio::test]
async fn the_listener_hears_only_ideas_being_discarded() {
    let wired = a_workbench();

    let heard: Vec<Subscription> = wired.unlink.listens_to();

    assert!(heard.iter().any(|subscription| {
        subscription.covers(&RoutingKey::parse(DISCARDED).expect("a declared key is fine"))
    }));
    assert_eq!(heard.len(), 1);
}

#[tokio::test]
async fn a_message_about_no_idea_at_all_is_refused() {
    let wired = a_workbench();
    let nonsense = Message::opening(
        RoutingKey::parse(DISCARDED).expect("a declared routing key is fine"),
        json!({ "nothing": "useful" }),
        at(1_000),
    );

    assert!(
        wired.unlink.handle(&nonsense).await.is_err(),
        "a message the listener cannot act on belongs in dead letters, not silently dropped"
    );
}
