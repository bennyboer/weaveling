use eventsourcing::Agent;
use time::OffsetDateTime;

use crate::link::{ProjectLink, ReadingCopyLink, SectionLink};

/// What a reading copy holds of the book.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extent {
    Book,
    Sections(u32),
    Scenes(u32),
}

/// What of the work a reading copy is asked to hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shared {
    Book,
    Sections {
        first: SectionLink,
        last: SectionLink,
    },
}

/// A reading copy as its project's members find it listed: what it is, who made it, and how long it is open and kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedReadingCopy {
    pub copy: ReadingCopyLink,
    pub project: ProjectLink,
    pub name: String,
    pub made_by: Agent,
    /// The name of whoever made it, as the project's members know them.
    pub maker: String,
    pub made_at: OffsetDateTime,
    pub extent: Extent,
    /// Until when its readers may read it and give feedback; none for no end.
    pub open_until: Option<OffsetDateTime>,
    /// When it is deleted for good, with its feedback; none to keep it.
    pub deleted_on: Option<OffsetDateTime>,
    pub open: bool,
}
