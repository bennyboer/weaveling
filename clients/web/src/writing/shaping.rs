use leptos::prelude::*;

use crate::http::ApiError;
use crate::outline::model::{Attachment, Outline, SectionId};
use crate::outline::service;
use crate::projects::model::ProjectId;
use crate::scenes::model::SceneId;
use crate::working_copy;
use crate::writing::scene_copies::SceneCopies;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Intent {
    AddSection {
        under: Option<SectionId>,
        after: Option<SectionId>,
    },
    Retitle {
        section: SectionId,
        title: String,
    },
    Nest {
        section: SectionId,
    },
    Lift {
        section: SectionId,
    },
    Attach {
        attachment: Attachment,
        to: SectionId,
        spot: Spot,
    },
    Detach {
        attachment: Attachment,
    },
    Delete {
        scene: SceneId,
    },
    Remove {
        section: SectionId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Spot {
    First,
    After(Attachment),
    Last,
}

pub(super) type Naming = Box<dyn FnOnce(SectionId)>;

impl Spot {
    pub(super) fn behind(scene: Option<SceneId>) -> Self {
        scene.map_or(Self::First, |scene| Self::After(Attachment::Scene(scene)))
    }
}

impl working_copy::Intent<Outline> for Intent {
    fn lay_over(&self, book: &mut Outline) {
        if let Self::Retitle { section, title } = self
            && let Some(held) = book.sections.iter_mut().find(|held| &held.id == section)
        {
            held.title = title.clone();
        }
    }

    fn merged_after(&self, earlier: &Self) -> Option<Self> {
        match (self, earlier) {
            (
                Self::Retitle { section, .. },
                Self::Retitle {
                    section: before, ..
                },
            ) if section == before => Some(self.clone()),
            _ => None,
        }
    }
}

impl Intent {
    pub(super) async fn made_on(
        &self,
        on: &Outline,
        project: &ProjectId,
        (added, naming): (
            StoredValue<Option<SectionId>>,
            StoredValue<Option<Naming>, LocalStorage>,
        ),
        scenes: SceneCopies,
    ) -> Result<Outline, ApiError> {
        let version = on.version;
        let told = match self {
            Self::AddSection { under, after } => {
                let (section, told) =
                    service::add(&on.id, under.clone(), after.clone(), "", version).await?;
                _ = added.try_set_value(Some(section.clone()));
                if let Some(named) = naming.try_update_value(Option::take).flatten() {
                    named(section);
                }
                told
            }
            Self::Retitle { section, title } => {
                service::retitle(&on.id, section, title, version).await?
            }
            Self::Nest { section } => service::demote(&on.id, section, version).await?,
            Self::Lift { section } => service::promote(&on.id, section, version).await?,
            Self::Attach {
                attachment,
                to,
                spot,
            } => {
                let behind = match spot {
                    Spot::First => None,
                    Spot::After(behind) => Some(behind.clone()),
                    Spot::Last => on
                        .sections
                        .iter()
                        .find(|section| &section.id == to)
                        .and_then(|section| section.attachments.last().cloned()),
                };

                service::attach(&on.id, attachment, to, behind, version).await?
            }
            Self::Detach { attachment } => {
                service::detach(&on.id, attachment, version).await?;

                return Self::read(project).await;
            }
            Self::Delete { scene } => {
                service::detach(&on.id, &Attachment::Scene(scene.clone()), version).await?;
                scenes.deleted(scene.clone()).await;

                return Self::read(project).await;
            }
            Self::Remove { section } => service::remove(&on.id, section, version).await?,
        };

        match on.fits(&told) {
            true => Ok(on.changed_by(told)),
            false => Self::read(project).await,
        }
    }

    async fn read(project: &ProjectId) -> Result<Outline, ApiError> {
        service::open(project).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outline::model::{OutlineId, Section};
    use crate::working_copy::Intent as _;

    fn section(id: &str) -> SectionId {
        SectionId::from(id.to_owned())
    }

    fn a_book() -> Outline {
        Outline {
            id: OutlineId::from("outline_1".to_owned()),
            version: 4,
            sections: vec![Section {
                id: section("one"),
                parent: None,
                title: "Part One".to_owned(),
                attachments: Vec::new(),
            }],
        }
    }

    fn retitled(id: &str, title: &str) -> Intent {
        Intent::Retitle {
            section: section(id),
            title: title.to_owned(),
        }
    }

    #[test]
    fn a_retitle_shows_its_title_at_once() {
        // given: a book
        let mut book = a_book();

        // when: a retitle of its section is laid over it
        retitled("one", "The Beginning").lay_over(&mut book);

        // then: the section shows the new title
        assert_eq!(book.sections[0].title, "The Beginning");
    }

    #[test]
    fn a_change_to_the_shape_shows_once_it_is_answered() {
        // given: a book
        let mut book = a_book();

        // when: nesting, removing and detaching are laid over it
        Intent::Nest {
            section: section("one"),
        }
        .lay_over(&mut book);
        Intent::Remove {
            section: section("one"),
        }
        .lay_over(&mut book);
        Intent::Detach {
            attachment: Attachment::Scene(SceneId::from("scene_1".to_owned())),
        }
        .lay_over(&mut book);

        // then: the book shows as it was confirmed
        assert_eq!(book, a_book());
    }

    #[test]
    fn a_retitle_after_one_of_the_same_section_is_sent_once_with_the_newer_title() {
        // given: two retitles of one section
        let earlier = retitled("one", "The Beginning");
        let later = retitled("one", "The Very Beginning");

        // then: the later one stands for both
        assert_eq!(later.merged_after(&earlier), Some(later.clone()));
    }

    #[test]
    fn retitles_of_different_sections_are_sent_each() {
        // given: retitles of two sections
        let earlier = retitled("one", "The Beginning");
        let later = retitled("two", "The Middle");

        // then: neither stands for the other
        assert_eq!(later.merged_after(&earlier), None);
    }

    #[test]
    fn what_builds_on_the_step_before_is_never_merged() {
        // given: a section nested twice
        let nested = Intent::Nest {
            section: section("one"),
        };

        // then: the second does not stand for the first
        assert_eq!(nested.merged_after(&nested.clone()), None);
        // and: a retitle does not stand for a nesting
        assert_eq!(retitled("one", "The Beginning").merged_after(&nested), None);
    }

    #[test]
    fn a_scene_brought_back_after_nothing_goes_first() {
        // given: a scene
        let scene = SceneId::from("scene_1".to_owned());

        // then: brought back after nothing it goes first, after another it goes behind it
        assert_eq!(Spot::behind(None), Spot::First);
        assert_eq!(
            Spot::behind(Some(scene.clone())),
            Spot::After(Attachment::Scene(scene))
        );
    }
}
