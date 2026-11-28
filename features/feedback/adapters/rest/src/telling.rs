use std::sync::Arc;

use feedback_contract::{GivenFeedbackChangeDTO, SubjectDTO};
use feedback_core::{FeedbackFollowers, FeedbackId, ProjectLink, Subject};
use live::{Broadcaster, Teller};
use transacting::Transaction;

use crate::GivenFeedbackChanges;

/// Tells whoever follows a project's feedback live of each piece given there that changed, once it is committed.
pub struct TellingFollowers {
    teller: Arc<Teller<GivenFeedbackChanges>>,
}

impl TellingFollowers {
    pub fn new(broadcaster: Broadcaster<GivenFeedbackChanges>) -> Self {
        Self {
            teller: Arc::new(Teller::new(broadcaster)),
        }
    }

    fn told(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        piece: &FeedbackId,
        change: GivenFeedbackChangeDTO,
    ) {
        let (teller, project, piece) = (self.teller.clone(), project.clone(), piece.to_string());

        transaction.after_commit(async move {
            teller.tell(GivenFeedbackChanges(project), &piece, change);
        });
    }
}

impl FeedbackFollowers for TellingFollowers {
    fn given_changed(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        piece: &FeedbackId,
        subject: &Subject,
    ) {
        self.told(
            transaction,
            project,
            piece,
            GivenFeedbackChangeDTO::Changed {
                subject: named(subject),
            },
        );
    }

    fn given_gone(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        piece: &FeedbackId,
        subject: &Subject,
    ) {
        self.told(
            transaction,
            project,
            piece,
            GivenFeedbackChangeDTO::Gone {
                subject: named(subject),
            },
        );
    }
}

/// A subject as `subjects=` names it.
fn named(subject: &Subject) -> String {
    match subject {
        Subject::ReadingCopy(copy) => SubjectDTO::ReadingCopy(copy.to_string()),
        Subject::Feedback(piece) => SubjectDTO::Feedback(piece.to_string()),
    }
    .to_string()
}
