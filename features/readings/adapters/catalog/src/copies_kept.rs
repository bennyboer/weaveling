use eventsourcing::{AccountId, Agent};
use readings_core::{
    CatalogError, Extent, ProjectLink, ReadingCopyId, ReadingCopyName, ReadingCopySummary, Times,
};
use time::OffsetDateTime;

const ACCOUNT: &str = "account";
const LOCAL: &str = "local";
const SYSTEM: &str = "system";

const BOOK: &str = "book";
const SECTIONS: &str = "sections";
const SCENES: &str = "scenes";

/// A copy as a row of the databases holds it, each moment already read by the database's own means.
pub(crate) struct KeptReadingCopySummary {
    pub copy: String,
    pub project: String,
    pub name: String,
    pub made_by_kind: String,
    pub made_by: Option<String>,
    pub made_at: OffsetDateTime,
    pub extent: String,
    pub extent_count: Option<i64>,
    pub open_until: Option<OffsetDateTime>,
    pub deleted_on: Option<OffsetDateTime>,
}

/// Who made a copy, as the databases keep it: the kind of agent, and the account's id if it is one.
pub(crate) fn maker_written(agent: &Agent) -> (&'static str, Option<&str>) {
    match agent {
        Agent::Account(account) => (ACCOUNT, Some(account.as_str())),
        Agent::Local => (LOCAL, None),
        Agent::System => (SYSTEM, None),
    }
}

/// What a copy holds, as the databases keep it: the kind, and how many sections or scenes.
pub(crate) fn extent_written(extent: Extent) -> (&'static str, Option<i64>) {
    match extent {
        Extent::Book => (BOOK, None),
        Extent::Sections(count) => (SECTIONS, Some(i64::from(count))),
        Extent::Scenes(count) => (SCENES, Some(i64::from(count))),
    }
}

impl KeptReadingCopySummary {
    pub(crate) fn summary(self) -> Result<ReadingCopySummary, CatalogError> {
        Ok(ReadingCopySummary {
            copy: self
                .copy
                .parse::<ReadingCopyId>()
                .map_err(|why| unreadable(why.to_string()))?,
            project: ProjectLink::from(self.project),
            name: ReadingCopyName::new(&self.name)
                .map_err(|_| unreadable(format!("{:?} is no copy's name", self.name)))?,
            made_by: maker_read(&self.made_by_kind, self.made_by)?,
            made_at: self.made_at,
            extent: extent_read(&self.extent, self.extent_count)?,
            times: Times {
                open_until: self.open_until,
                deleted_on: self.deleted_on,
            },
        })
    }
}

fn maker_read(kind: &str, account: Option<String>) -> Result<Agent, CatalogError> {
    match (kind, account) {
        (ACCOUNT, Some(account)) => Ok(Agent::Account(AccountId::from(account))),
        (LOCAL, None) => Ok(Agent::Local),
        (SYSTEM, None) => Ok(Agent::System),
        (kind, account) => Err(unreadable(format!("{kind} {account:?} is no copy's maker"))),
    }
}

fn extent_read(kind: &str, count: Option<i64>) -> Result<Extent, CatalogError> {
    let counted = |count: i64| {
        u32::try_from(count).map_err(|_| unreadable(format!("{count} is no count of a copy")))
    };

    match (kind, count) {
        (BOOK, None) => Ok(Extent::Book),
        (SECTIONS, Some(count)) => Ok(Extent::Sections(counted(count)?)),
        (SCENES, Some(count)) => Ok(Extent::Scenes(counted(count)?)),
        (kind, count) => Err(unreadable(format!(
            "{kind} {count:?} is not what a copy holds"
        ))),
    }
}

fn unreadable(why: String) -> CatalogError {
    CatalogError::Backend(why.into())
}
