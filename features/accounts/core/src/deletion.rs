use time::{Duration, OffsetDateTime};

/// How long an account is kept after its deletion was asked for, before it is deleted for good.
/// During that time it can still be kept after all.
pub const KEPT_BEFORE_DELETION: Duration = Duration::days(30);

/// An account whose deletion was asked for.
/// It holds until when the account is kept, and who asked.
/// Who asked matters, because only an operator can call off a deletion that an operator asked for.
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
