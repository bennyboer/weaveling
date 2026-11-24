use std::collections::HashMap;

use feedback_contract::{
    AboutDTO, AskedDTO, COUNTED_AT_ONCE, CommentBlockDTO, DraftFeedbackRequest, FeedbackCountDTO,
    FeedbackDTO, GiverDTO, ListedGiverDTO, PendingDTO, PlaceDTO, RewriteFeedbackRequest,
    SubjectDTO, refused,
};
use gloo_net::http::Request;

use crate::feedback::model::{
    About, CommentBlock, Feedback, FeedbackCount, FeedbackId, Giver, Pending,
};
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
        "{FEEDBACK}/submitted?{}",
        Asked::in_copy(copy).query()
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
        let asked = AskedDTO {
            subjects: batch
                .iter()
                .map(|copy| SubjectDTO::ReadingCopy(copy.to_string()))
                .collect(),
            ..AskedDTO::default()
        };
        let response = http::sent(Request::get(&format!(
            "{FEEDBACK}/counts?{}",
            asked.to_query()
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

/// Which feedback is asked for: in a project, in some reading copies, on some scenes and sections of their text.
/// Each narrows what the others leave.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Asked {
    pub project: Option<ProjectId>,
    pub reading_copies: Vec<ReadingCopyId>,
    pub scenes: Vec<SceneId>,
    pub sections: Vec<SectionId>,
    /// Who gave it, as the feedback named them when it listed them.
    pub given_by: Vec<String>,
}

impl Asked {
    /// Everything in a project.
    pub fn in_project(project: &ProjectId) -> Self {
        Self {
            project: Some(project.clone()),
            ..Self::default()
        }
    }

    /// Everything in one reading copy.
    pub fn in_copy(copy: &ReadingCopyId) -> Self {
        Self {
            reading_copies: vec![copy.clone()],
            ..Self::default()
        }
    }

    /// The query it is asked with, as the contract names subjects and places.
    fn query(&self) -> String {
        AskedDTO {
            project: self.project.as_ref().map(ToString::to_string),
            subjects: self
                .reading_copies
                .iter()
                .map(|copy| SubjectDTO::ReadingCopy(copy.to_string()))
                .collect(),
            places: self
                .sections
                .iter()
                .map(|section| PlaceDTO::Section(section.to_string()))
                .chain(
                    self.scenes
                        .iter()
                        .map(|scene| PlaceDTO::Scene(scene.to_string())),
                )
                .collect(),
            given_by: GiverDTO::listed(&self.given_by.join(",")).unwrap_or_default(),
        }
        .to_query()
    }

    /// The questions it is asked in, none looking on more than the feedback looks on at once:
    /// the scenes, the one list that grows long, are parted among them, the sections asked with the first alone.
    fn in_questions(&self) -> Vec<Self> {
        let room = COUNTED_AT_ONCE
            .saturating_sub(self.reading_copies.len() + self.sections.len())
            .max(1);
        if self.scenes.len() <= room {
            return vec![self.clone()];
        }

        self.scenes
            .chunks(room)
            .enumerate()
            .map(|(at, scenes)| Self {
                scenes: scenes.to_vec(),
                sections: if at == 0 {
                    self.sections.clone()
                } else {
                    Vec::new()
                },
                ..self.clone()
            })
            .collect()
    }
}

/// What the feedback answers where asked, in as many questions as it looks on places at once.
async fn listed(route: &str, asked: &Asked, only: &str) -> Result<Vec<Feedback>, ApiError> {
    let mut found = Vec::new();
    for question in asked.in_questions() {
        let response = http::sent(Request::get(&format!(
            "{FEEDBACK}/{route}?{}{only}",
            question.query()
        )))
        .await?;
        let told: Vec<FeedbackDTO> = http::parsed(response, SUBJECT).await?;
        for piece in told {
            found.push(Feedback::try_from(piece)?);
        }
    }

    Ok(found)
}

/// The pieces asked for by their ids, as you find them: those not there for you any more are left out.
/// They are asked for as many at once as the feedback counts at once.
pub async fn pieces(ids: &[FeedbackId]) -> Result<Vec<Feedback>, ApiError> {
    let mut found = Vec::new();
    for some in ids.chunks(feedback_contract::COUNTED_AT_ONCE) {
        let named = some
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let response = http::sent(Request::get(&format!("{FEEDBACK}/pieces?ids={named}"))).await?;
        let told: Vec<FeedbackDTO> = http::parsed(response, SUBJECT).await?;
        for piece in told {
            found.push(Feedback::try_from(piece)?);
        }
    }

    Ok(found)
}

/// Your own drafts where asked, oldest first, which only you see until you submit them.
pub async fn drafts(asked: &Asked) -> Result<Vec<Feedback>, ApiError> {
    listed("drafts", asked, "").await
}

/// The feedback submitted where asked, in every copy, oldest first, and if asked only what is pending or done (`status`).
pub async fn submitted(
    asked: &Asked,
    status: Option<&'static str>,
) -> Result<Vec<Feedback>, ApiError> {
    let only = status
        .map(|status| format!("&status={status}"))
        .unwrap_or_default();

    listed("on", asked, &only).await
}

/// One page of the feedback submitted where asked: in an order, after a piece, and at most so many,
/// and if asked only what is pending or done.
pub async fn page_in(
    asked: &Asked,
    status: Option<&'static str>,
    order: &'static str,
    after: Option<&FeedbackId>,
    at_most: usize,
) -> Result<Vec<Feedback>, ApiError> {
    let only = status
        .map(|status| format!("&status={status}"))
        .unwrap_or_default();
    let after = after
        .map(|after| format!("&after={after}"))
        .unwrap_or_default();
    let response = http::sent(Request::get(&format!(
        "{FEEDBACK}/on?{}&order={order}&limit={at_most}{only}{after}",
        asked.query()
    )))
    .await?;
    let found: Vec<FeedbackDTO> = http::parsed(response, SUBJECT).await?;

    found.into_iter().map(Feedback::try_from).collect()
}

/// How much feedback given where asked is not done yet, in every copy: on each scene, on each section itself, and in all.
pub async fn pending(asked: &Asked) -> Result<Pending, ApiError> {
    let response = http::sent(Request::get(&format!(
        "{FEEDBACK}/pending?{}",
        asked.query()
    )))
    .await?;
    let told: PendingDTO = http::parsed(response, SUBJECT).await?;

    Ok(Pending::from(told))
}

/// Whoever gave feedback in a project, in the order they first submitted any.
pub async fn givers_in(project: &ProjectId) -> Result<Vec<Giver>, ApiError> {
    let response = http::sent(Request::get(&format!(
        "{FEEDBACK}/givers?project={project}"
    )))
    .await?;
    let told: Vec<ListedGiverDTO> = http::parsed(response, SUBJECT).await?;

    Ok(told.into_iter().map(Giver::from).collect())
}

/// How often a draft in a copy just made is asked again while the feedback has not heard of the copy yet, and how long between.
const ASKED_AGAIN_AT_MOST: u32 = 20;
const ASKED_AGAIN_AFTER_MILLISECONDS: i32 = 150;

/// Starts a draft in a reading copy just made, asking again for a few seconds while the feedback has not heard of the copy yet,
/// as it hears of each copy a moment after it is made.
pub async fn draft_in_new_copy(
    about: &About,
    comment: &[CommentBlock],
) -> Result<Feedback, ApiError> {
    let mut asked = 1;
    loop {
        match draft(about, comment).await {
            Err(failed)
                if asked < ASKED_AGAIN_AT_MOST
                    && http::refused_as(&failed, refused::READING_COPY_UNKNOWN) =>
            {
                asked += 1;
                http::a_moment(ASKED_AGAIN_AFTER_MILLISECONDS).await;
            }
            drafted => return drafted,
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn scenes(count: usize) -> Vec<SceneId> {
        (0..count)
            .map(|at| SceneId::from(format!("scene_{at}")))
            .collect()
    }

    #[test]
    fn the_narrowings_are_asked_together_in_one_query() {
        // given: a project, a scene and a section asked for together
        let asked = Asked {
            scenes: scenes(1),
            sections: vec![SectionId::from("section_morning".to_owned())],
            ..Asked::in_project(&ProjectId::from("project_loom".to_owned()))
        };

        // expect: one query naming each of them
        assert_eq!(
            asked.query(),
            "project=project_loom&places=section:section_morning,scene:scene_0"
        );
    }

    #[test]
    fn many_scenes_are_parted_among_questions_and_the_sections_asked_once() {
        // given: more scenes than are looked on at once, beside a copy and a section
        let asked = Asked {
            scenes: scenes(COUNTED_AT_ONCE + 10),
            sections: vec![SectionId::from("section_morning".to_owned())],
            ..Asked::in_copy(&ReadingCopyId::from("copy_loom".to_owned()))
        };

        // when: it is parted into questions
        let questions = asked.in_questions();

        // then: none looks on more than at once, every scene is asked once, and the copy narrows each
        assert!(questions.iter().all(|question| {
            question.scenes.len() + question.sections.len() + question.reading_copies.len()
                <= COUNTED_AT_ONCE
        }));
        assert_eq!(
            questions
                .iter()
                .flat_map(|question| question.scenes.clone())
                .collect::<Vec<_>>(),
            asked.scenes
        );
        assert!(
            questions
                .iter()
                .all(|question| question.reading_copies == asked.reading_copies)
        );
        // and: the section is asked with the first question alone
        assert_eq!(
            questions
                .iter()
                .map(|question| question.sections.len())
                .collect::<Vec<_>>(),
            [1, 0]
        );
    }
}
