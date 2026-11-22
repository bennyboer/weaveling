use std::collections::HashMap;
use std::error::Error;

use async_trait::async_trait;
use thiserror::Error;
use time::OffsetDateTime;
use transacting::Transaction;

use crate::link::{ProjectLink, ReadingCopyLink};

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("what the feedback feature keeps could not be reached")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
}

/// A reading copy as the feedback feature heard of it from the readings:
/// the project it was made of, and until when it is open for feedback, none for no end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownReadingCopy {
    pub project: ProjectLink,
    pub open_until: Option<OffsetDateTime>,
}

impl KnownReadingCopy {
    /// Whether feedback may still be given on it at that time.
    pub fn is_open_at(&self, at: OffsetDateTime) -> bool {
        self.open_until.is_none_or(|until| at < until)
    }
}

/// What the feedback feature keeps of the reading copies it heard of, so it knows each copy's project and whether it is still open.
#[async_trait]
pub trait ReadingCopyCatalog: Send + Sync {
    /// Keeps a copy as a message told it at a version of the copy, unless a newer one was kept or the copy is deleted.
    /// It answers whether it was kept.
    async fn keep(
        &self,
        transaction: &mut Transaction,
        copy: &ReadingCopyLink,
        known: &KnownReadingCopy,
        version: u64,
    ) -> Result<bool, CatalogError>;

    /// Forgets a deleted copy, and lays a tombstone so a late message does not bring it back.
    async fn forget(
        &self,
        transaction: &mut Transaction,
        copy: &ReadingCopyLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    /// Forgets the tombstones laid before a time, the oldest first and no more than asked; it answers how many.
    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError>;

    /// The copies asked for that are kept; one never heard of or deleted is left out.
    async fn known(
        &self,
        copies: &[ReadingCopyLink],
    ) -> Result<HashMap<ReadingCopyLink, KnownReadingCopy>, CatalogError>;
}

#[cfg(test)]
mod tests {
    use time::Duration;

    use super::*;

    fn open_until(until: Option<OffsetDateTime>) -> KnownReadingCopy {
        KnownReadingCopy {
            project: ProjectLink::from("project_loom"),
            open_until: until,
        }
    }

    #[test]
    fn a_copy_is_open_until_its_time_comes_and_one_without_end_always() {
        // given: a time a copy closes at
        let closes = OffsetDateTime::UNIX_EPOCH + Duration::days(30);

        // expect: it is open before then, and closed from then on
        assert!(open_until(Some(closes)).is_open_at(closes - Duration::seconds(1)));
        assert!(!open_until(Some(closes)).is_open_at(closes));
        // and: a copy open without end is open at any time
        assert!(open_until(None).is_open_at(closes + Duration::days(3650)));
    }
}
