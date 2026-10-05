mod sweeping;
mod unlinking;

pub use sweeping::{AT_MOST, DeleteOnProjectDeleted, when_more_to_sweep, when_project_deleted};
pub use unlinking::{UnlinkOnDiscard, when_idea_discarded};
