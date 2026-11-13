use std::future::Future;

use leptos::prelude::*;

use crate::http::ApiError;
use crate::ideas::model::IdeaId;
use crate::scenes::model::{Scene, SceneId};
use crate::scenes::service as scenes;
use crate::working_copy;
use crate::working_copy::{Landed, Versioned, WorkingCopies};
use crate::writing::problems::Problems;
use crate::writing::saving::Saving;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Intent {
    Retitle { title: String },
    Link { idea: IdeaId },
    Unlink { idea: IdeaId },
    Delete,
}

/// The scenes the writing view changes, each with its own working copy.
///
/// A scene is loaded from the server the first time it is changed.
/// After that, it is changed one request at a time.
/// Each request is based on the version the scene was loaded at or last answered with.
#[derive(Clone, Copy)]
pub struct SceneCopies {
    copies: WorkingCopies<SceneId, Scene, Intent>,
    problems: Problems,
    saving: Saving,
}

impl SceneCopies {
    pub fn of(problems: Problems, saving: Saving) -> Self {
        let copies = WorkingCopies::new(
            |scene: SceneId, on: Scene, intent: Intent| async move { intent.made_on(&scene, on).await },
            |scene: SceneId| async move { scenes::open(&scene).await },
            move |failure| problems.report(failure),
        );

        Self {
            copies,
            problems,
            saving,
        }
    }

    pub fn hold(self, scene: Scene) {
        self.copies.take_in(scene.id.clone(), scene);
    }

    pub fn is_held(self, scene: &SceneId) -> bool {
        untrack(|| self.copies.is_listed(scene))
    }

    pub fn ideas_of(self, scene: &SceneId) -> Option<Vec<IdeaId>> {
        self.copies.shown_one(scene).map(|shown| shown.ideas)
    }

    pub fn retitled(self, scene: SceneId, title: String) -> impl Future<Output = bool> + 'static {
        self.changed(scene, vec![Intent::Retitle { title }])
    }

    pub fn linked(
        self,
        scene: SceneId,
        ideas: Vec<IdeaId>,
    ) -> impl Future<Output = bool> + 'static {
        let links = ideas
            .into_iter()
            .map(|idea| Intent::Link { idea })
            .collect();

        self.changed(scene, links)
    }

    pub fn unlinked(self, scene: SceneId, idea: IdeaId) -> impl Future<Output = bool> + 'static {
        self.changed(scene, vec![Intent::Unlink { idea }])
    }

    pub fn deleted(self, scene: SceneId) -> impl Future<Output = bool> + 'static {
        let deleting = self.changed(scene.clone(), vec![Intent::Delete]);

        async move {
            let gone = deleting.await;
            if gone {
                self.copies.forget(&scene);
            }

            gone
        }
    }

    fn changed(self, scene: SceneId, intents: Vec<Intent>) -> impl Future<Output = bool> + 'static {
        self.saving.write_begun();
        let at_once = self
            .is_held(&scene)
            .then(|| self.intended(&scene, intents.clone()));

        async move {
            let landings = match at_once {
                Some(landings) => landings,
                None => match scenes::open(&scene).await {
                    Ok(opened) => {
                        self.hold(opened);
                        self.intended(&scene, intents)
                    }
                    Err(failure) => {
                        self.problems.report(failure);
                        Vec::new()
                    }
                },
            };
            let mut landed = !landings.is_empty();
            for landing in landings {
                if let Err(failure) = landing.await {
                    self.problems.report(failure);
                    landed = false;
                }
            }
            self.saving.write_done();

            landed
        }
    }

    fn intended(self, scene: &SceneId, intents: Vec<Intent>) -> Vec<Landed> {
        intents
            .into_iter()
            .map(|intent| self.copies.intended(scene, intent))
            .collect()
    }
}

impl Versioned for Scene {
    fn version(&self) -> u64 {
        self.version
    }
}

impl working_copy::Intent<Scene> for Intent {
    fn lay_over(&self, scene: &mut Scene) {
        match self {
            Self::Retitle { title } => scene.title = title.clone(),
            Self::Link { idea } => {
                if !scene.ideas.contains(idea) {
                    scene.ideas.push(idea.clone());
                }
            }
            Self::Unlink { idea } => scene.ideas.retain(|linked| linked != idea),
            Self::Delete => {}
        }
    }

    fn merged_after(&self, earlier: &Self) -> Option<Self> {
        match (self, earlier) {
            (Self::Retitle { .. }, Self::Retitle { .. }) => Some(self.clone()),
            _ => None,
        }
    }
}

impl Intent {
    async fn made_on(&self, scene: &SceneId, on: Scene) -> Result<Scene, ApiError> {
        let version = on.version;

        match self {
            Self::Retitle { title } => scenes::retitle(scene, title, version).await,
            Self::Link { idea } => scenes::link(scene, idea, version).await,
            Self::Unlink { idea } => scenes::unlink(scene, idea, version).await,
            Self::Delete => {
                scenes::delete(scene, version).await?;

                Ok(on)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::working_copy::Intent as _;

    fn a_scene(ideas: &[&str]) -> Scene {
        Scene {
            id: SceneId::from("scene_1".to_owned()),
            title: "The arrival".to_owned(),
            ideas: ideas.iter().map(|named| idea(named)).collect(),
            text: String::new(),
            version: 3,
        }
    }

    fn idea(named: &str) -> IdeaId {
        IdeaId::from(named.to_owned())
    }

    #[test]
    fn a_retitle_shows_its_title_at_once() {
        // given: a scene
        let mut scene = a_scene(&[]);

        // when: a retitle is laid over it
        Intent::Retitle {
            title: "The loom".to_owned(),
        }
        .lay_over(&mut scene);

        // then: it shows the new title
        assert_eq!(scene.title, "The loom");
    }

    #[test]
    fn a_link_is_added_after_the_ideas_held_and_never_twice() {
        // given: a scene drawing on one idea
        let mut scene = a_scene(&["idea_1"]);

        // when: a new idea and the one held are linked
        Intent::Link {
            idea: idea("idea_2"),
        }
        .lay_over(&mut scene);
        Intent::Link {
            idea: idea("idea_1"),
        }
        .lay_over(&mut scene);

        // then: the new one follows, and the held one is there once
        assert_eq!(scene.ideas, vec![idea("idea_1"), idea("idea_2")]);
    }

    #[test]
    fn an_unlink_takes_only_its_own_idea_away() {
        // given: a scene drawing on two ideas
        let mut scene = a_scene(&["idea_1", "idea_2"]);

        // when: one is unlinked
        Intent::Unlink {
            idea: idea("idea_1"),
        }
        .lay_over(&mut scene);

        // then: only the other is left
        assert_eq!(scene.ideas, vec![idea("idea_2")]);
    }

    #[test]
    fn a_retitle_after_a_retitle_is_sent_once_with_the_newer_title() {
        // given: two retitles of a scene
        let earlier = Intent::Retitle {
            title: "The loom".to_owned(),
        };
        let later = Intent::Retitle {
            title: "The loom remembers".to_owned(),
        };

        // then: the later one stands for both
        assert_eq!(later.merged_after(&earlier), Some(later.clone()));
    }

    #[test]
    fn links_are_sent_in_turn_and_never_merged() {
        // given: a link and an unlink of the same idea
        let linked = Intent::Link {
            idea: idea("idea_1"),
        };
        let unlinked = Intent::Unlink {
            idea: idea("idea_1"),
        };

        // then: neither stands for the other
        assert_eq!(unlinked.merged_after(&linked), None);
        assert_eq!(linked.merged_after(&linked.clone()), None);
        // and: a retitle does not stand for a link
        assert_eq!(
            Intent::Retitle {
                title: "The loom".to_owned()
            }
            .merged_after(&linked),
            None
        );
    }
}
