use transacting::Transaction;

use crate::id::FeedbackId;
use crate::link::ProjectLink;

/// Tells whoever follows a project's feedback live that a piece given there changed or is gone,
/// once the change is committed.
/// Drafts are never told of, as only whoever writes them sees them.
pub trait FeedbackFollowers: Send + Sync {
    fn given_changed(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        piece: &FeedbackId,
    );

    fn given_gone(&self, transaction: &mut Transaction, project: &ProjectLink, piece: &FeedbackId);
}

/// No followers at all, for a core whose feedback nobody follows live.
pub struct NobodyFollows;

impl FeedbackFollowers for NobodyFollows {
    fn given_changed(
        &self,
        _transaction: &mut Transaction,
        _project: &ProjectLink,
        _piece: &FeedbackId,
    ) {
    }

    fn given_gone(
        &self,
        _transaction: &mut Transaction,
        _project: &ProjectLink,
        _piece: &FeedbackId,
    ) {
    }
}
