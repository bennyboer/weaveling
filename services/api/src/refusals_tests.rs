use std::sync::Arc;

use async_trait::async_trait;
use axum::http::StatusCode;
use axum_test::TestServer;
use clock::FixedClock;
use messaging::{
    Deliveries, InMemoryDeliveries, InProcessDispatcher, Listener, ListenerName, Message,
    NotHandled, RoutingKey, Subscription,
};
use messaging_contract::RefusalDTO;
use time::macros::datetime;
use time::{Duration, OffsetDateTime};

use crate::refusals::router;

const OCCURRED: OffsetDateTime = datetime!(2026-10-08 09:00 UTC);

const GIVEN_UP: OffsetDateTime = datetime!(2026-10-08 09:01 UTC);

const ASKED: OffsetDateTime = datetime!(2026-10-08 10:00 UTC);

const CATALOGUING: &str = "catalogue-idea";

const UNWIRED: &str = "a-listener-taken-out-since";

struct Cataloguing;

#[async_trait]
impl Listener for Cataloguing {
    fn named(&self) -> ListenerName {
        ListenerName::parse(CATALOGUING).expect("a plain name is fine")
    }

    fn listens_to(&self) -> Vec<Subscription> {
        vec![Subscription::parse("idea.#").expect("a plain pattern is fine")]
    }

    fn when_refused(&self) -> &'static str {
        "A change to an idea did not reach the list of ideas."
    }

    async fn handle(&self, _message: &Message) -> Result<(), NotHandled> {
        Ok(())
    }
}

struct Refused {
    deliveries: Arc<InMemoryDeliveries>,
    server: TestServer,
}

impl Refused {
    fn nothing_yet() -> Self {
        let deliveries = Arc::new(InMemoryDeliveries::new());
        let dispatcher = Arc::new(InProcessDispatcher::queueing_to(deliveries.clone()));
        dispatcher.listen(Arc::new(Cataloguing));
        let server = TestServer::new(router(
            deliveries.clone(),
            dispatcher,
            Arc::new(FixedClock::new(ASKED)),
        ));

        Self { deliveries, server }
    }

    async fn given_up_on(&self, routing: &str) {
        self.given_up_by(CATALOGUING, routing).await;
    }

    async fn given_up_by(&self, listener: &str, routing: &str) {
        let message = Message::opening(
            RoutingKey::parse(routing).expect("a plain key is fine"),
            serde_json::json!({ "nothing": "much" }),
            OCCURRED,
        );
        self.deliveries
            .enqueue(
                &ListenerName::parse(listener).expect("a plain name is fine"),
                &message,
            )
            .await
            .expect("enqueuing should succeed");
        let claimed = self
            .deliveries
            .claim_due(OCCURRED, 16)
            .await
            .expect("claiming should succeed");
        self.deliveries
            .give_up(claimed[0].id, "the projection had not landed", GIVEN_UP)
            .await
            .expect("dead-lettering should succeed");
    }

    async fn listed(&self) -> Vec<RefusalDTO> {
        let response = self.server.get("/refusals").await;
        response.assert_status_ok();

        response.json()
    }
}

#[tokio::test]
async fn nothing_refused_is_an_empty_list() {
    let refused = Refused::nothing_yet();

    let listed = refused.listed().await;

    assert!(listed.is_empty());
}

#[tokio::test]
async fn a_refused_message_is_listed_with_what_why_and_when() {
    let refused = Refused::nothing_yet();
    refused.given_up_on("idea.captured").await;

    let listed = refused.listed().await;

    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].listener, "catalogue-idea");
    assert_eq!(listed[0].routing, "idea.captured");
    assert_eq!(listed[0].attempts, 1);
    assert_eq!(listed[0].why, "the projection had not landed");
    assert_eq!(listed[0].occurred_at, "2026-10-08T09:00:00Z");
    assert_eq!(listed[0].given_up_at, "2026-10-08T09:01:00Z");
    assert_eq!(listed[0].acknowledged_at, None);
}

#[tokio::test]
async fn a_refusal_says_in_plain_words_what_the_listener_could_not_do() {
    let refused = Refused::nothing_yet();
    refused.given_up_on("idea.captured").await;

    let listed = refused.listed().await;

    assert_eq!(
        listed[0].plainly.as_deref(),
        Some("A change to an idea did not reach the list of ideas."),
        "the listener says it itself, so a new one cannot be added without words for the author"
    );
}

#[tokio::test]
async fn a_refusal_by_a_listener_no_longer_wired_has_no_plain_words() {
    let refused = Refused::nothing_yet();
    refused.given_up_by(UNWIRED, "idea.captured").await;

    let listed = refused.listed().await;

    assert_eq!(
        listed[0].plainly, None,
        "a dead letter outlives the code that refused it, so the client needs its own fallback"
    );
}

#[tokio::test]
async fn trying_again_offers_the_message_afresh_from_now() {
    let refused = Refused::nothing_yet();
    refused.given_up_on("idea.captured").await;
    let id = refused.listed().await[0].id;

    let response = refused.server.post(&format!("/refusals/{id}/retry")).await;

    response.assert_status(StatusCode::NO_CONTENT);
    assert!(refused.listed().await.is_empty());
    assert!(
        refused
            .deliveries
            .claim_due(ASKED - Duration::seconds(1), 16)
            .await
            .expect("claiming should succeed")
            .is_empty(),
        "the retry happens when the author asks for it, so it is due from then"
    );
    let again = refused
        .deliveries
        .claim_due(ASKED, 16)
        .await
        .expect("claiming should succeed");
    assert_eq!(again.len(), 1);
    assert_eq!(again[0].message.routing.to_string(), "idea.captured");
}

#[tokio::test]
async fn acknowledging_keeps_the_message_but_marks_it_seen() {
    let refused = Refused::nothing_yet();
    refused.given_up_on("idea.captured").await;
    refused.given_up_on("idea.retitled").await;
    let id = refused.listed().await[0].id;

    let response = refused
        .server
        .post(&format!("/refusals/{id}/acknowledge"))
        .await;

    response.assert_status(StatusCode::NO_CONTENT);
    let listed: Vec<_> = refused
        .listed()
        .await
        .into_iter()
        .map(|refusal| (refusal.routing, refusal.acknowledged_at))
        .collect();
    assert_eq!(
        listed,
        vec![
            (
                "idea.captured".to_owned(),
                Some("2026-10-08T10:00:00Z".to_owned())
            ),
            ("idea.retitled".to_owned(), None),
        ],
        "an acknowledged refusal stays listed, so it can still be tried again"
    );
}

#[tokio::test]
async fn acting_on_something_already_gone_is_harmless() {
    let refused = Refused::nothing_yet();

    let retried = refused.server.post("/refusals/404/retry").await;
    let acknowledged = refused.server.post("/refusals/404/acknowledge").await;

    retried.assert_status(StatusCode::NO_CONTENT);
    acknowledged.assert_status(StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn an_id_that_is_not_a_number_is_a_bad_request() {
    let refused = Refused::nothing_yet();

    let response = refused.server.post("/refusals/latest/retry").await;

    response.assert_status(StatusCode::BAD_REQUEST);
}
