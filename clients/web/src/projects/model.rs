use std::fmt::{self, Display, Formatter};

use time::OffsetDateTime;

use quotation::QuotationMarks;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProjectId(String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    pub id: ProjectId,
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
    pub name: String,
    pub details: ProjectDetails,
}

impl From<String> for ProjectId {
    fn from(given: String) -> Self {
        Self(given)
    }
}

impl Display for ProjectId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}
