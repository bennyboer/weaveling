use std::sync::Arc;

use axum::Extension;
use axum::http::StatusCode;
use axum_test::{TestResponse, TestServer};
use clock::FixedClock;
use eventsourcing::{AccountId, Agent};
use feedback_contract::{
    AboutDTO, CommentBlockDTO, CommentRunDTO, DraftFeedbackRequest, FeedbackDTO,
    ReadingCopyPlaceDTO,
};
use feedback_core::{FeedbackCore, ReadingCopyLink};
use serde_json::Value;

pub const READING_COPY: &str = "copy_loom";

/// The feedback feature, having heard that the loom and the weft copy were made of the loom project,
/// and the shuttle copy of another.
pub fn feedback() -> FeedbackCore {
    let feedback = FeedbackCore::new(Arc::new(FixedClock::new(clock::at(1_700_000_000))));
    feedback.remember_copy(ReadingCopyLink::from(READING_COPY), "project_loom");
    feedback.remember_copy(ReadingCopyLink::from("copy_weft"), "project_loom");
    feedback.remember_copy(ReadingCopyLink::from("copy_shuttle"), "project_shuttle");

    feedback
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
        format!("/feedback/on?reading_copies={READING_COPY}"),
        format!("/feedback/drafts?reading_copies={READING_COPY}"),
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
