use std::future::Future;

use leptos::prelude::*;
use leptos::task;

use crate::http::ApiError;
use crate::ideas::model::IdeaId;
use crate::outline::model::{Attachment, Outline, OutlineId, Section, SectionId};
use crate::outline::service;
use crate::projects::model::ProjectId;
use crate::scenes::model::SceneId;
use crate::working_copy::{Landed, WorkingCopy};
use crate::writing::cursor_kept;
use crate::writing::problems::Problems;
use crate::writing::rearranged;
use crate::writing::rearranged::Removed;
use crate::writing::saving::Saving;
use crate::writing::scene_copies::SceneCopies;
use crate::writing::shaping::{Intent, Naming, Spot};
use outline_contract::OutlineEventDTO;

#[derive(Clone, Copy)]
pub struct Structure {
    project: StoredValue<ProjectId>,
    working: WorkingCopy<Outline, Intent>,
    outline: Memo<Option<Outline>>,
    unanswered: Memo<bool>,
    added: StoredValue<Option<SectionId>>,
    naming: StoredValue<Option<Naming>, LocalStorage>,
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
                move |failure| {
                    crate::writing::structure::told_refused(failure, overtaken, problems)
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
            rearranged_by_others: RwSignal::new(Vec::new()),
            removed_by_others: RwSignal::new(Vec::new()),
            overtaken,
            problems,
        };

        working.noticing_others(move |before, after, taking_in| {
            cursor_kept::keeping_the_cursor(taking_in);
            if !structure.is_unanswered() {
                structure.notice(before, after);
            }
        });
        working.read_again();
        working.following(
            "outlines",
            |outline: &Outline| ("outline", outline.id.to_string()),
            |held: Outline, event: OutlineEventDTO| held.taken_in(event),
        );

        structure
    }

    fn notice(self, before: &Outline, after: &Outline) {
        let moved = rearranged::rearranged(
            &rearranged::placements(before),
            &rearranged::placements(after),
        );
        if !moved.is_empty() {
            _ = self.rearranged_by_others.try_set(moved);
        }
        let gone = rearranged::removed(before, after);
        if !gone.is_empty() {
            _ = self.removed_by_others.try_set(gone);
        }
    }

    fn is_unanswered(self) -> bool {
        self.unanswered.try_get_untracked().unwrap_or(true)
    }

    /// Reports the refusal of a change the author waited on, the same way as one nobody waited on.
    /// If another change got in first, a quiet note is shown; otherwise it is shown as a problem.
    pub fn refused(self, failure: ApiError) {
        crate::writing::structure::told_refused(failure, self.overtaken, self.problems);
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

    /// Adds an untitled section.
    /// Returns the section once the server created it, or the reason it was refused.
    /// `named` is called with the section's id as soon as the server assigned it, before the section is shown.
    pub fn added(
        self,
        under: Option<SectionId>,
        after: Option<SectionId>,
        named: impl FnOnce(SectionId) + 'static,
    ) -> impl Future<Output = Result<SectionId, ApiError>> + 'static {
        _ = self.naming.try_set_value(Some(Box::new(named)));
        let landed = self.working.intended(Intent::AddSection { under, after });

        async move {
            landed.await?;
            self.added
                .try_update_value(Option::take)
                .flatten()
                .ok_or(ApiError::Unexpected)
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
            match landed.await {
                Ok(()) => then(),
                Err(failure) => self.refused(failure),
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

fn told_refused(failure: ApiError, overtaken: RwSignal<u64>, problems: Problems) {
    match failure {
        ApiError::Overtaken => _ = overtaken.try_update(|times| *times += 1),
        failure => problems.report(failure),
    }
}
