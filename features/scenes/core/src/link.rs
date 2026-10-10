use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectLink(String);

impl ProjectLink {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ProjectLink {
    fn from(given: String) -> Self {
        Self(given)
    }
}

impl From<&str> for ProjectLink {
    fn from(given: &str) -> Self {
        Self(given.to_owned())
    }
}

impl Display for ProjectLink {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IdeaLink(String);

impl IdeaLink {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for IdeaLink {
    fn from(given: String) -> Self {
        Self(given)
    }
}

impl From<&str> for IdeaLink {
    fn from(given: &str) -> Self {
        Self(given.to_owned())
    }
}

impl Display for IdeaLink {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}
