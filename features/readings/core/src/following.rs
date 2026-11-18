use transacting::Transaction;

use crate::link::{ProjectLink, SceneLink};

/// Tells whoever follows a project's live book that a piece of it changed, once the change is committed.
/// Each is told only when the piece was kept, not when a late or repeated message changed nothing.
pub trait BookFollowers: Send + Sync {
    fn text_kept(&self, transaction: &mut Transaction, project: &ProjectLink, scene: &SceneLink);

    fn outline_kept(&self, transaction: &mut Transaction, project: &ProjectLink);

    fn title_kept(&self, transaction: &mut Transaction, project: &ProjectLink);
}

/// No followers at all, for a core whose books nobody follows live.
pub struct NobodyFollows;

impl BookFollowers for NobodyFollows {
    fn text_kept(
        &self,
        _transaction: &mut Transaction,
        _project: &ProjectLink,
        _scene: &SceneLink,
    ) {
    }

    fn outline_kept(&self, _transaction: &mut Transaction, _project: &ProjectLink) {}

    fn title_kept(&self, _transaction: &mut Transaction, _project: &ProjectLink) {}
}
