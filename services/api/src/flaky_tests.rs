use std::sync::Arc;

use async_trait::async_trait;
use messaging::{Listener, ListenerName, Message, NotHandled, RoutingKey, Subscription};
use time::macros::datetime;

use crate::flaky::{Flakiness, Flaky};

struct Willing;

#[async_trait]
impl Listener for Willing {
    fn named(&self) -> ListenerName {
        ListenerName::parse("willing").expect("a plain name is fine")
    }

    fn listens_to(&self) -> Vec<Subscription> {
        vec![Subscription::parse("#").expect("a plain pattern is fine")]
    }

    fn when_refused(&self) -> &'static str {
        "Nothing an author would miss."
    }

    async fn handle(&self, _message: &Message) -> Result<(), NotHandled> {
        Ok(())
    }
}

fn a_message() -> Message {
    Message::opening(
        RoutingKey::parse("idea.captured").expect("a plain key is fine"),
        serde_json::json!({}),
        datetime!(2026-10-10 09:00 UTC),
    )
}

async fn refused_out_of(share: &str, tries: usize) -> usize {
    let flaky = Flaky::wrapping(
        Arc::new(Willing),
        Flakiness::parse(share).expect("a share from 0 to 1 is fine"),
    );
    let mut refused = 0;

    for _ in 0..tries {
        if flaky.handle(&a_message()).await.is_err() {
            refused += 1;
        }
    }

    refused
}

#[tokio::test]
async fn all_flakiness_refuses_everything() {
    assert_eq!(refused_out_of("1", 50).await, 50);
}

#[tokio::test]
async fn no_flakiness_refuses_nothing() {
    assert_eq!(refused_out_of("0", 50).await, 0);
}

#[tokio::test]
async fn half_flakiness_refuses_some_and_lets_some_through() {
    let refused = refused_out_of("0.5", 200).await;

    assert!(
        (40..=160).contains(&refused),
        "a coin is not a constant: {refused} of 200 refused"
    );
}

#[tokio::test]
async fn a_flaky_listener_is_still_the_listener_it_wraps() {
    let flaky = Flaky::wrapping(
        Arc::new(Willing),
        Flakiness::parse("1").expect("a share from 0 to 1 is fine"),
    );

    assert_eq!(flaky.named().as_str(), "willing");
    assert_eq!(flaky.when_refused(), "Nothing an author would miss.");
}

#[test]
fn a_share_outside_nought_to_one_is_refused() {
    for nonsense in ["1.5", "-0.1", "half", ""] {
        assert!(
            Flakiness::parse(nonsense).is_err(),
            "{nonsense:?} should not start a service"
        );
    }
}
