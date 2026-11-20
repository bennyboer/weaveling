use feedback_contract::{
    AboutDTO, CommentBlockDTO, DraftFeedbackRequest, FeedbackDTO, RewriteFeedbackRequest, kind,
};
use gloo_net::http::Request;

use crate::feedback::model::{About, CommentBlock, Feedback, FeedbackId};
use crate::http;
use crate::http::ApiError;
use crate::readings::reading_copies::ReadingCopyId;

const FEEDBACK: &str = "/api/feedback";
const SUBJECT: &str = "comment";

/// The feedback given on a reading copy that you may see, oldest first.
pub async fn on_reading_copy(copy: &ReadingCopyId) -> Result<Vec<Feedback>, ApiError> {
    let response = http::sent(Request::get(&format!(
        "{FEEDBACK}?kind={}&id={copy}",
        kind::READING_COPY
    )))
    .await?;
    let found: Vec<FeedbackDTO> = http::parsed(response, SUBJECT).await?;

    found.into_iter().map(Feedback::try_from).collect()
}

/// Starts a draft, which only you see until you submit it.
pub async fn draft(about: &About, comment: &[CommentBlock]) -> Result<Feedback, ApiError> {
    let payload = DraftFeedbackRequest {
        about: AboutDTO::from(about),
        comment: comment.iter().map(CommentBlockDTO::from).collect(),
    };
    let response = http::sent_with(Request::post(FEEDBACK), &payload).await?;
    let drafted: FeedbackDTO = http::parsed(response, SUBJECT).await?;

    Feedback::try_from(drafted)
}

/// Rewrites what a piece of your feedback says.
pub async fn rewrite(
    feedback: &FeedbackId,
    comment: &[CommentBlock],
) -> Result<Feedback, ApiError> {
    let payload = RewriteFeedbackRequest {
        comment: comment.iter().map(CommentBlockDTO::from).collect(),
    };
    let response = http::sent_with(
        Request::put(&format!("{FEEDBACK}/{feedback}/comment")),
        &payload,
    )
    .await?;
    let rewritten: FeedbackDTO = http::parsed(response, SUBJECT).await?;

    Feedback::try_from(rewritten)
}

/// Deletes a piece of your feedback.
pub async fn delete(feedback: &FeedbackId) -> Result<(), ApiError> {
    let response = http::sent(Request::delete(&format!("{FEEDBACK}/{feedback}"))).await?;

    http::checked(response, SUBJECT).await?;

    Ok(())
}
