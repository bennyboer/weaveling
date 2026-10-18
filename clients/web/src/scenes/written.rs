use std::cell::RefCell;

use crate::projects::model::ProjectId;
use crate::scenes::model::{SceneId, SceneSummary};

thread_local! {
    static MADE: RefCell<Vec<(ProjectId, SceneSummary)>> = const { RefCell::new(Vec::new()) };
}

pub fn remember(project: &ProjectId, made: SceneSummary) {
    MADE.with_borrow_mut(|known| known.push((project.clone(), made)));
}

pub fn forget(scene: &SceneId) {
    MADE.with_borrow_mut(|known| known.retain(|(_, made)| &made.id != scene));
}

pub fn caught_up(project: &ProjectId, mut listed: Vec<SceneSummary>) -> Vec<SceneSummary> {
    MADE.with_borrow_mut(|known| {
        known.retain(|(of, made)| {
            if of != project {
                return true;
            }
            if listed.iter().any(|held| held.id == made.id) {
                return false;
            }
            listed.push(made.clone());

            true
        });
    });

    listed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_project(id: &str) -> ProjectId {
        ProjectId::from(id.to_owned())
    }

    fn a_scene(id: &str) -> SceneSummary {
        SceneSummary {
            id: SceneId::from(id.to_owned()),
            title: String::new(),
            opening: String::new(),
        }
    }

    fn ids(listed: &[SceneSummary]) -> Vec<String> {
        listed.iter().map(|scene| scene.id.to_string()).collect()
    }

    #[test]
    fn a_scene_made_here_is_listed_before_the_catalog_has_heard_of_it() {
        remember(&a_project("project_made"), a_scene("scene_new"));

        let listed = caught_up(&a_project("project_made"), vec![a_scene("scene_old")]);

        assert_eq!(ids(&listed), ["scene_old", "scene_new"]);
    }

    #[test]
    fn once_the_catalog_lists_it_the_scene_is_not_listed_twice_or_kept() {
        remember(&a_project("project_caught"), a_scene("scene_heard"));

        let first = caught_up(&a_project("project_caught"), vec![a_scene("scene_heard")]);
        let later = caught_up(&a_project("project_caught"), vec![]);

        assert_eq!(ids(&first), ["scene_heard"]);
        assert!(
            later.is_empty(),
            "what the catalog once listed is the catalog's to list from then on"
        );
    }

    #[test]
    fn a_scene_made_in_another_project_stays_out() {
        remember(&a_project("project_one"), a_scene("scene_elsewhere"));

        let listed = caught_up(&a_project("project_two"), vec![]);

        assert!(listed.is_empty());
    }

    #[test]
    fn a_deleted_scene_is_not_brought_back() {
        remember(&a_project("project_gone"), a_scene("scene_gone"));

        forget(&SceneId::from("scene_gone".to_owned()));

        assert!(caught_up(&a_project("project_gone"), vec![]).is_empty());
    }
}
