mod sweeping;
mod tidying;

pub use sweeping::{AT_MOST, DeleteOnProjectDeleted, when_more_to_sweep, when_project_deleted};
pub use tidying::{DeleteOnDiscard, when_discarded};
