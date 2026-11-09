use time::{Duration, OffsetDateTime};

/// How long an account asked to be deleted is kept, to be kept after all, before it is gone for good.
pub const KEPT_BEFORE_DELETION: Duration = Duration::days(30);

/// An account asked to be deleted: until when it is kept, and who asked, as only the operator calls off what the operator asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingDeletion {
    pub until: OffsetDateTime,
    pub asked_by: DeletionAskedBy,
}

/// Who asked for an account to be deleted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeletionAskedBy {
    Holder,
    Operator,
}
