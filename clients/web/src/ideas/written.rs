use std::cell::RefCell;
use std::collections::HashMap;

use crate::ideas::model::{Idea, IdeaId};
use crate::projects::model::ProjectId;

thread_local! {
    static WRITTEN: RefCell<HashMap<IdeaId, Idea>> = RefCell::new(HashMap::new());
}

pub fn remember(idea: &Idea) {
    WRITTEN.with_borrow_mut(|written| match written.get(&idea.id) {
        Some(known) if known.version >= idea.version => {}
        _ => {
            written.insert(idea.id.clone(), idea.clone());
        }
    });
}

pub fn caught_up(project: &ProjectId, mut listed: Vec<Idea>) -> Vec<Idea> {
    WRITTEN.with_borrow_mut(|written| {
        written.retain(|id, mine| {
            if &mine.project != project {
                return true;
            }

            match listed.iter_mut().find(|known| &known.id == id) {
                Some(known) if known.version >= mine.version => false,
                Some(known) => {
                    *known = mine.clone();
                    true
                }
                None => {
                    listed.push(mine.clone());
                    true
                }
            }
        });
    });
    listed.sort_by(|one, other| other.id.cmp(&one.id));

    listed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn an_idea(id: &str, project: &str, version: u64, title: &str) -> Idea {
        Idea {
            id: IdeaId::from(id.to_owned()),
            version,
            project: ProjectId::from(project.to_owned()),
            title: title.to_owned(),
        }
    }

    fn titles(listed: &[Idea]) -> Vec<&str> {
        listed.iter().map(|idea| idea.title.as_str()).collect()
    }

    #[test]
    fn an_idea_the_catalog_has_not_heard_of_yet_is_listed_anyway() {
        remember(&an_idea("idea_2", "project_a", 1, "Fresh"));

        let listed = caught_up(
            &ProjectId::from("project_a".to_owned()),
            vec![an_idea("idea_1", "project_a", 1, "Old")],
        );

        assert_eq!(
            titles(&listed),
            vec!["Fresh", "Old"],
            "a view opened right after capturing must not miss what was just captured"
        );
    }

    #[test]
    fn a_rename_the_catalog_has_not_caught_up_with_wins() {
        remember(&an_idea("idea_1", "project_b", 2, "Renamed"));

        let listed = caught_up(
            &ProjectId::from("project_b".to_owned()),
            vec![an_idea("idea_1", "project_b", 1, "Before")],
        );

        assert_eq!(titles(&listed), vec!["Renamed"]);
    }

    #[test]
    fn once_the_catalog_has_caught_up_it_is_trusted_again() {
        remember(&an_idea("idea_1", "project_c", 2, "Renamed"));
        caught_up(
            &ProjectId::from("project_c".to_owned()),
            vec![an_idea("idea_1", "project_c", 2, "Renamed")],
        );

        let later = caught_up(
            &ProjectId::from("project_c".to_owned()),
            vec![an_idea("idea_1", "project_c", 3, "Renamed elsewhere")],
        );

        assert_eq!(
            titles(&later),
            vec!["Renamed elsewhere"],
            "a memory of our own write must not outlive the catalog catching up"
        );
    }

    #[test]
    fn an_older_write_never_replaces_a_newer_one() {
        remember(&an_idea("idea_1", "project_d", 3, "Newest"));
        remember(&an_idea("idea_1", "project_d", 2, "Older"));

        let listed = caught_up(&ProjectId::from("project_d".to_owned()), Vec::new());

        assert_eq!(titles(&listed), vec!["Newest"]);
    }

    #[test]
    fn ideas_of_another_project_stay_out() {
        remember(&an_idea("idea_9", "project_elsewhere", 1, "Elsewhere"));

        let listed = caught_up(&ProjectId::from("project_e".to_owned()), Vec::new());

        assert!(listed.is_empty());
    }
}
