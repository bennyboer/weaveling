use std::collections::VecDeque;
use std::pin::Pin;

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::ideas::model::IdeaId;
use crate::live::Following;
use crate::outline::model::{Attachment, Outline, OutlineChange, OutlineId, Section, SectionId};
use crate::outline::service;
use crate::projects::model::ProjectId;
use crate::scenes::model::SceneId;
use crate::scenes::service as scenes;
use crate::writing::cursor_kept::keeping_the_cursor;
use crate::writing::problems::Problems;
use crate::writing::rearranged::{placements, rearranged};
use crate::writing::saving::Saving;

type Write = Pin<Box<dyn Future<Output = ()>>>;

enum Shift {
    In,
    Out,
}

#[derive(Clone, Copy)]
pub struct Structure {
    project: StoredValue<ProjectId>,
    outline: RwSignal<Option<Outline>>,
    writes: StoredValue<VecDeque<Write>, LocalStorage>,
    writing: StoredValue<bool>,
    following: StoredValue<Option<Following>, LocalStorage>,
    reload_in_flight: StoredValue<bool>,
    rearranged_by_others: RwSignal<Vec<SectionId>>,
    outdated_meanwhile: StoredValue<bool>,
    problems: Problems,
    saving: Saving,
}

impl Structure {
    pub fn open(project: &ProjectId, problems: Problems, saving: Saving) -> Self {
        let structure = Self {
            project: StoredValue::new(project.clone()),
            outline: RwSignal::new(None),
            writes: StoredValue::new_local(VecDeque::new()),
            writing: StoredValue::new(false),
            following: StoredValue::new_local(None),
            reload_in_flight: StoredValue::new(false),
            rearranged_by_others: RwSignal::new(Vec::new()),
            outdated_meanwhile: StoredValue::new(false),
            problems,
            saving,
        };
        let project = project.clone();
        on_cleanup(move || _ = structure.following.try_update_value(|held| *held = None));

        spawn_local(async move {
            match service::open(&project).await {
                Ok(opened) => {
                    let outline = opened.id.clone();
                    structure.outline.set(Some(opened));
                    structure.follow(&outline);
                }
                Err(failure) => structure.problems.report(failure),
            }
        });

        structure
    }

    fn follow(self, outline: &OutlineId) {
        let following = Following::start(
            &format!("/api/outlines/live?outline={outline}"),
            move |version| self.heard_of(version),
            move || self.reload(),
        );
        _ = self
            .following
            .try_update_value(|held| *held = Some(following));
    }

    fn heard_of(self, version: u64) {
        let behind = self
            .outline
            .try_with_untracked(|held| held.as_ref().is_some_and(|held| held.version < version))
            .unwrap_or(false);

        if behind {
            self.reload();
        }
    }

    fn reload(self) {
        if self.reload_in_flight.try_get_value().unwrap_or(true) {
            _ = self.outdated_meanwhile.try_set_value(true);
            return;
        }
        _ = self.reload_in_flight.try_set_value(true);

        spawn_local(async move {
            loop {
                _ = self.outdated_meanwhile.try_set_value(false);
                let Some(project) = self.project.try_get_value() else {
                    return;
                };
                let before = self.read_untracked(placements);
                let mine_in_flight = self.writing.try_get_value().unwrap_or(true);
                match service::open(&project).await {
                    Ok(opened) if !self.outline.is_disposed() => {
                        self.adopt(opened);
                        let mine_in_flight =
                            mine_in_flight || self.writing.try_get_value().unwrap_or(true);
                        if let (Some(before), Some(after), false) =
                            (before, self.read_untracked(placements), mine_in_flight)
                        {
                            let moved = rearranged(&before, &after);
                            if !moved.is_empty() {
                                self.rearranged_by_others.set(moved);
                            }
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

    pub fn project(self) -> ProjectId {
        self.project.get_value()
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

    pub fn retitle(self, section: SectionId, title: String) {
        let Some(outline) = self.id() else {
            return;
        };

        self.in_turn(async move {
            match service::retitle(&outline, &section, &title).await {
                Ok(renamed) => self.take(renamed),
                Err(failure) => self.problems.report(failure),
            }
        });
    }

    pub fn holds(self, scene: &SceneId) -> bool {
        self.read_untracked(|book| {
            book.sections
                .iter()
                .any(|section| section.scenes().contains(scene))
        })
        .unwrap_or(false)
    }

    pub fn bring_back(self, scene: SceneId, to: SectionId, after: Option<SceneId>) {
        self.bring_back_then(scene, to, after, || {});
    }

    pub fn bring_back_then(
        self,
        scene: SceneId,
        to: SectionId,
        after: Option<SceneId>,
        then: impl FnOnce() + 'static,
    ) {
        let Some(outline) = self.id() else {
            return;
        };

        self.in_turn(async move {
            let behind = after.map(Attachment::Scene);

            match service::attach(&outline, &Attachment::Scene(scene), &to, behind).await {
                Ok(placed) => {
                    self.take(placed);
                    then();
                }
                Err(failure) => self.problems.report(failure),
            }
        });
    }

    pub fn can_nest(self, section: &SectionId) -> bool {
        self.read_untracked(|book| book.can_demote(section))
            .unwrap_or(false)
    }

    pub fn can_lift(self, section: &SectionId) -> bool {
        self.read_untracked(|book| book.can_promote(section))
            .unwrap_or(false)
    }

    pub fn nest(self, section: SectionId) {
        self.shift(section, Shift::In);
    }

    pub fn lift(self, section: SectionId) {
        self.shift(section, Shift::Out);
    }

    fn shift(self, section: SectionId, towards: Shift) {
        let Some(outline) = self.id() else {
            return;
        };

        self.in_turn(async move {
            let moved = match towards {
                Shift::In => service::demote(&outline, &section).await,
                Shift::Out => service::promote(&outline, &section).await,
            };

            match moved {
                Ok(moved) => self.take(moved),
                Err(failure) => self.problems.report(failure),
            }
        });
    }

    pub fn is_pinned(self, idea: &IdeaId) -> bool {
        self.outline.with(|outline| {
            outline
                .as_ref()
                .is_some_and(|book| book.holds(&Attachment::Idea(idea.clone())))
        })
    }

    pub fn pin(self, section: SectionId, ideas: Vec<IdeaId>) {
        let Some(outline) = self.id() else {
            return;
        };
        let mut behind = self
            .read_untracked(|book| {
                book.sections
                    .iter()
                    .find(|held| held.id == section)
                    .and_then(|held| held.attachments.last().cloned())
            })
            .flatten();

        self.in_turn(async move {
            for idea in ideas {
                let pinned = Attachment::Idea(idea);

                match service::attach(&outline, &pinned, &section, behind.take()).await {
                    Ok(placed) => {
                        self.take(placed);
                        behind = Some(pinned);
                    }
                    Err(failure) => return self.problems.report(failure),
                }
            }
        });
    }

    pub fn unpin(self, idea: IdeaId) {
        let Some(outline) = self.id() else {
            return;
        };

        self.in_turn(async move {
            match service::detach(&outline, &Attachment::Idea(idea)).await {
                Ok(()) => self.refresh().await,
                Err(failure) => self.problems.report(failure),
            }
        });
    }

    pub fn take_out(self, scene: SceneId) {
        let Some(outline) = self.id() else {
            return;
        };

        self.in_turn(async move {
            match service::detach(&outline, &Attachment::Scene(scene)).await {
                Ok(()) => self.refresh().await,
                Err(failure) => self.problems.report(failure),
            }
        });
    }

    pub fn delete(self, scene: SceneId) {
        let Some(outline) = self.id() else {
            return;
        };

        self.in_turn(async move {
            if let Err(failure) = service::detach(&outline, &Attachment::Scene(scene.clone())).await
            {
                return self.problems.report(failure);
            }
            if let Err(failure) = scenes::delete(&scene).await {
                self.problems.report(failure);
            }
            self.refresh().await;
        });
    }

    pub fn remove(self, section: SectionId) {
        let Some(outline) = self.id() else {
            return;
        };

        self.in_turn(async move {
            match service::remove(&outline, &section).await {
                Ok(removed) => self.take(removed),
                Err(failure) => self.problems.report(failure),
            }
        });
    }

    pub fn in_turn(self, write: impl Future<Output = ()> + 'static) {
        self.saving.write_begun();
        self.writes
            .update_value(|writes| writes.push_back(Box::pin(write)));

        if self.writing.get_value() {
            return;
        }
        self.writing.set_value(true);

        spawn_local(async move {
            while let Some(next) = self.writes.try_update_value(VecDeque::pop_front).flatten() {
                next.await;
                self.saving.write_done();
            }
            _ = self.writing.try_update_value(|writing| *writing = false);
        });
    }

    async fn refresh(self) {
        match service::open(&self.project()).await {
            Ok(opened) => self.adopt(opened),
            Err(failure) => self.problems.report(failure),
        }
    }

    pub fn take(self, change: OutlineChange) {
        self.outline.update(|held| {
            if let Some(held) = held
                && change.version >= held.version
            {
                *held = held.changed_by(change);
            }
        });
    }

    pub fn adopt(self, arrived: Outline) {
        let newer = self.outline.with_untracked(|held| {
            held.as_ref()
                .is_none_or(|held| arrived.version >= held.version)
        });

        if newer {
            keeping_the_cursor(|| self.outline.set(Some(arrived)));
        }
    }
}
