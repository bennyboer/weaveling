use std::fmt::{self, Display, Formatter};

use time::OffsetDateTime;

use quotation::QuotationMarks;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProjectId(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    pub id: ProjectId,
    pub version: Version,
    pub name: String,
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectDetails {
    pub language: Option<String>,
    pub quotation: Option<QuotationMarks>,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DescribedProject {
    pub version: Version,
    pub name: String,
    pub details: ProjectDetails,
}

impl ProjectDetails {
    pub fn quotation_marks(&self) -> QuotationMarks {
        self.quotation
            .unwrap_or_else(|| QuotationMarks::of_language(self.language.as_deref()))
    }
}

impl From<String> for ProjectId {
    fn from(given: String) -> Self {
        Self(given)
    }
}

impl From<u64> for Version {
    fn from(counted: u64) -> Self {
        Self(counted)
    }
}

impl Display for Version {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Display for ProjectId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}
