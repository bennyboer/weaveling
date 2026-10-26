use invitations_core::{CatalogError, ProjectLink, Role, Scope};

const OWNER: &str = "owner";
const AUTHOR: &str = "author";
const READER: &str = "reader";

pub fn written(scope: &Scope) -> (Option<String>, Option<&'static str>) {
    match scope {
        Scope::Instance => (None, None),
        Scope::Project { project, role } => (
            Some(project.to_string()),
            Some(match role {
                Role::Owner => OWNER,
                Role::Author => AUTHOR,
                Role::Reader => READER,
            }),
        ),
    }
}

pub fn read(project: Option<String>, role: Option<&str>) -> Result<Scope, CatalogError> {
    let role = match role {
        None => None,
        Some(OWNER) => Some(Role::Owner),
        Some(AUTHOR) => Some(Role::Author),
        Some(READER) => Some(Role::Reader),
        Some(other) => {
            return Err(CatalogError::Backend(
                format!("{other:?} is no role in a project").into(),
            ));
        }
    };

    match (project, role) {
        (None, None) => Ok(Scope::Instance),
        (Some(project), Some(role)) => Ok(Scope::Project {
            project: ProjectLink::from(project),
            role,
        }),
        (project, role) => Err(CatalogError::Backend(
            format!("{project:?} as {role:?} is no way for an invitation to be scoped").into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scope_reads_back_as_it_was_written() {
        for scope in [
            Scope::Instance,
            Scope::Project {
                project: ProjectLink::from("project_1"),
                role: Role::Owner,
            },
            Scope::Project {
                project: ProjectLink::from("project_1"),
                role: Role::Author,
            },
            Scope::Project {
                project: ProjectLink::from("project_1"),
                role: Role::Reader,
            },
        ] {
            let (project, role) = written(&scope);
            assert_eq!(read(project, role).expect("readable"), scope);
        }
    }

    #[test]
    fn half_a_scope_or_a_role_nobody_knows_is_a_failure() {
        assert!(read(Some("project_1".to_owned()), None).is_err());
        assert!(read(None, Some(OWNER)).is_err());
        assert!(read(Some("project_1".to_owned()), Some("editor")).is_err());
    }
}
