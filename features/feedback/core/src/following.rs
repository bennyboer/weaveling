use transacting::Transaction;

use crate::about::Subject;
use crate::id::FeedbackId;
use crate::link::ProjectLink;

/// Tells whoever follows a project's feedback live that a piece given there changed or is gone, naming what it is given on,
/// once the change is committed.
/// Drafts are never told of, as only whoever writes them sees them.
pub trait FeedbackFollowers: Send + Sync {
    fn given_changed(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        piece: &FeedbackId,
        subject: &Subject,
    );

    fn given_gone(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        piece: &FeedbackId,
        subject: &Subject,
    );
}

/// No followers at all, for a core whose feedback nobody follows live.
pub struct NobodyFollows;

impl FeedbackFollowers for NobodyFollows {
    fn given_changed(
        &self,
        _transaction: &mut Transaction,
        _project: &ProjectLink,
        _piece: &FeedbackId,
        _subject: &Subject,
    ) {
    }

    fn given_gone(
        &self,
        _transaction: &mut Transaction,
        _project: &ProjectLink,
        _piece: &FeedbackId,
        _subject: &Subject,
    ) {
    }
}
