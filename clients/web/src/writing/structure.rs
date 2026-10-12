use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::outline::model::{Outline, OutlineId, Section, SectionId};
use crate::outline::service;
use crate::projects::model::ProjectId;
use crate::scenes::model::SceneId;
use crate::writing::problems::Problems;

#[derive(Clone, Copy)]
pub struct Structure {
    project: StoredValue<ProjectId>,
    outline: RwSignal<Option<Outline>>,
    problems: Problems,
}

impl Structure {
    pub fn open(project: &ProjectId, problems: Problems) -> Self {
        let structure = Self {
            project: StoredValue::new(project.clone()),
            outline: RwSignal::new(None),
            problems,
        };
        let project = project.clone();

        spawn_local(async move {
            match service::open(&project).await {
                Ok(opened) => structure.outline.set(Some(opened)),
                Err(failure) => structure.problems.report(failure),
            }
        });

        structure
    }

    pub fn project(self) -> ProjectId {
        self.project.get_value()
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

        spawn_local(async move {
            match service::retitle(&outline, &section, &title).await {
                Ok(renamed) => self.adopt(renamed),
                Err(failure) => self.problems.report(failure),
            }
        });
    }

    pub fn adopt(self, arrived: Outline) {
        let newer = self.outline.with_untracked(|held| {
            held.as_ref()
                .is_none_or(|held| arrived.version >= held.version)
        });

        if newer {
            self.outline.set(Some(arrived));
        }
    }
}
