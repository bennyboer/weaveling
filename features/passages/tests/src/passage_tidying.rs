use std::sync::Arc;

use clock::FixedClock;
use ideas_contract::DISCARDED;
use messaging::{Listener, Message, RoutingKey};
use passages_core::{PassageId, PassageService};
use passages_messaging::DeleteOnDiscard;
use passages_store::InMemoryPassageStore;
use serde_json::json;
use time::{Duration, OffsetDateTime};

const A_PROJECT: &str = "project_1";

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

struct Wired {
    passages: PassageService,
    tidy: DeleteOnDiscard,
}

fn a_workbench() -> Wired {
    let passages = PassageService::new(
        Arc::new(InMemoryPassageStore::new()),
        Arc::new(FixedClock::new(at(1_000))),
    );

    Wired {
        tidy: DeleteOnDiscard::new(passages.clone()),
        passages,
    }
}

impl Wired {
    async fn a_passage(&self) -> PassageId {
        self.passages
            .create(A_PROJECT)
            .await
            .expect("creating a passage should succeed")
            .id()
    }

    async fn still_there(&self, passage: &PassageId) -> bool {
        self.passages.open(&passage.to_string()).await.is_ok()
    }
}

fn discarded(passage: Option<&str>) -> Message {
    Message::opening(
        RoutingKey::parse(DISCARDED).expect("a declared routing key is fine"),
        json!({
            "event": {
                "version": 0,
                "name": "DISCARDED",
                "passage": passage,
            },
            "aggregate": { "id": "idea_1", "kind": "idea", "version": 4 },
            "agent": { "kind": "system", "id": null },
            "occurred_at": "1970-01-01T00:16:40Z",
        }),
        at(1_000),
    )
}

#[tokio::test]
async fn discarding_a_idea_takes_its_prose_with_it() {
    let wired = a_workbench();
    let passage = wired.a_passage().await;

    wired
        .tidy
        .handle(&discarded(Some(&passage.to_string())))
        .await
        .expect("tidying should succeed");

    assert!(
        !wired.still_there(&passage).await,
        "the CRDT store is the only copy of the prose, so a deleted project has to reach it \
         or the words outlive everything that could ever find them again"
    );
}

#[tokio::test]
async fn discarding_a_idea_that_never_had_prose_is_harmless() {
    let wired = a_workbench();
    let untouched = wired.a_passage().await;

    wired
        .tidy
        .handle(&discarded(None))
        .await
        .expect("a idea with no passage is nothing to do");

    assert!(wired.still_there(&untouched).await);
}

#[tokio::test]
async fn hearing_the_same_discard_twice_is_harmless() {
    let wired = a_workbench();
    let passage = wired.a_passage().await;

    for _ in 0..2 {
        wired
            .tidy
            .handle(&discarded(Some(&passage.to_string())))
            .await
            .expect("a redelivery is not a failure");
    }

    assert!(!wired.still_there(&passage).await);
}

#[tokio::test]
async fn a_passage_of_another_idea_is_left_alone() {
    let wired = a_workbench();
    let mine = wired.a_passage().await;
    let theirs = wired.a_passage().await;

    wired
        .tidy
        .handle(&discarded(Some(&mine.to_string())))
        .await
        .expect("tidying should succeed");

    assert!(!wired.still_there(&mine).await);
    assert!(
        wired.still_there(&theirs).await,
        "one discarded idea must never take another author's prose with it"
    );
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
        wired.tidy.handle(&nonsense).await.is_err(),
        "a message the listener cannot act on belongs in dead letters, not silently dropped"
    );
}
