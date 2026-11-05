use std::future::Future;

use leptos::prelude::*;
use leptos::task;

use crate::http::ApiError;
use crate::ideas::model::IdeaId;
use crate::live::Subscription;
use crate::outline::model::{Attachment, Outline, OutlineId, Section, SectionId};
use crate::outline::service;
use crate::projects::model::ProjectId;
use crate::scenes::model::SceneId;
use crate::working_copy;
use crate::working_copy::{Landed, Versioned, WorkingCopy};
use crate::writing::cursor_kept;
use crate::writing::problems::Problems;
use crate::writing::rearranged;
use crate::writing::rearranged::Removed;
use crate::writing::saving::Saving;
use crate::writing::scene_copies::SceneCopies;
use live_contract::FrameDTO;
use outline_contract::OutlineEventDTO;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Intent {
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
enum Spot {
    First,
    After(Attachment),
    Last,
}

type Naming = Box<dyn FnOnce(SectionId)>;

#[derive(Clone, Copy)]
pub struct Structure {
    project: StoredValue<ProjectId>,
    working: WorkingCopy<Outline, Intent>,
    outline: Memo<Option<Outline>>,
    unanswered: Memo<bool>,
    added: StoredValue<Option<SectionId>>,
    naming: StoredValue<Option<Naming>, LocalStorage>,
    following: StoredValue<Option<Subscription>, LocalStorage>,
    reload_in_flight: StoredValue<bool>,
    outdated_meanwhile: StoredValue<bool>,
    told_while_unanswered: StoredValue<bool>,
    rearranged_by_others: RwSignal<Vec<SectionId>>,
    removed_by_others: RwSignal<Vec<Removed>>,
    overtaken: RwSignal<u64>,
    problems: Problems,
}

impl Structure {
    pub fn open(
        project: &ProjectId,
        problems: Problems,
        saving: Saving,
        scenes: SceneCopies,
    ) -> Self {
        let added = StoredValue::new(None::<SectionId>);
        let naming = StoredValue::new_local(None::<Naming>);
        let overtaken = RwSignal::new(0);
        let working = {
            let (sending_to, reading) = (project.clone(), project.clone());

            WorkingCopy::new(
                move |on: Outline, intent: Intent| {
                    let project = sending_to.clone();

                    async move { intent.made_on(&on, &project, (added, naming), scenes).await }
                },
                move || {
                    let project = reading.clone();

                    async move { service::open(&project).await }
                },
                move |failure| match failure {
                    ApiError::Overtaken => _ = overtaken.try_update(|times| *times += 1),
                    failure => problems.report(failure),
                },
            )
        };
        let unanswered = Memo::new(move |_| working.unanswered());
        saving.while_unanswered(unanswered.into());

        let structure = Self {
            project: StoredValue::new(project.clone()),
            working,
            outline: Memo::new(move |_| working.shown()),
            unanswered,
            added,
            naming,
            following: StoredValue::new_local(None),
            reload_in_flight: StoredValue::new(false),
            outdated_meanwhile: StoredValue::new(false),
            told_while_unanswered: StoredValue::new(false),
            rearranged_by_others: RwSignal::new(Vec::new()),
            removed_by_others: RwSignal::new(Vec::new()),
            overtaken,
            problems,
        };

        Effect::new(move |was_unanswered: Option<bool>| {
            let now = unanswered.get();
            let told_meanwhile = !now
                && was_unanswered == Some(true)
                && structure
                    .told_while_unanswered
                    .try_update_value(std::mem::take)
                    .unwrap_or(false);
            if told_meanwhile {
                structure.reload();
            }

            now
        });

        let project = project.clone();
        task::spawn_local(async move {
            match service::open(&project).await {
                Ok(opened) => {
                    let outline = opened.id.clone();
                    structure.take_in(opened);
                    structure.follow(&outline);
                }
                Err(failure) => structure.problems.report(failure),
            }
        });

        structure
    }

    fn follow(self, outline: &OutlineId) {
        let following = Subscription::start(
            "outlines",
            &[("outline", outline.to_string().as_str())],
            move |told| self.heard_of(told),
            || {},
        );
        _ = self
            .following
            .try_update_value(|held| *held = Some(following));
    }

    fn heard_of(self, told: FrameDTO) {
        if self.removed_by_others.is_disposed() {
            return;
        }
        let Some(held) = self.working.confirmed_untracked() else {
            return;
        };
        if told.version <= held.version() {
            return;
        }
        if self.is_unanswered() {
            _ = self.told_while_unanswered.try_set_value(true);
            return;
        }
        let before = held;
        let next = (told.version == before.version + 1)
            .then(|| serde_json::from_value::<OutlineEventDTO>(told.event).ok())
            .flatten()
            .and_then(|event| before.clone().taken_in(event));
        let Some(next) = next else {
            return self.reload();
        };
        self.take_in(next.clone());
        self.notice(&before, &next);
    }

    fn notice(self, before: &Outline, after: &Outline) {
        let moved = rearranged::rearranged(
            &rearranged::placements(before),
            &rearranged::placements(after),
        );
        if !moved.is_empty() {
            self.rearranged_by_others.set(moved);
        }
        let gone = rearranged::removed(before, after);
        if !gone.is_empty() {
            self.removed_by_others.set(gone);
        }
    }

    fn reload(self) {
        if self.reload_in_flight.try_get_value().unwrap_or(true) {
            _ = self.outdated_meanwhile.try_set_value(true);
            return;
        }
        _ = self.reload_in_flight.try_set_value(true);

        task::spawn_local(async move {
            loop {
                _ = self.outdated_meanwhile.try_set_value(false);
                let Some(project) = self.project.try_get_value() else {
                    return;
                };
                let mine_on_their_way = self.is_unanswered();
                let before = self.working.confirmed_untracked();
                match service::open(&project).await {
                    Ok(opened) if !self.removed_by_others.is_disposed() => {
                        let mine_on_their_way = mine_on_their_way || self.is_unanswered();
                        self.take_in(opened.clone());
                        if let (Some(before), false) = (before, mine_on_their_way)
                            && before.version < opened.version
                        {
                            self.notice(&before, &opened);
                        }
                    }
                    Ok(_) => return,
                    Err(failure) => self.problems.report(failure),
                }
                if !self.outdated_meanwhile.try_get_value().unwrap_or(false) {
                    break;
                }
            }
            _ = self.reload_in_flight.try_set_value(false);
        });
    }

    fn is_unanswered(self) -> bool {
        self.unanswered.try_get_untracked().unwrap_or(true)
    }

    fn take_in(self, outline: Outline) {
        cursor_kept::keeping_the_cursor(|| self.working.take_in(outline));
    }

    /// Opens the book again and takes it in, as when a change of the author's was refused and what became of its place has to be known.
    pub async fn caught_up(self) {
        let Some(project) = self.project.try_get_value() else {
            return;
        };
        match service::open(&project).await {
            Ok(opened) if !self.removed_by_others.is_disposed() => self.take_in(opened),
            Ok(_) => {}
            Err(failure) => self.problems.report(failure),
        }
    }

    pub fn project(self) -> ProjectId {
        self.project.get_value()
    }

    pub fn overtaken(self) -> u64 {
        self.overtaken.get()
    }

    pub fn removed_by_others(self) -> Vec<Removed> {
        self.removed_by_others.get()
    }

    pub fn rearranged_by_others(self) -> Vec<SectionId> {
        self.rearranged_by_others.get()
    }

    pub fn is_open(self) -> bool {
        self.outline.with(Option::is_some)
    }

    pub fn is_unshaped(self) -> bool {
        self.outline.with(|outline| {
            outline
                .as_ref()
                .is_some_and(|book| book.sections.is_empty())
        })
    }

    pub fn sections(self) -> Vec<(usize, Section)> {
        self.outline.with(|outline| {
            outline
                .as_ref()
                .map(|book| {
                    book.in_reading_order()
                        .into_iter()
                        .map(|(depth, section)| (depth, section.clone()))
                        .collect()
                })
                .unwrap_or_default()
        })
    }

    pub fn scenes_of(self, section: &SectionId) -> Vec<SceneId> {
        self.outline.with(|outline| {
            outline
                .as_ref()
                .and_then(|book| book.sections.iter().find(|held| &held.id == section))
                .map(Section::scenes)
                .unwrap_or_default()
        })
    }

    pub fn ideas_of(self, section: &SectionId) -> Vec<IdeaId> {
        self.outline.with(|outline| {
            outline
                .as_ref()
                .and_then(|book| book.sections.iter().find(|held| &held.id == section))
                .map(Section::ideas)
                .unwrap_or_default()
        })
    }

    pub fn section_holding(self, scene: &SceneId) -> Option<SectionId> {
        self.read_untracked(|book| {
            book.sections
                .iter()
                .find(|section| section.scenes().contains(scene))
                .map(|section| section.id.clone())
        })
        .flatten()
    }

    pub fn has_children(self, section: &SectionId) -> bool {
        self.read_untracked(|book| {
            book.sections
                .iter()
                .any(|held| held.parent.as_ref() == Some(section))
        })
        .unwrap_or(false)
    }

    pub fn top_level(self) -> Vec<SectionId> {
        self.read_untracked(|book| {
            book.in_reading_order()
                .into_iter()
                .filter(|(depth, _)| *depth == 0)
                .map(|(_, section)| section.id.clone())
                .collect()
        })
        .unwrap_or_default()
    }

    pub fn first_section(self) -> Option<SectionId> {
        self.outline.with(|outline| {
            outline
                .as_ref()?
                .in_reading_order()
                .first()
                .map(|(_, section)| section.id.clone())
        })
    }

    pub fn title_of(self, section: &SectionId) -> Option<String> {
        self.outline.with(|outline| {
            outline
                .as_ref()?
                .sections
                .iter()
                .find(|held| &held.id == section)
                .map(|held| held.title.clone())
        })
    }

    pub fn sections_within(self, section: &SectionId) -> Vec<SectionId> {
        self.read_untracked(|book| {
            let read = book.in_reading_order();
            let Some(start) = read.iter().position(|(_, held)| &held.id == section) else {
                return Vec::new();
            };
            let (depth, _) = read[start];

            read[start..]
                .iter()
                .enumerate()
                .take_while(|(nth, (nested, _))| *nth == 0 || *nested > depth)
                .map(|(_, (_, held))| held.id.clone())
                .collect()
        })
        .unwrap_or_default()
    }

    pub fn scenes_within(self, section: &SectionId) -> Vec<SceneId> {
        self.outline.with(|outline| {
            let Some(book) = outline.as_ref() else {
                return Vec::new();
            };
            let read = book.in_reading_order();
            let Some(start) = read.iter().position(|(_, held)| &held.id == section) else {
                return Vec::new();
            };
            let (depth, _) = read[start];

            read[start..]
                .iter()
                .enumerate()
                .take_while(|(nth, (nested, _))| *nth == 0 || *nested > depth)
                .flat_map(|(_, (_, held))| held.scenes())
                .collect()
        })
    }

    pub fn every_scene(self) -> Vec<SceneId> {
        self.outline.with(|outline| {
            outline
                .as_ref()
                .map(|book| {
                    book.in_reading_order()
                        .into_iter()
                        .flat_map(|(_, section)| section.scenes())
                        .collect()
                })
                .unwrap_or_default()
        })
    }

    pub fn last_section(self) -> Option<SectionId> {
        self.outline.with(|outline| {
            outline
                .as_ref()?
                .in_reading_order()
                .last()
                .map(|(_, section)| section.id.clone())
        })
    }

    pub fn ends_in_a_scene(self) -> bool {
        self.last_section()
            .is_some_and(|last| !self.scenes_of(&last).is_empty())
    }

    pub fn parent_of(self, section: &SectionId) -> Option<SectionId> {
        self.read_untracked(|book| book.parent_of(section))
            .flatten()
    }

    pub fn depth_in_book(self, section: &SectionId) -> usize {
        self.outline.with(|outline| {
            outline
                .as_ref()
                .and_then(|book| {
                    book.in_reading_order()
                        .into_iter()
                        .find(|(_, held)| &held.id == section)
                        .map(|(depth, _)| depth)
                })
                .unwrap_or(0)
        })
    }

    pub fn depth_of(self, section: &SectionId) -> usize {
        self.read_untracked(|book| {
            book.in_reading_order()
                .into_iter()
                .find(|(_, held)| &held.id == section)
                .map(|(depth, _)| depth)
        })
        .flatten()
        .unwrap_or(0)
    }

    pub fn end_of_block(self, section: &SectionId) -> Option<SectionId> {
        self.outline.with(|outline| {
            let book = outline.as_ref()?;
            let read = book.in_reading_order();
            let start = read.iter().position(|(_, held)| &held.id == section)?;
            let (depth, _) = read[start];

            read[start..]
                .iter()
                .take_while(|(nested, held)| &held.id == section || *nested > depth)
                .last()
                .map(|(_, held)| held.id.clone())
        })
    }

    pub fn read<T>(self, reading: impl FnOnce(&Outline) -> T) -> Option<T> {
        self.outline.with(|outline| outline.as_ref().map(reading))
    }

    pub fn read_untracked<T>(self, reading: impl FnOnce(&Outline) -> T) -> Option<T> {
        self.outline
            .with_untracked(|outline| outline.as_ref().map(reading))
    }

    pub fn id(self) -> Option<OutlineId> {
        self.read_untracked(|book| book.id.clone())
    }

    pub fn holds(self, scene: &SceneId) -> bool {
        self.read_untracked(|book| {
            book.sections
                .iter()
                .any(|section| section.scenes().contains(scene))
        })
        .unwrap_or(false)
    }

    pub fn can_nest(self, section: &SectionId) -> bool {
        self.read_untracked(|book| book.can_demote(section))
            .unwrap_or(false)
    }

    pub fn can_lift(self, section: &SectionId) -> bool {
        self.read_untracked(|book| book.can_promote(section))
            .unwrap_or(false)
    }

    pub fn is_pinned(self, idea: &IdeaId) -> bool {
        self.outline.with(|outline| {
            outline
                .as_ref()
                .is_some_and(|book| book.holds(&Attachment::Idea(idea.clone())))
        })
    }

    pub fn retitle(self, section: SectionId, title: String) {
        self.working.intend(Intent::Retitle { section, title });
    }

    /// Adds an untitled section, answering with it once the server made it, or with nothing when it was refused. `named` hears it as soon as the server named it, before it is shown.
    pub fn added(
        self,
        under: Option<SectionId>,
        after: Option<SectionId>,
        named: impl FnOnce(SectionId) + 'static,
    ) -> impl Future<Output = Option<SectionId>> + 'static {
        _ = self.naming.try_set_value(Some(Box::new(named)));
        let landed = self.working.intended(Intent::AddSection { under, after });

        async move {
            landed
                .await
                .then(|| self.added.try_update_value(Option::take).flatten())
                .flatten()
        }
    }

    pub fn bring_back(self, scene: SceneId, to: SectionId, after: Option<SceneId>) {
        self.working.intend(Intent::Attach {
            attachment: Attachment::Scene(scene),
            to,
            spot: Spot::behind(after),
        });
    }

    pub fn brought_back(self, scene: SceneId, to: SectionId, after: Option<SceneId>) -> Landed {
        self.working.intended(Intent::Attach {
            attachment: Attachment::Scene(scene),
            to,
            spot: Spot::behind(after),
        })
    }

    pub fn bring_back_then(
        self,
        scene: SceneId,
        to: SectionId,
        after: Option<SceneId>,
        then: impl FnOnce() + 'static,
    ) {
        let landed = self.brought_back(scene, to, after);

        task::spawn_local(async move {
            if landed.await {
                then();
            }
        });
    }

    pub fn nest(self, section: SectionId) {
        self.working.intend(Intent::Nest { section });
    }

    pub fn lift(self, section: SectionId) {
        self.working.intend(Intent::Lift { section });
    }

    pub fn pin(self, section: SectionId, ideas: Vec<IdeaId>) {
        for idea in ideas {
            self.working.intend(Intent::Attach {
                attachment: Attachment::Idea(idea),
                to: section.clone(),
                spot: Spot::Last,
            });
        }
    }

    pub fn unpin(self, idea: IdeaId) {
        self.working.intend(Intent::Detach {
            attachment: Attachment::Idea(idea),
        });
    }

    pub fn take_out(self, scene: SceneId) {
        self.working.intend(Intent::Detach {
            attachment: Attachment::Scene(scene),
        });
    }

    pub fn delete(self, scene: SceneId) {
        self.working.intend(Intent::Delete { scene });
    }

    pub fn remove(self, section: SectionId) {
        self.working.intend(Intent::Remove { section });
    }
}

impl Spot {
    fn behind(scene: Option<SceneId>) -> Self {
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
    async fn made_on(
        &self,
        on: &Outline,
        project: &ProjectId,
        (added, naming): (
            StoredValue<Option<SectionId>>,
            StoredValue<Option<Naming>, LocalStorage>,
        ),
        scenes: SceneCopies,
    ) -> Result<Outline, ApiError> {
        let version = Some(on.version);
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
