use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProjectLink(String);

impl ProjectLink {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ProjectLink {
    fn from(given: &str) -> Self {
        Self(given.to_owned())
    }
}

impl From<String> for ProjectLink {
    fn from(given: String) -> Self {
        Self(given)
    }
}

impl Display for ProjectLink {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Owner,
    Author,
    Reader,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    Instance,
    Project { project: ProjectLink, role: Role },
}

impl Scope {
    pub fn project(&self) -> Option<&ProjectLink> {
        match self {
            Scope::Instance => None,
            Scope::Project { project, .. } => Some(project),
        }
    }
}
