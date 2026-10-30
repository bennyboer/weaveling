use std::error::Error;

use eventsourcing::AccountId;
use notifications_core::{About, CatalogError, ProjectLink, Role};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum KeptAbout {
    RoleChanged {
        role: KeptRole,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        by: Option<String>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeptRole {
    Owner,
    Author,
    Reader,
}

pub fn unreachable(failure: impl Error + Send + Sync + 'static) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

pub fn unreadable(what: String) -> CatalogError {
    CatalogError::Backend(what.into())
}

pub fn project_of(about: &About) -> Option<&str> {
    match about {
        About::RoleChanged { project, .. } => Some(project.as_str()),
    }
}

pub fn kept_as(about: &About) -> KeptAbout {
    match about {
        About::RoleChanged { role, by, .. } => KeptAbout::RoleChanged {
            role: KeptRole::from(*role),
            by: by.as_ref().map(ToString::to_string),
        },
    }
}

pub fn read(project: Option<String>, kept: KeptAbout) -> Result<About, CatalogError> {
    match kept {
        KeptAbout::RoleChanged { role, by } => Ok(About::RoleChanged {
            project: ProjectLink::from(
                project
                    .ok_or_else(|| unreadable("a changed role names no project".to_owned()))?
                    .as_str(),
            ),
            role: Role::from(role),
            by: by.map(AccountId::from),
        }),
    }
}

impl From<Role> for KeptRole {
    fn from(role: Role) -> Self {
        match role {
            Role::Owner => Self::Owner,
            Role::Author => Self::Author,
            Role::Reader => Self::Reader,
        }
    }
}

impl From<KeptRole> for Role {
    fn from(kept: KeptRole) -> Self {
        match kept {
            KeptRole::Owner => Self::Owner,
            KeptRole::Author => Self::Author,
            KeptRole::Reader => Self::Reader,
        }
    }
}
