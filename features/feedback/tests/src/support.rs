use std::sync::{Arc, Mutex};

use access::{HearsAccess, InMemoryAccess, KeptAccess, Role};
use axum::Extension;
use axum::http::StatusCode;
use axum_test::{TestResponse, TestServer};
use clock::Clock;
use eventsourcing::{AccountId, Agent};
use eventsourcing::{InMemoryEventStore, Version};
use feedback_catalog::{
    InMemoryAccountNameCatalog, InMemoryFeedbackCatalog, InMemoryReadingCopyCatalog,
};
use feedback_contract::{
    AboutDTO, CommentBlockDTO, CommentRunDTO, DraftFeedbackRequest, FeedbackDTO,
    ReadingCopyPlaceDTO,
};
use feedback_core::{
    FeedbackCore, FeedbackEvent, FeedbackFollowers, KnownReadingCopy, NobodyFollows, ProjectLink,
    ReadingCopyLink,
};
use feedback_rest::{GivenFeedbackChanges, TellingFollowers};
use live::{Broadcaster, Topics};
use serde_json::Value;
use time::{Duration, OffsetDateTime};
use transacting::InMemoryTransactions;

pub const READING_COPY: &str = "copy_loom";

/// When the tests' clock stands, unless a test moves it.
pub const NOW: i64 = 1_700_000_000;

/// A copy of the loom project that closes a day after `NOW`.
pub const CLOSING_COPY: &str = "copy_closing";

/// A clock that moves on a millisecond each time it is read, so what happens one after another happens at times one after another,
/// as it does on a real clock; a test may set it to a time of its own.
#[derive(Debug)]
pub struct TickingClock {
    now: Mutex<OffsetDateTime>,
}

impl TickingClock {
    pub fn at(now: OffsetDateTime) -> Self {
        Self {
            now: Mutex::new(now),
        }
    }

    pub fn set(&self, now: OffsetDateTime) {
        *self.now.lock().expect("clock lock poisoned") = now;
    }
}

impl Clock for TickingClock {
    fn now(&self) -> OffsetDateTime {
        let mut now = self.now.lock().expect("clock lock poisoned");
        let read = *now;
        *now += Duration::milliseconds(1);

        read
    }
}

/// The feedback feature from `NOW` on.
pub async fn feedback() -> FeedbackCore {
    feedback_on(Arc::new(TickingClock::at(clock::at(NOW)))).await
}

/// The feedback feature on a clock a test may move, having heard that the loom, the weft and the closing copy
/// were made of the loom project, the closing one open for a day, and the shuttle copy of another;
/// that ada and bea signed up as Ada and Bea; that ada owns the loom project, otto and hedy write it,
/// and bea reads it; and that ada owns the shuttle project, of which bea is no member.
pub async fn feedback_on(clock: Arc<TickingClock>) -> FeedbackCore {
    feedback_keeping(
        clock,
        Arc::new(InMemoryEventStore::new()),
        Arc::new(NobodyFollows),
    )
    .await
}

/// The feedback feature at `NOW` with the event store it keeps its pieces in, for a test to look into.
pub async fn feedback_with_events() -> (FeedbackCore, Arc<InMemoryEventStore<FeedbackEvent>>) {
    let events = Arc::new(InMemoryEventStore::new());
    let feedback = feedback_keeping(
        Arc::new(TickingClock::at(clock::at(NOW))),
        events.clone(),
        Arc::new(NobodyFollows),
    )
    .await;

    (feedback, events)
}

/// The feedback, telling whoever follows its projects live on the broadcaster it answers with.
pub async fn feedback_followed() -> (FeedbackCore, Broadcaster<GivenFeedbackChanges>) {
    let broadcaster = Broadcaster::new();
    let feedback = feedback_keeping(
        Arc::new(TickingClock::at(clock::at(NOW))),
        Arc::new(InMemoryEventStore::new()),
        Arc::new(TellingFollowers::new(broadcaster.clone())),
    )
    .await;

    (feedback, broadcaster)
}

/// The live socket an account follows the feedback on.
pub fn socket_as(
    feedback: &FeedbackCore,
    broadcaster: &Broadcaster<GivenFeedbackChanges>,
    account: &str,
) -> TestServer {
    TestServer::builder().http_transport().build(
        live::live_socket(Topics::of(vec![feedback_rest::topic(
            feedback.clone(),
            broadcaster.clone(),
        )]))
        .layer(Extension(Agent::Account(AccountId::from(account)))),
    )
}

async fn feedback_keeping(
    clock: Arc<TickingClock>,
    events: Arc<InMemoryEventStore<FeedbackEvent>>,
    followers: Arc<dyn FeedbackFollowers>,
) -> FeedbackCore {
    let access = KeptAccess::new(Arc::new(InMemoryAccess::new()));
    let feedback = FeedbackCore::new(
        events,
        Arc::new(InMemoryFeedbackCatalog::new()),
        Arc::new(InMemoryReadingCopyCatalog::new()),
        Arc::new(InMemoryAccountNameCatalog::new()),
        followers,
        Arc::new(InMemoryTransactions::new()),
        access.clone(),
        Arc::new(access),
        clock,
    );
    for (version, project, account, role) in [
        (1, "project_loom", "account_ada", Role::Owner),
        (2, "project_loom", "account_bea", Role::Reader),
        (3, "project_loom", "account_otto", Role::Author),
        (4, "project_loom", "account_hedy", Role::Author),
        (1, "project_shuttle", "account_ada", Role::Owner),
    ] {
        feedback
            .remember_role(
                None,
                project,
                &AccountId::from(account),
                Version::of(version),
                Some(role),
            )
            .await
            .expect("hearing a role should succeed");
    }
    for (account, name) in [("account_ada", "Ada"), ("account_bea", "Bea")] {
        feedback
            .name_heard(None, &AccountId::from(account), name)
            .await
            .expect("hearing a name should succeed");
    }
    for (copy, project, open_until) in [
        (READING_COPY, "project_loom", None),
        ("copy_weft", "project_loom", None),
        (
            CLOSING_COPY,
            "project_loom",
            Some(clock::at(NOW + 24 * 60 * 60)),
        ),
        ("copy_shuttle", "project_shuttle", None),
    ] {
        heard_of(&feedback, copy, project, open_until).await;
    }

    feedback
}

async fn heard_of(
    feedback: &FeedbackCore,
    copy: &str,
    project: &str,
    open_until: Option<OffsetDateTime>,
) {
    feedback
        .reading_copy_heard(
            None,
            &ReadingCopyLink::from(copy),
            &KnownReadingCopy {
                project: ProjectLink::from(project),
                open_until,
            },
            1,
        )
        .await
        .expect("hearing of a copy should succeed");
}

pub fn as_(feedback: &FeedbackCore, account: &str) -> TestServer {
    TestServer::new(
        feedback_rest::router(feedback.clone())
            .layer(Extension(Agent::Account(AccountId::from(account)))),
    )
}

pub fn in_the_copy(place: ReadingCopyPlaceDTO) -> AboutDTO {
    AboutDTO::ReadingCopy {
        reading_copy: READING_COPY.to_owned(),
        place,
    }
}

pub fn a_passage() -> AboutDTO {
    in_the_copy(ReadingCopyPlaceDTO::Passage {
        scene: "scene_harbour".to_owned(),
        block_path: vec![0],
        from: 4,
        to: 9,
        quote: "boats".to_owned(),
    })
}

/// A comment of one plain paragraph.
pub fn saying(text: &str) -> Vec<CommentBlockDTO> {
    vec![CommentBlockDTO::Paragraph {
        runs: vec![CommentRunDTO::Text {
            text: text.to_owned(),
            emphasis: false,
            strong: false,
            link: None,
        }],
    }]
}

pub async fn drafted(
    server: &TestServer,
    about: AboutDTO,
    comment: Vec<CommentBlockDTO>,
) -> FeedbackDTO {
    let response = server
        .post("/feedback")
        .json(&DraftFeedbackRequest { about, comment })
        .await;
    response.assert_status(StatusCode::CREATED);

    response.json::<FeedbackDTO>()
}

/// What one sees on the copy: everyone's feedback submitted, then one's own drafts, as the reading view asks for both.
pub async fn on_the_copy(server: &TestServer) -> Vec<FeedbackDTO> {
    let mut seen = Vec::new();
    for asked in [
        format!("/feedback/on?subjects=reading_copy:{READING_COPY}"),
        format!("/feedback/drafts?subjects=reading_copy:{READING_COPY}"),
    ] {
        let response = server.get(&asked).await;
        response.assert_status_ok();
        seen.extend(response.json::<Vec<FeedbackDTO>>());
    }

    seen
}

pub fn refusal(response: &TestResponse) -> Value {
    response.json::<Value>()["type"].clone()
}
