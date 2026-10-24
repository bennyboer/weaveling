use members_core::{CatalogError, Role};

pub fn written(role: Role) -> &'static str {
    match role {
        Role::Owner => "owner",
        Role::Author => "author",
        Role::Reader => "reader",
    }
}

pub fn read(kept: &str) -> Result<Role, CatalogError> {
    match kept {
        "owner" => Ok(Role::Owner),
        "author" => Ok(Role::Author),
        "reader" => Ok(Role::Reader),
        other => Err(CatalogError::Backend(
            format!("role holds {other:?}, which is no role").into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_role_reads_back_as_written() {
        for role in [Role::Owner, Role::Author, Role::Reader] {
            assert_eq!(read(written(role)).ok(), Some(role));
        }
    }

    #[test]
    fn a_role_nobody_declared_is_no_role() {
        assert!(read("editor").is_err());
    }
}
