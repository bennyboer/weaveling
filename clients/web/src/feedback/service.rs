use std::collections::HashMap;

use feedback_contract::{
    AboutDTO, COUNTED_AT_ONCE, CommentBlockDTO, DraftFeedbackRequest, FeedbackCountDTO,
    FeedbackDTO, PendingDTO, RewriteFeedbackRequest, kind,
};
use gloo_net::http::Request;

use crate::feedback::model::{About, CommentBlock, Feedback, FeedbackCount, FeedbackId, Pending};
use crate::http;
use crate::http::ApiError;
use crate::outline::model::SectionId;
use crate::projects::model::ProjectId;
use crate::readings::reading_copies::ReadingCopyId;
use crate::scenes::model::SceneId;

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

/// Submits your drafts on a reading copy together, so everyone in its project sees them; answers with what was submitted.
pub async fn submit_on_reading_copy(copy: &ReadingCopyId) -> Result<Vec<Feedback>, ApiError> {
    let response = http::sent(Request::post(&format!(
        "{FEEDBACK}/submitted?kind={}&id={copy}",
        kind::READING_COPY
    )))
    .await?;
    let submitted: Vec<FeedbackDTO> = http::parsed(response, SUBJECT).await?;

    submitted.into_iter().map(Feedback::try_from).collect()
}

/// How much feedback each reading copy holds: what is not done yet, and your drafts.
/// It asks for as many copies at once as the feedback counts, so any number of copies takes few questions.
pub async fn counted_on_reading_copies(
    copies: &[ReadingCopyId],
) -> Result<HashMap<ReadingCopyId, FeedbackCount>, ApiError> {
    let mut counted = HashMap::new();
    for batch in copies.chunks(COUNTED_AT_ONCE) {
        let ids = batch
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let response = http::sent(Request::get(&format!(
            "{FEEDBACK}/counts?kind={}&ids={ids}",
            kind::READING_COPY
        )))
        .await?;
        let told: Vec<FeedbackCountDTO> = http::parsed(response, SUBJECT).await?;
        counted.extend(told.into_iter().map(|count| {
            (
                ReadingCopyId::from(count.id),
                FeedbackCount {
                    not_done: count.not_done,
                    your_drafts: count.your_drafts,
                },
            )
        }));
    }

    Ok(counted)
}

/// The feedback given on a scene, in every copy of it, oldest first.
pub async fn given_on_scene(scene: &SceneId) -> Result<Vec<Feedback>, ApiError> {
    let response = http::sent(Request::get(&format!("{FEEDBACK}/on?scene={scene}"))).await?;
    let found: Vec<FeedbackDTO> = http::parsed(response, SUBJECT).await?;

    found.into_iter().map(Feedback::try_from).collect()
}

/// The feedback given on a section itself, in every copy of it, oldest first.
pub async fn given_on_section(section: &SectionId) -> Result<Vec<Feedback>, ApiError> {
    let response = http::sent(Request::get(&format!("{FEEDBACK}/on?section={section}"))).await?;
    let found: Vec<FeedbackDTO> = http::parsed(response, SUBJECT).await?;

    found.into_iter().map(Feedback::try_from).collect()
}

/// How much feedback given is not done yet in a project, in every copy: on each scene, and on each section itself.
pub async fn pending_in(project: &ProjectId) -> Result<Pending, ApiError> {
    let response = http::sent(Request::get(&format!(
        "{FEEDBACK}/pending?project={project}"
    )))
    .await?;
    let told: PendingDTO = http::parsed(response, SUBJECT).await?;

    Ok(Pending::from(told))
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

/// Marks a piece of feedback given as done, or not done again.
pub async fn mark_done(feedback: &FeedbackId, done: bool) -> Result<Feedback, ApiError> {
    let at = format!("{FEEDBACK}/{feedback}/done");
    let response = http::sent(if done {
        Request::put(&at)
    } else {
        Request::delete(&at)
    })
    .await?;
    let marked: FeedbackDTO = http::parsed(response, SUBJECT).await?;

    Feedback::try_from(marked)
}

/// Deletes a piece of your feedback.
pub async fn delete(feedback: &FeedbackId) -> Result<(), ApiError> {
    let response = http::sent(Request::delete(&format!("{FEEDBACK}/{feedback}"))).await?;

    http::checked(response, SUBJECT).await?;

    Ok(())
}
