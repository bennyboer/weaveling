use std::fmt::{self, Display, Formatter};

use time::OffsetDateTime;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProjectId(String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub updated_at: OffsetDateTime,
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
