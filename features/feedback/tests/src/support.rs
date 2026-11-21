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
use feedback_core::FeedbackCore;
use serde_json::Value;

pub const READING_COPY: &str = "copy_loom";

pub fn feedback() -> FeedbackCore {
    FeedbackCore::new(Arc::new(FixedClock::new(clock::at(1_700_000_000))))
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

pub async fn on_the_copy(server: &TestServer) -> Vec<FeedbackDTO> {
    let response = server
        .get(&format!("/feedback?kind=reading_copy&id={READING_COPY}"))
        .await;
    response.assert_status_ok();

    response.json::<Vec<FeedbackDTO>>()
}

pub fn refusal(response: &TestResponse) -> Value {
    response.json::<Value>()["type"].clone()
}
