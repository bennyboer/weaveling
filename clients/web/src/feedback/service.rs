use feedback_contract::{FeedbackDTO, kind};
use gloo_net::http::Request;

use crate::feedback::model::{Feedback, FeedbackId};
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

/// Deletes a piece of your feedback.
pub async fn delete(feedback: &FeedbackId) -> Result<(), ApiError> {
    let response = http::sent(Request::delete(&format!("{FEEDBACK}/{feedback}"))).await?;

    http::checked(response, SUBJECT).await?;

    Ok(())
}
