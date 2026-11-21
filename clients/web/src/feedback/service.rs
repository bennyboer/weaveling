use std::collections::HashMap;

use feedback_contract::{
    AboutDTO, COUNTED_AT_ONCE, CommentBlockDTO, DraftFeedbackRequest, FeedbackCountDTO,
    FeedbackDTO, PendingDTO, RewriteFeedbackRequest,
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

/// Submits your drafts on a reading copy together, so everyone in its project sees them; answers with what was submitted.
pub async fn submit_on_reading_copy(copy: &ReadingCopyId) -> Result<Vec<Feedback>, ApiError> {
    let response = http::sent(Request::post(&format!(
        "{FEEDBACK}/submitted?reading_copies={copy}"
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
            "{FEEDBACK}/counts?reading_copies={ids}"
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

/// Where feedback is asked for: some reading copies; or, in every copy, some scenes, on them or passages in them,
/// or some sections themselves.
pub enum On<'a> {
    ReadingCopies(&'a [ReadingCopyId]),
    Scenes(&'a [SceneId]),
    Sections(&'a [SectionId]),
}

impl On<'_> {
    /// The places' name in the query, and their ids.
    fn asked(&self) -> (&'static str, Vec<String>) {
        fn ids<T: ToString>(listed: &[T]) -> Vec<String> {
            listed.iter().map(ToString::to_string).collect()
        }

        match self {
            Self::ReadingCopies(copies) => ("reading_copies", ids(copies)),
            Self::Scenes(scenes) => ("scenes", ids(scenes)),
            Self::Sections(sections) => ("sections", ids(sections)),
        }
    }
}

/// What the feedback answers on these places, asked in as many questions as it looks on places at once.
async fn listed(route: &str, on: &On<'_>, only: &str) -> Result<Vec<Feedback>, ApiError> {
    let (named, ids) = on.asked();
    let mut found = Vec::new();
    for batch in ids.chunks(COUNTED_AT_ONCE) {
        let response = http::sent(Request::get(&format!(
            "{FEEDBACK}/{route}?{named}={}{only}",
            batch.join(",")
        )))
        .await?;
        let told: Vec<FeedbackDTO> = http::parsed(response, SUBJECT).await?;
        for piece in told {
            found.push(Feedback::try_from(piece)?);
        }
    }

    Ok(found)
}

/// Your own drafts where asked, oldest first, which only you see until you submit them.
pub async fn drafts(on: On<'_>) -> Result<Vec<Feedback>, ApiError> {
    listed("drafts", &on, "").await
}

/// The feedback submitted where asked, in every copy, oldest first, and if asked only what is pending or done (`status`);
/// as many scenes or sections to a question as the feedback looks on at once.
pub async fn submitted(
    on: On<'_>,
    status: Option<&'static str>,
) -> Result<Vec<Feedback>, ApiError> {
    let only = status
        .map(|status| format!("&status={status}"))
        .unwrap_or_default();

    listed("on", &on, &only).await
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
