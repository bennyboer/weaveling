use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use clock::FixedClock;
use messaging::{Listener, Message, RoutingKey};
use serde_json::json;
use thiserror::Error;
use time::OffsetDateTime;
use transacting::{InMemoryTransactions, LostRace, Transaction, memory};

use crate::{InMemoryInbox, Inboxed, KeptHandler, Refused};

#[derive(Debug, Error)]
#[error("{0}")]
struct Refusal(&'static str, bool);

impl LostRace for Refusal {
    fn lost_race(&self) -> bool {
        self.1
    }
}

#[derive(Default)]
struct Noting {
    noted: Arc<Mutex<Vec<String>>>,
    refusing: Mutex<Vec<Refusal>>,
    tried: Mutex<u32>,
}

impl Noting {
    fn refusing(refusals: Vec<Refusal>) -> Self {
        Self {
            refusing: Mutex::new(refusals),
            ..Self::default()
        }
    }
}

#[async_trait]
impl KeptHandler for Noting {
    const NAME: &'static str = "note-what-was-heard";
    const LISTENS_TO: &'static [&'static str] = &["thing.happened"];
    const WHEN_REFUSED: &'static str = "Something heard was not noted.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Refused> {
        *self.tried.lock().expect("nothing panics holding it") += 1;
        self.noted
            .lock()
            .expect("nothing panics holding it")
            .push(message.id.as_uuid().to_string());
        let noted = self.noted.clone();
        memory::undo_on_rollback(transaction, move || {
            noted.lock().expect("nothing panics holding it").pop();
        })?;
        if let Some(refusal) = self
            .refusing
            .lock()
            .expect("nothing panics holding it")
            .pop()
        {
            return Err(Refused::of(refusal));
        }

        Ok(())
    }
}

fn inboxed(handler: Noting) -> Inboxed<Noting> {
    Inboxed::new(
        handler,
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemoryInbox::new(Arc::new(FixedClock::new(
            OffsetDateTime::UNIX_EPOCH,
        )))),
    )
}

fn a_message() -> Message {
    Message::opening(
        RoutingKey::parse("thing.happened").expect("a plain key"),
        json!({}),
        OffsetDateTime::UNIX_EPOCH,
    )
}

fn noted(listener: &Inboxed<Noting>) -> (usize, u32) {
    (
        listener
            .handler
            .noted
            .lock()
            .expect("nothing panics holding it")
            .len(),
        *listener
            .handler
            .tried
            .lock()
            .expect("nothing panics holding it"),
    )
}

#[tokio::test]
async fn a_message_handed_over_twice_is_taken_in_once() {
    let listener = inboxed(Noting::default());
    let message = a_message();

    for _ in 0..2 {
        listener
            .handle(&message)
            .await
            .expect("handling should succeed");
    }

    assert_eq!(noted(&listener), (1, 1));
}

#[tokio::test]
async fn a_message_whose_handling_failed_is_taken_in_anew_when_it_comes_again() {
    let listener = inboxed(Noting::refusing(vec![Refusal("the disk is full", false)]));
    let message = a_message();

    let refused = listener.handle(&message).await;
    listener
        .handle(&message)
        .await
        .expect("handling should succeed the second time");

    let refused = refused.expect_err("the first handling was refused");
    assert_eq!(refused.listener.as_str(), "note-what-was-heard");
    assert_eq!(refused.because.to_string(), "the disk is full");
    assert_eq!(
        noted(&listener),
        (1, 2),
        "what was noted and heard in the refused try was rolled back, so it was heard anew"
    );
}

#[tokio::test]
async fn a_lost_race_is_tried_again_in_a_fresh_transaction() {
    let listener = inboxed(Noting::refusing(vec![
        Refusal("lost again", true),
        Refusal("lost", true),
    ]));

    listener
        .handle(&a_message())
        .await
        .expect("the third try lands");

    assert_eq!(noted(&listener), (1, 3));
}

#[tokio::test]
async fn a_refusal_other_than_a_lost_race_is_not_tried_again() {
    let listener = inboxed(Noting::refusing(vec![Refusal("refused", false)]));

    let refused = listener.handle(&a_message()).await;

    assert!(refused.is_err());
    assert_eq!(noted(&listener), (0, 1));
}

#[tokio::test]
async fn an_inboxed_handler_is_named_and_listens_as_it_says() {
    let listener = inboxed(Noting::default());

    assert_eq!(listener.named().as_str(), "note-what-was-heard");
    assert!(listener.hears(&RoutingKey::parse("thing.happened").expect("a plain key")));
    assert_eq!(listener.when_refused(), "Something heard was not noted.");
}
