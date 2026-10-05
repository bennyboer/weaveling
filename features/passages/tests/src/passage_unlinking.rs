use std::sync::Arc;

use clock::FixedClock;
use ideas_contract::DISCARDED;
use messaging::{Listener, Message, RoutingKey, Subscription};
use passages_core::{IdeaLink, PassageId, PassageService};
use passages_messaging::UnlinkOnDiscard;
use passages_store::InMemoryPassageStore;
use serde_json::json;
use time::{Duration, OffsetDateTime};

const A_PROJECT: &str = "project_1";

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

struct Wired {
    passages: PassageService,
    unlink: UnlinkOnDiscard,
}

fn a_workbench() -> Wired {
    let passages = PassageService::new(
        Arc::new(InMemoryPassageStore::new()),
        Arc::new(FixedClock::new(at(1_000))),
    );

    Wired {
        unlink: UnlinkOnDiscard::new(passages.clone()),
        passages,
    }
}

impl Wired {
    async fn a_passage_linked_to(&self, ideas: &[&str]) -> PassageId {
        let id = self
            .passages
            .create(A_PROJECT)
            .await
            .expect("creating a passage should succeed")
            .id();

        for idea in ideas {
            self.passages
                .link(&id.to_string(), idea)
                .await
                .expect("linking should succeed");
        }

        id
    }

    async fn ideas_of(&self, passage: PassageId) -> Vec<IdeaLink> {
        self.passages
            .open(&passage.to_string())
            .await
            .expect("the passage should still be there")
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
async fn a_discarded_idea_is_unlinked_from_every_passage() {
    let wired = a_workbench();
    let one = wired.a_passage_linked_to(&["idea_gone", "idea_kept"]).await;
    let other = wired.a_passage_linked_to(&["idea_gone"]).await;

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
async fn a_discarded_idea_takes_no_passage_with_it() {
    let wired = a_workbench();
    let passage = wired.a_passage_linked_to(&["idea_gone"]).await;

    wired
        .unlink
        .handle(&discarded("idea_gone"))
        .await
        .expect("unlinking should succeed");

    assert!(
        wired.passages.open(&passage.to_string()).await.is_ok(),
        "ideas are disposable and passages are the book: losing the idea drops the link, never the text"
    );
}

#[tokio::test]
async fn hearing_the_same_discard_twice_is_harmless() {
    let wired = a_workbench();
    wired.a_passage_linked_to(&["idea_gone"]).await;

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
