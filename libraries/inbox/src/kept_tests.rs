use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use clock::FixedClock;
use messaging::{Failure, Listener, Message, RoutingKey};
use serde_json::json;
use thiserror::Error;
use time::OffsetDateTime;
use transacting::{InMemoryTransactions, Transaction, Transactions, memory};

use crate::{EffectHandler, InMemoryInbox, Inboxed, InboxedEffect, KeptHandler};

#[derive(Debug, Error)]
#[error("{0}")]
struct Refusal(&'static str);

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
    ) -> Result<(), Failure> {
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
            return Err(Box::new(refusal));
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
    // given: a listener and a message
    let listener = inboxed(Noting::default());
    let message = a_message();

    // when: the message is handed over twice
    for _ in 0..2 {
        listener
            .handle(&message)
            .await
            .expect("handling should succeed");
    }

    // then: it is taken in once
    assert_eq!(noted(&listener), (1, 1));
}

#[tokio::test]
async fn a_message_whose_handling_failed_is_taken_in_anew_when_it_comes_again() {
    // given: a listener whose first handling fails
    let listener = inboxed(Noting::refusing(vec![Refusal("the disk is full")]));
    let message = a_message();

    // when: a message is handed over twice
    let refused = listener.handle(&message).await;
    listener
        .handle(&message)
        .await
        .expect("handling should succeed the second time");

    // then: the first handling is refused, naming the listener and why
    let refused = refused.expect_err("the first handling was refused");
    assert_eq!(refused.listener.as_str(), "note-what-was-heard");
    assert_eq!(refused.because.to_string(), "the disk is full");
    // and: the second takes it in anew
    assert_eq!(
        noted(&listener),
        (1, 2),
        "what was noted and heard in the refused try was rolled back, so it was heard anew"
    );
}

#[tokio::test]
async fn a_refused_handling_is_tried_once_and_left_to_be_offered_again() {
    // given: a listener whose handling fails
    let listener = inboxed(Noting::refusing(vec![Refusal("lost a race")]));

    // when: a message is handed over
    let refused = listener.handle(&a_message()).await;

    // then: it is refused
    assert!(refused.is_err());
    // and: tried once, keeping nothing
    assert_eq!(noted(&listener), (0, 1));
}

#[tokio::test]
async fn an_inboxed_handler_is_named_and_listens_as_it_says() {
    // given: an inboxed handler
    let listener = inboxed(Noting::default());

    // then: it is named, listens and is refused as its handler says
    assert_eq!(listener.named().as_str(), "note-what-was-heard");
    assert!(listener.hears(&RoutingKey::parse("thing.happened").expect("a plain key")));
    assert_eq!(listener.when_refused(), "Something heard was not noted.");
}

struct Mailing {
    transactions: Arc<InMemoryTransactions>,
    sent: Mutex<u32>,
    refusing: Mutex<Vec<Refusal>>,
}

#[async_trait]
impl EffectHandler for Mailing {
    const NAME: &'static str = "mail-what-was-heard";
    const LISTENS_TO: &'static [&'static str] = &["thing.happened"];
    const WHEN_REFUSED: &'static str = "Something heard was not mailed.";

    async fn take_in(&self, _message: &Message) -> Result<(), Failure> {
        let beside =
            tokio::time::timeout(std::time::Duration::from_secs(1), self.transactions.begin())
                .await
                .expect("no transaction is held while the mail goes, so another may begin")
                .expect("beginning should succeed");
        drop(beside);
        if let Some(refusal) = self
            .refusing
            .lock()
            .expect("nothing panics holding it")
            .pop()
        {
            return Err(Box::new(refusal));
        }
        *self.sent.lock().expect("nothing panics holding it") += 1;

        Ok(())
    }
}

fn mailing(refusals: Vec<Refusal>) -> InboxedEffect<Mailing> {
    let transactions = Arc::new(InMemoryTransactions::new());

    InboxedEffect::new(
        Mailing {
            transactions: transactions.clone(),
            sent: Mutex::new(0),
            refusing: Mutex::new(refusals),
        },
        transactions,
        Arc::new(InMemoryInbox::new(Arc::new(FixedClock::new(
            OffsetDateTime::UNIX_EPOCH,
        )))),
    )
}

fn sent(listener: &InboxedEffect<Mailing>) -> u32 {
    *listener
        .handler
        .sent
        .lock()
        .expect("nothing panics holding it")
}

#[tokio::test]
async fn an_effect_is_carried_out_with_no_transaction_held_and_once_for_a_message_handed_over_twice()
 {
    // given: an effect and a message
    let listener = mailing(Vec::new());
    let message = a_message();

    // when: the message is handed over twice
    for _ in 0..2 {
        listener
            .handle(&message)
            .await
            .expect("handling should succeed");
    }

    // then: the effect is carried out once
    assert_eq!(sent(&listener), 1);
}

#[tokio::test]
async fn an_effect_that_failed_is_carried_out_when_the_message_comes_again() {
    // given: an effect whose first try fails
    let listener = mailing(vec![Refusal("the mail server is down")]);
    let message = a_message();

    // when: a message is handed over twice
    let refused = listener.handle(&message).await;
    listener
        .handle(&message)
        .await
        .expect("handling should succeed the second time");

    // then: the first handling is refused, saying why
    assert_eq!(
        refused
            .expect_err("the first handling was refused")
            .because
            .to_string(),
        "the mail server is down"
    );
    // and: the second carries the effect out
    assert_eq!(
        sent(&listener),
        1,
        "a failed effect noted nothing, so it is carried out anew"
    );
}

#[tokio::test]
async fn an_effect_handler_is_named_and_listens_as_it_says() {
    // given: an inboxed effect
    let listener = mailing(Vec::new());

    // then: it is named, listens and is refused as its handler says
    assert_eq!(listener.named().as_str(), "mail-what-was-heard");
    assert!(listener.hears(&RoutingKey::parse("thing.happened").expect("a plain key")));
    assert_eq!(listener.when_refused(), "Something heard was not mailed.");
}
