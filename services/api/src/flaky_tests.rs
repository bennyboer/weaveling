use std::sync::Arc;

use async_trait::async_trait;
use messaging::{Listener, ListenerName, Message, NotHandled, RoutingKey, Subscription};
use time::macros::datetime;

use crate::flaky::{Flakiness, FlakyListener, WEAVELING_FLAKY};
use crate::settings::Setting;

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
    let flaky = FlakyListener::wrapping(
        Arc::new(Willing),
        Flakiness::parse(Setting::new(WEAVELING_FLAKY, share))
            .expect("a share from 0 to 1 is fine"),
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
    // expect: a listener flaky through and through refuses every try
    assert_eq!(refused_out_of("1", 50).await, 50);
}

#[tokio::test]
async fn no_flakiness_refuses_nothing() {
    // expect: a listener not flaky at all refuses no try
    assert_eq!(refused_out_of("0", 50).await, 0);
}

#[tokio::test]
async fn half_flakiness_refuses_some_and_lets_some_through() {
    // when: a listener flaky half the time is tried many times
    let refused = refused_out_of("0.5", 200).await;

    // then: some tries are refused and some let through
    assert!(
        (40..=160).contains(&refused),
        "a coin is not a constant: {refused} of 200 refused"
    );
}

#[tokio::test]
async fn a_flaky_listener_is_still_the_listener_it_wraps() {
    // given: a flaky listener
    let flaky = FlakyListener::wrapping(
        Arc::new(Willing),
        Flakiness::parse(Setting::new(WEAVELING_FLAKY, "1")).expect("a share from 0 to 1 is fine"),
    );

    // then: it is named and refuses in the words of the listener it wraps
    assert_eq!(flaky.named().as_str(), "willing");
    assert_eq!(flaky.when_refused(), "Nothing an author would miss.");
}

#[test]
fn a_share_outside_nought_to_one_is_refused() {
    // expect: a share outside nought to one is refused
    for nonsense in ["1.5", "-0.1", "half", ""] {
        assert!(
            Flakiness::parse(Setting::new(WEAVELING_FLAKY, nonsense)).is_err(),
            "{nonsense:?} should not start a service"
        );
    }
}
