use std::sync::Arc;

use axum::Extension;
use axum::http::StatusCode;
use axum_test::{TestResponse, TestServer};
use clock::FixedClock;
use eventsourcing::InMemoryEventStore;
use eventsourcing::{AccountId, Agent};
use feedback_catalog::{
    InMemoryAccountNameCatalog, InMemoryFeedbackCatalog, InMemoryReadingCopyCatalog,
};
use feedback_contract::{
    AboutDTO, CommentBlockDTO, CommentRunDTO, DraftFeedbackRequest, FeedbackDTO,
    ReadingCopyPlaceDTO,
};
use feedback_core::{FeedbackCore, KnownReadingCopy, ProjectLink, ReadingCopyLink};
use serde_json::Value;
use time::OffsetDateTime;
use transacting::InMemoryTransactions;

pub const READING_COPY: &str = "copy_loom";

/// When the tests' clock stands, unless a test moves it.
pub const NOW: i64 = 1_700_000_000;

/// A copy of the loom project that closes a day after `NOW`.
pub const CLOSING_COPY: &str = "copy_closing";

/// The feedback feature at `NOW`.
pub async fn feedback() -> FeedbackCore {
    feedback_on(Arc::new(FixedClock::new(clock::at(NOW)))).await
}

/// The feedback feature on a clock a test may move, having heard that the loom, the weft and the closing copy
/// were made of the loom project, the closing one open for a day, and the shuttle copy of another;
/// and that ada and bea signed up as Ada and Bea.
pub async fn feedback_on(clock: Arc<FixedClock>) -> FeedbackCore {
    let feedback = FeedbackCore::new(
        Arc::new(InMemoryEventStore::new()),
        Arc::new(InMemoryFeedbackCatalog::new()),
        Arc::new(InMemoryReadingCopyCatalog::new()),
        Arc::new(InMemoryAccountNameCatalog::new()),
        Arc::new(InMemoryTransactions::new()),
        clock,
    );
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
