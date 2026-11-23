use std::collections::HashMap;
use std::error::Error;

use async_trait::async_trait;
use eventsourcing::{AccountId, Agent};
use thiserror::Error;
use time::OffsetDateTime;
use transacting::Transaction;

use crate::about::{About, Subject};
use crate::comment::Comment;
use crate::feedback::{Asked, Counted, Page, Pending, Status};
use crate::id::FeedbackId;
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

    /// Some of the copies closed by a time whose drafts are not dropped yet, no more than asked.
    /// A copy whose times are kept anew, as when it is extended, has its drafts dropped again once it closes again.
    async fn closed_with_drafts_left(
        &self,
        by: OffsetDateTime,
        at_most: usize,
    ) -> Result<Vec<ReadingCopyLink>, CatalogError>;

    /// Keeps that a closed copy's drafts were dropped.
    async fn drafts_dropped(
        &self,
        transaction: &mut Transaction,
        copy: &ReadingCopyLink,
    ) -> Result<(), CatalogError>;

    /// The copies asked for that are kept; one never heard of or deleted is left out.
    async fn known(
        &self,
        copies: &[ReadingCopyLink],
    ) -> Result<HashMap<ReadingCopyLink, KnownReadingCopy>, CatalogError>;
}

/// The names accounts signed up with, as the accounts told them, so feedback names whoever gave it.
/// A deleted account's name is forgotten, and its feedback names nobody.
#[async_trait]
pub trait AccountNameCatalog: Send + Sync {
    /// Keeps the name an account signed up with, unless the account is deleted.
    async fn remember_name(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        name: &str,
    ) -> Result<(), CatalogError>;

    /// Forgets a deleted account's name, and lays a tombstone so a late sign-up does not bring it back.
    async fn forget_name(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    /// Forgets the tombstones laid before a time, the oldest first and no more than asked; it answers how many.
    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError>;

    /// The names of those accounts whose name is kept.
    async fn names_of(
        &self,
        accounts: &[AccountId],
    ) -> Result<HashMap<AccountId, String>, CatalogError>;
}

/// A piece of feedback as the catalog keeps it, to be found and counted:
/// its project and what it is about, what it says, who wrote it and when, and how far it was worked through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CataloguedFeedback {
    pub id: FeedbackId,
    pub project: ProjectLink,
    pub about: About,
    pub comment: Comment,
    pub written_by: Agent,
    pub written_at: OffsetDateTime,
    /// When it was submitted; until then it is a draft, which only whoever wrote it sees.
    pub submitted_at: Option<OffsetDateTime>,
    /// Whether what it says was changed after it was submitted.
    pub edited: bool,
    /// Whether the authors marked it done.
    pub done: bool,
}

impl CataloguedFeedback {
    pub fn is_draft(&self) -> bool {
        self.submitted_at.is_none()
    }
}

/// The feedback as it now stands, kept for finding and counting it.
/// Each piece is kept in the transaction its events are stored in, so what was just written is found at once.
/// Pieces submitted at the same time are listed in the order they were written, and those written at the same time by their ids.
#[async_trait]
pub trait FeedbackCatalog: Send + Sync {
    /// Keeps a piece as it now stands, in place of what was kept of it before.
    async fn keep(
        &self,
        transaction: &mut Transaction,
        piece: &CataloguedFeedback,
    ) -> Result<(), CatalogError>;

    /// Forgets a deleted piece.
    async fn forget(
        &self,
        transaction: &mut Transaction,
        piece: &FeedbackId,
    ) -> Result<(), CatalogError>;

    /// One piece, drafted or submitted, if it is kept.
    async fn piece(&self, piece: &FeedbackId) -> Result<Option<CataloguedFeedback>, CatalogError>;

    /// A writer's own drafts where asked, oldest first.
    async fn drafts_of(
        &self,
        asked: &Asked,
        writer: &Agent,
    ) -> Result<Vec<CataloguedFeedback>, CatalogError>;

    /// The feedback submitted where asked, drafts left out, if asked only what is pending or done,
    /// in the order the page asks for, after the piece it names, and no more than it asks for.
    async fn submitted(
        &self,
        asked: &Asked,
        status: Option<Status>,
        page: &Page,
    ) -> Result<Vec<CataloguedFeedback>, CatalogError>;

    /// How much feedback submitted where asked is pending, not done yet: on each scene and section, and in all.
    async fn pending(&self, asked: &Asked) -> Result<Pending, CatalogError>;

    /// Some of the drafts on a subject, by anyone, no more than asked, to be dropped as the subject closes.
    async fn drafts_on(
        &self,
        subject: &Subject,
        at_most: usize,
    ) -> Result<Vec<FeedbackId>, CatalogError>;

    /// Some of the pieces given or drafted on a subject, no more than asked, to be swept as the subject goes.
    async fn pieces_on(
        &self,
        subject: &Subject,
        at_most: usize,
    ) -> Result<Vec<FeedbackId>, CatalogError>;

    /// Whoever submitted feedback in a project, in the order they first submitted any.
    async fn givers_in(&self, project: &ProjectLink) -> Result<Vec<Agent>, CatalogError>;

    /// How much feedback each subject holds, in the order asked: what is submitted and not done, and the asker's own drafts.
    async fn counted_on(
        &self,
        subjects: &[Subject],
        asker: &Agent,
    ) -> Result<Vec<Counted>, CatalogError>;
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
