use std::sync::Arc;

use async_trait::async_trait;
use axum::Extension;
use axum::http::StatusCode;
use axum_test::TestServer;
use clock::FixedClock;
use eventsourcing::Agent;
use live::Broadcaster;
use live::testing::Subscriber;
use messaging::RETRIABLE_FOR;
use messaging::{
    Deliveries, InMemoryDeliveries, InProcessDispatcher, Listener, ListenerName, Message,
    NotHandled, RoutingKey, Subscription,
};
use messaging_contract::{RefusalDTO, refused};
use time::macros::datetime;
use time::{Duration, OffsetDateTime};

use crate::refusals;
use crate::refusals::{RefusalsTopic, TellingDeliveries};

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
    deliveries: Arc<dyn Deliveries>,
    server: TestServer,
    live: TestServer,
}

impl Refused {
    fn nothing_yet() -> Self {
        let broadcaster = Broadcaster::new();
        let deliveries: Arc<dyn Deliveries> = Arc::new(TellingDeliveries::around(
            Arc::new(InMemoryDeliveries::new()),
            broadcaster.clone(),
        ));
        let dispatcher = Arc::new(InProcessDispatcher::queueing_to(deliveries.clone()));
        dispatcher.listen(Arc::new(Cataloguing));
        let server = TestServer::builder()
            .http_transport()
            .build(refusals::router(
                deliveries.clone(),
                dispatcher,
                Arc::new(FixedClock::new(ASKED)),
            ));
        let live = TestServer::builder().http_transport().build(
            live::live_socket(live::Topics::of(vec![Arc::new(RefusalsTopic {
                broadcaster,
                operators: None,
            })]))
            .layer(Extension(Agent::Local)),
        );

        Self {
            deliveries,
            server,
            live,
        }
    }

    async fn followed(&self) -> Subscriber {
        let mut socket = Subscriber::on(&self.live).await;
        socket.subscribe("refusals", "refusals", &[]).await;

        socket
    }

    async fn given_up_on(&self, routing: &str) {
        self.given_up_by(CATALOGUING, routing).await;
    }

    async fn given_up_by(&self, listener: &str, routing: &str) {
        self.given_up_on_what_happened(listener, routing, OCCURRED)
            .await;
    }

    async fn given_up_on_what_happened(
        &self,
        listener: &str,
        routing: &str,
        occurred: OffsetDateTime,
    ) {
        let message = Message::opening(
            RoutingKey::parse(routing).expect("a plain key is fine"),
            serde_json::json!({ "nothing": "much" }),
            occurred,
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
            .give_up(&claimed[0], "the projection had not landed", GIVEN_UP)
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
    // given: nothing refused
    let refused = Refused::nothing_yet();

    // when: the refusals are listed
    let listed = refused.listed().await;

    // then: the list is empty
    assert!(listed.is_empty());
}

#[tokio::test]
async fn a_refused_message_is_listed_with_what_why_and_when() {
    // given: a refused message
    let refused = Refused::nothing_yet();
    refused.given_up_on("idea.captured").await;

    // when: the refusals are listed
    let listed = refused.listed().await;

    // then: it is listed with what, why and when
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
    // given: a refused message
    let refused = Refused::nothing_yet();
    refused.given_up_on("idea.captured").await;

    // when: the refusals are listed
    let listed = refused.listed().await;

    // then: it says in plain words what the listener could not do
    assert_eq!(
        listed[0].plainly.as_deref(),
        Some("A change to an idea did not reach the list of ideas."),
        "the listener says it itself, so a new one cannot be added without words for the author"
    );
}

#[tokio::test]
async fn a_refusal_by_a_listener_no_longer_wired_has_no_plain_words() {
    // given: a message refused by a listener no longer wired
    let refused = Refused::nothing_yet();
    refused.given_up_by(UNWIRED, "idea.captured").await;

    // when: the refusals are listed
    let listed = refused.listed().await;

    // then: it has no plain words
    assert_eq!(
        listed[0].plainly, None,
        "a dead letter outlives the code that refused it, so the client needs its own fallback"
    );
}

#[tokio::test]
async fn trying_again_offers_the_message_afresh_from_now() {
    // given: a refused message
    let refused = Refused::nothing_yet();
    refused.given_up_on("idea.captured").await;
    let id = refused.listed().await[0].id;

    // when: it is tried again
    let response = refused.server.post(&format!("/refusals/{id}/retry")).await;

    // then: it is no longer listed
    response.assert_status(StatusCode::NO_CONTENT);
    assert!(refused.listed().await.is_empty());
    // and: it is offered afresh from now, not before
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
    // given: two refused messages
    let refused = Refused::nothing_yet();
    refused.given_up_on("idea.captured").await;
    refused.given_up_on("idea.retitled").await;
    let id = refused.listed().await[0].id;

    // when: the first is acknowledged
    let response = refused
        .server
        .post(&format!("/refusals/{id}/acknowledge"))
        .await;

    // then: both stay listed with only the first marked seen
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
    // given: nothing refused
    let refused = Refused::nothing_yet();

    // when: a refusal that is gone is tried again and acknowledged
    let retried = refused.server.post("/refusals/404/retry").await;
    let acknowledged = refused.server.post("/refusals/404/acknowledge").await;

    // then: both are done
    retried.assert_status(StatusCode::NO_CONTENT);
    acknowledged.assert_status(StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn an_id_that_is_not_a_number_is_a_bad_request() {
    // given: nothing refused
    let refused = Refused::nothing_yet();

    // when: a refusal is tried again by an id that is not a number
    let response = refused.server.post("/refusals/latest/retry").await;

    // then: it is a bad request
    response.assert_status(StatusCode::BAD_REQUEST);
}

async fn told(socket: &mut Subscriber) -> serde_json::Value {
    socket.frame().await
}

#[tokio::test]
async fn whoever_follows_the_refusals_is_told_when_a_delivery_is_given_up() {
    // given: someone following the refusals
    let refused = Refused::nothing_yet();
    let mut socket = refused.followed().await;

    // when: a delivery is given up
    refused.given_up_on("idea.captured").await;

    // then: they are told the refusals changed
    assert_eq!(told(&mut socket).await["event"]["name"], "REFUSALS_CHANGED");
}

#[tokio::test]
async fn acknowledging_and_retrying_are_told_too_each_with_a_newer_version() {
    // given: two refused messages
    let refused = Refused::nothing_yet();
    refused.given_up_on("idea.captured").await;
    refused.given_up_on("idea.retitled").await;
    let listed = refused.listed().await;
    // and: someone following the refusals
    let mut socket = refused.followed().await;

    // when: one is acknowledged
    refused
        .server
        .post(&format!("/refusals/{}/acknowledge", listed[0].id))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    let acknowledged = told(&mut socket).await;
    // and: the other is tried again
    refused
        .server
        .post(&format!("/refusals/{}/retry", listed[1].id))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    let retried = told(&mut socket).await;

    // then: the later telling has the newer version
    assert!(retried["version"].as_u64() > acknowledged["version"].as_u64());
}

#[tokio::test]
async fn a_delivery_refused_but_offered_again_tells_nothing() {
    // given: someone following the refusals
    let refused = Refused::nothing_yet();
    let mut socket = refused.followed().await;
    // and: a delivery taken up for handling
    let message = Message::opening(
        RoutingKey::parse("idea.captured").expect("a plain key is fine"),
        serde_json::json!({ "nothing": "much" }),
        OCCURRED,
    );
    refused
        .deliveries
        .enqueue(
            &ListenerName::parse(CATALOGUING).expect("a plain name is fine"),
            &message,
        )
        .await
        .expect("enqueuing should succeed");
    let claimed = refused
        .deliveries
        .claim_due(OCCURRED, 16)
        .await
        .expect("claiming should succeed");

    // when: it is refused to be offered again
    refused
        .deliveries
        .mark_as_refused(&claimed[0], "not yet", GIVEN_UP)
        .await
        .expect("refusing should succeed");

    // then: nothing is told
    assert!(
        socket.silent().await,
        "a delivery that will be offered again is not a refusal anyone needs to see"
    );
}

#[tokio::test]
async fn a_change_is_tried_again_for_thirty_days_after_it_happened_and_not_after() {
    // given: a change refused thirty days after it happened
    let refused = Refused::nothing_yet();
    refused
        .given_up_on_what_happened(CATALOGUING, "idea.captured", ASKED - RETRIABLE_FOR)
        .await;
    // and: one refused a moment later than that
    refused
        .given_up_on_what_happened(
            CATALOGUING,
            "idea.retitled",
            ASKED - RETRIABLE_FOR - Duration::seconds(1),
        )
        .await;

    // when: the refusals are listed
    let listed = refused.listed().await;

    // then: only the first may be tried again
    let triable: Vec<(String, bool)> = listed
        .iter()
        .map(|refusal| (refusal.routing.clone(), refusal.may_be_tried_again))
        .collect();
    assert_eq!(
        triable,
        vec![
            ("idea.captured".to_owned(), true),
            ("idea.retitled".to_owned(), false),
        ],
        "the list says which may be tried again, so the page need not know the limit"
    );
    // when: the second is tried again
    let too_old = listed[1].id;
    let response = refused
        .server
        .post(&format!("/refusals/{too_old}/retry"))
        .await;
    // then: it is refused as too late
    response.assert_status(StatusCode::CONFLICT);
    assert_eq!(
        response.json::<serde_json::Value>()["type"],
        refused::TOO_LATE_TO_TRY_AGAIN
    );
    // and: it stays listed
    assert_eq!(
        refused.listed().await.len(),
        2,
        "a change refused for being too old stays listed, to be acknowledged"
    );
    // and: the first may still be tried again
    refused
        .server
        .post(&format!("/refusals/{}/retry", listed[0].id))
        .await
        .assert_status(StatusCode::NO_CONTENT);
}
