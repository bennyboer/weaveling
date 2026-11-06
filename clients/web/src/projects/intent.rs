use crate::http::ApiError;
use crate::projects::model::{DescribedProject, Project, ProjectDetails, TrashedProject};
use crate::working_copy;

const SUBJECT: &str = "project";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Intent {
    Rename {
        name: String,
    },
    ChangeDetails {
        details: ProjectDetails,
        seen: ProjectDetails,
    },
    Trash,
    Restore,
}

impl working_copy::Intent<Project> for Intent {
    fn lay_over(&self, on: &mut Project) {
        match self {
            Self::Rename { name } => on.name = name.clone(),
            Self::Trash => on.trashed = true,
            Self::ChangeDetails { .. } | Self::Restore => {}
        }
    }

    fn merged_after(&self, earlier: &Self) -> Option<Self> {
        self.outright_after(earlier)
    }
}

impl working_copy::Intent<DescribedProject> for Intent {
    fn lay_over(&self, on: &mut DescribedProject) {
        match self {
            Self::Rename { name } => on.name = name.clone(),
            Self::ChangeDetails { details, .. } => on.details = details.clone(),
            Self::Trash | Self::Restore => {}
        }
    }

    fn merged_after(&self, earlier: &Self) -> Option<Self> {
        self.outright_after(earlier)
    }

    fn still_fits(&self, on: &DescribedProject) -> Result<(), ApiError> {
        match self {
            Self::ChangeDetails { seen, .. } if *seen != on.details => {
                Err(ApiError::Outdated(SUBJECT))
            }
            _ => Ok(()),
        }
    }
}

impl working_copy::Intent<TrashedProject> for Intent {
    fn lay_over(&self, on: &mut TrashedProject) {
        if let Self::Restore = self {
            on.restored = true;
        }
    }

    fn merged_after(&self, earlier: &Self) -> Option<Self> {
        self.outright_after(earlier)
    }
}

impl Intent {
    fn outright_after(&self, earlier: &Self) -> Option<Self> {
        match (self, earlier) {
            (Self::Rename { .. }, Self::Rename { .. }) => Some(self.clone()),
            (Self::ChangeDetails { details, .. }, Self::ChangeDetails { seen, .. }) => {
                Some(Self::ChangeDetails {
                    details: details.clone(),
                    seen: seen.clone(),
                })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::*;
    use crate::projects::model::{ProjectId, Version};
    use crate::working_copy::Intent as _;

    fn a_project() -> Project {
        Project {
            id: ProjectId::from("project_1".to_owned()),
            version: Version::from(3),
            name: "The Loom".to_owned(),
            updated_at: datetime!(2026-11-05 10:00 UTC),
            trashed: false,
        }
    }

    fn an_open_project() -> DescribedProject {
        DescribedProject {
            version: Version::from(3),
            name: "The Loom".to_owned(),
            details: some_details("A weaver's book"),
        }
    }

    fn a_trashed_project() -> TrashedProject {
        TrashedProject {
            id: ProjectId::from("project_1".to_owned()),
            version: Version::from(4),
            name: "The Loom".to_owned(),
            until: datetime!(2026-12-05 10:00 UTC),
            restored: false,
        }
    }

    fn some_details(description: &str) -> ProjectDetails {
        ProjectDetails {
            language: Some("en".to_owned()),
            quotation: None,
            description: description.to_owned(),
        }
    }

    fn renaming(name: &str) -> Intent {
        Intent::Rename {
            name: name.to_owned(),
        }
    }

    #[test]
    fn a_rename_shows_the_new_name_in_the_list_and_in_the_open_project() {
        // given: a project listed and open
        let mut listed = a_project();
        let mut open = an_open_project();

        // when: a rename is laid over both
        renaming("The Weft").lay_over(&mut listed);
        renaming("The Weft").lay_over(&mut open);

        // then: both show the new name at the version they had
        assert_eq!(listed.name, "The Weft");
        assert_eq!(open.name, "The Weft");
        assert_eq!(listed.version, Version::from(3));
        // and: the open project keeps its details
        assert_eq!(open.details, an_open_project().details);
    }

    #[test]
    fn a_trashing_marks_the_listed_project_as_gone_and_a_restoring_the_trashed_one() {
        // given: a project listed and one in the trash
        let mut listed = a_project();
        let mut trashed = a_trashed_project();

        // when: a trashing is laid over the one and a restoring over the other
        Intent::Trash.lay_over(&mut listed);
        Intent::Restore.lay_over(&mut trashed);

        // then: the listed one is going to the trash and the trashed one coming out
        assert!(listed.trashed);
        assert!(trashed.restored);
    }

    #[test]
    fn what_an_intent_is_not_about_leaves_a_project_as_it_was() {
        // given: changes of details, of name, a trashing and a restoring
        let unrelated = [
            (
                Intent::ChangeDetails {
                    details: some_details("Elsewhere"),
                    seen: some_details("A weaver's book"),
                },
                "the list shows no details",
            ),
            (Intent::Restore, "a listed project is in no trash"),
        ];

        // then: none of them changes the listed project
        for (intent, why) in unrelated {
            let mut listed = a_project();
            intent.lay_over(&mut listed);
            assert_eq!(listed, a_project(), "{why}");
        }
        // and: a rename or a trashing changes no trashed project
        for intent in [renaming("The Weft"), Intent::Trash] {
            let mut trashed = a_trashed_project();
            intent.lay_over(&mut trashed);
            assert_eq!(trashed, a_trashed_project());
        }
    }

    #[test]
    fn new_details_show_on_the_open_project_and_keep_its_name() {
        // given: an open project
        let mut open = an_open_project();

        // when: new details are laid over it
        Intent::ChangeDetails {
            details: some_details("Two cities"),
            seen: some_details("A weaver's book"),
        }
        .lay_over(&mut open);

        // then: it shows them under its name
        assert_eq!(open.details, some_details("Two cities"));
        assert_eq!(open.name, "The Loom");
    }

    #[test]
    fn renames_close_together_go_out_as_the_last() {
        // when: a rename follows another waiting
        let merged = renaming("The Weft").outright_after(&renaming("The Warp"));

        // then: only the later one goes
        assert_eq!(merged, Some(renaming("The Weft")));
    }

    #[test]
    fn details_changed_twice_go_out_as_the_last_against_what_was_seen_first() {
        // given: details changed from what was seen, and then again
        let first = Intent::ChangeDetails {
            details: some_details("Two cities"),
            seen: some_details("A weaver's book"),
        };
        let second = Intent::ChangeDetails {
            details: some_details("Three cities"),
            seen: some_details("Two cities"),
        };

        // when: they are merged
        let merged = second.outright_after(&first);

        // then: the later details go, made on what was seen before the first
        assert_eq!(
            merged,
            Some(Intent::ChangeDetails {
                details: some_details("Three cities"),
                seen: some_details("A weaver's book"),
            }),
            "the server holds what was seen before either, so that is what the change was made on"
        );
    }

    #[test]
    fn a_trashing_or_restoring_is_merged_with_nothing() {
        // expect: neither merges with what comes before, nor with each other
        for (later, earlier) in [
            (Intent::Trash, renaming("The Weft")),
            (renaming("The Weft"), Intent::Trash),
            (Intent::Trash, Intent::Trash),
            (Intent::Restore, Intent::Restore),
        ] {
            assert_eq!(later.outright_after(&earlier), None);
        }
    }

    #[test]
    fn details_changed_elsewhere_since_they_were_seen_are_not_overwritten() {
        // given: details changed from what was seen
        let changing = Intent::ChangeDetails {
            details: some_details("Written here"),
            seen: some_details("A weaver's book"),
        };

        // then: an open project still holding what was seen takes them
        assert_eq!(changing.still_fits(&an_open_project()), Ok(()));
        // and: one whose details changed elsewhere meanwhile does not
        let elsewhere = DescribedProject {
            details: some_details("Written in another tab"),
            ..an_open_project()
        };
        assert_eq!(
            changing.still_fits(&elsewhere),
            Err(ApiError::Outdated("project"))
        );
        // and: a rename meanwhile stands in the way of nothing
        let renamed = DescribedProject {
            name: "The Weft".to_owned(),
            ..an_open_project()
        };
        assert_eq!(changing.still_fits(&renamed), Ok(()));
        // and: a rename never was changed meanwhile
        assert_eq!(renaming("The Weft").still_fits(&elsewhere), Ok(()));
    }
}
