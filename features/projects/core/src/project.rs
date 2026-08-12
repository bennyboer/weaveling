use time::OffsetDateTime;

use crate::{ProjectId, ProjectName};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    id: ProjectId,
    name: ProjectName,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

impl Project {
    pub fn new(name: ProjectName, now: OffsetDateTime) -> Self {
        Self {
            id: ProjectId::generate(now),
            name,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn from_parts(
        id: ProjectId,
        name: ProjectName,
        created_at: OffsetDateTime,
        updated_at: OffsetDateTime,
    ) -> Self {
        Self {
            id,
            name,
            created_at,
            updated_at,
        }
    }

    pub fn rename(&mut self, name: ProjectName, now: OffsetDateTime) {
        self.name = name;
        self.updated_at = now;
    }

    pub fn id(&self) -> ProjectId {
        self.id
    }

    pub fn name(&self) -> &ProjectName {
        &self.name
    }

    pub fn created_at(&self) -> OffsetDateTime {
        self.created_at
    }

    pub fn updated_at(&self) -> OffsetDateTime {
        self.updated_at
    }
}

#[cfg(test)]
mod tests {
    use time::Duration;

    use super::*;

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
    }

    fn name(raw: &str) -> ProjectName {
        ProjectName::new(raw).expect("test name should be valid")
    }

    #[test]
    fn new_project_is_created_and_updated_at_the_same_moment() {
        let project = Project::new(name("Tapestry"), at(1_000));

        assert_eq!(project.created_at(), at(1_000));
        assert_eq!(project.updated_at(), at(1_000));
    }

    #[test]
    fn new_projects_get_distinct_ids() {
        let one = Project::new(name("Tapestry"), at(1_000));
        let other = Project::new(name("Tapestry"), at(1_000));

        assert_ne!(one.id(), other.id());
    }

    #[test]
    fn renaming_replaces_the_name_and_bumps_updated_at() {
        let mut project = Project::new(name("Working Title"), at(1_000));

        project.rename(name("The Weaver's Apprentice"), at(2_000));

        assert_eq!(project.name().as_str(), "The Weaver's Apprentice");
        assert_eq!(project.updated_at(), at(2_000));
    }

    #[test]
    fn renaming_leaves_identity_and_creation_time_untouched() {
        let mut project = Project::new(name("Working Title"), at(1_000));
        let id = project.id();

        project.rename(name("Renamed"), at(2_000));

        assert_eq!(project.id(), id);
        assert_eq!(project.created_at(), at(1_000));
    }

    #[test]
    fn the_id_carries_the_same_moment_as_created_at() {
        let project = Project::new(name("Tapestry"), at(1_700_000_000));

        let (seconds, _) = project
            .id()
            .as_uuid()
            .get_timestamp()
            .expect("a v7 id carries a timestamp")
            .to_unix();

        assert_eq!(
            i64::try_from(seconds).expect("timestamp fits"),
            project.created_at().unix_timestamp()
        );
    }

    #[test]
    fn rehydrated_project_keeps_every_part() {
        let id = ProjectId::generate(at(500));

        let project = Project::from_parts(id, name("Tapestry"), at(1_000), at(2_000));

        assert_eq!(project.id(), id);
        assert_eq!(project.name().as_str(), "Tapestry");
        assert_eq!(project.created_at(), at(1_000));
        assert_eq!(project.updated_at(), at(2_000));
    }
}
