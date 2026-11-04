use std::sync::Arc;

use authorizing::AllowingEverything;
use clock::FixedClock;
use eventsourcing::Agent;
use ideas_contract::DISCARDED;
use inbox::{InMemoryInbox, Inboxed};
use messaging::{Listener, Message, RoutingKey, Subscription};
use scenes_catalog::{InMemoryQuotationRestylingTasks, InMemorySceneCatalog};
use scenes_core::{IdeaLink, SceneId, ScenesCore};
use scenes_messaging::UnlinkOnDiscardListener;
use scenes_store::InMemorySceneStore;
use serde_json::json;
use time::{Duration, OffsetDateTime};
use transacting::InMemoryTransactions;

const A_PROJECT: &str = "project_1";

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

struct Wired {
    scenes: ScenesCore,
    unlink: Inboxed<UnlinkOnDiscardListener>,
}

fn a_workbench() -> Wired {
    let transactions = Arc::new(InMemoryTransactions::new());
    let clock = Arc::new(FixedClock::new(at(1_000)));
    let scenes = ScenesCore::new(
        Arc::new(InMemorySceneStore::new()),
        transactions.clone(),
        Arc::new(InMemorySceneCatalog::new()),
        Arc::new(InMemoryQuotationRestylingTasks::new()),
        Arc::new(AllowingEverything),
        clock.clone(),
    );

    Wired {
        unlink: Inboxed::new(
            UnlinkOnDiscardListener::new(scenes.clone()),
            transactions,
            Arc::new(InMemoryInbox::new(clock)),
        ),
        scenes,
    }
}

impl Wired {
    async fn a_scene_linked_to(&self, ideas: &[&str]) -> SceneId {
        let id = self
            .scenes
            .create(A_PROJECT, &Agent::Local)
            .await
            .expect("creating a scene should succeed")
            .id();

        for idea in ideas {
            self.scenes
                .link(&id.to_string(), idea, &Agent::Local)
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
    // given: a scene linked to two ideas
    let wired = a_workbench();
    let one = wired.a_scene_linked_to(&["idea_gone", "idea_kept"]).await;
    // and: another linked to one of them
    let other = wired.a_scene_linked_to(&["idea_gone"]).await;

    // when: that idea's discarding is heard
    wired
        .unlink
        .handle(&discarded("idea_gone"))
        .await
        .expect("unlinking should succeed");

    // then: the first keeps only the other idea
    assert_eq!(wired.ideas_of(one).await, vec![IdeaLink::from("idea_kept")]);
    // and: the second keeps none
    assert!(
        wired.ideas_of(other).await.is_empty(),
        "a link to an idea nobody kept is a reference to nothing"
    );
}

#[tokio::test]
async fn a_discarded_idea_takes_no_scene_with_it() {
    // given: a scene linked to an idea
    let wired = a_workbench();
    let scene = wired.a_scene_linked_to(&["idea_gone"]).await;

    // when: the idea's discarding is heard
    wired
        .unlink
        .handle(&discarded("idea_gone"))
        .await
        .expect("unlinking should succeed");

    // then: the scene is still there
    assert!(
        wired.scenes.open(&scene.to_string()).await.is_ok(),
        "ideas are disposable and scenes are the book: losing the idea drops the link, never the text"
    );
}

#[tokio::test]
async fn hearing_the_same_discard_twice_is_harmless() {
    // given: a scene linked to an idea
    let wired = a_workbench();
    wired.a_scene_linked_to(&["idea_gone"]).await;

    // then: hearing the idea's discarding twice succeeds both times
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
    // given: the listener
    let wired = a_workbench();

    // when: what it listens to is asked
    let heard: Vec<Subscription> = wired.unlink.listens_to();

    // then: it hears ideas being discarded
    assert!(heard.iter().any(|subscription| {
        subscription.covers(&RoutingKey::parse(DISCARDED).expect("a declared key is fine"))
    }));
    // and: nothing else
    assert_eq!(heard.len(), 1);
}

#[tokio::test]
async fn a_message_about_no_idea_at_all_is_refused() {
    // given: a message about no idea at all
    let wired = a_workbench();
    let nonsense = Message::opening(
        RoutingKey::parse(DISCARDED).expect("a declared routing key is fine"),
        json!({ "nothing": "useful" }),
        at(1_000),
    );

    // then: hearing it fails
    assert!(
        wired.unlink.handle(&nonsense).await.is_err(),
        "a message the listener cannot act on belongs in dead letters, not silently dropped"
    );
}
