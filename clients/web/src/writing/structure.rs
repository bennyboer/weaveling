use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::outline::model::{Attachment, Outline, OutlineId, Section, SectionId};
use crate::outline::service;
use crate::projects::model::ProjectId;
use crate::scenes::model::SceneId;
use crate::scenes::service as scenes;
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

    pub fn section_holding(self, scene: &SceneId) -> Option<SectionId> {
        self.read_untracked(|book| {
            book.sections
                .iter()
                .find(|section| section.scenes().contains(scene))
                .map(|section| section.id.clone())
        })
        .flatten()
    }

    pub fn parent_of(self, section: &SectionId) -> Option<SectionId> {
        self.read_untracked(|book| book.parent_of(section))
            .flatten()
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

    pub fn holds(self, scene: &SceneId) -> bool {
        self.read_untracked(|book| {
            book.sections
                .iter()
                .any(|section| section.scenes().contains(scene))
        })
        .unwrap_or(false)
    }

    pub fn bring_back(self, scene: SceneId, to: SectionId, after: Option<SceneId>) {
        let Some(outline) = self.id() else {
            return;
        };

        spawn_local(async move {
            let behind = after.map(Attachment::Scene);

            match service::attach(&outline, &Attachment::Scene(scene), &to, behind).await {
                Ok(placed) => self.adopt(placed),
                Err(failure) => self.problems.report(failure),
            }
        });
    }

    pub fn take_out(self, scene: SceneId) {
        let Some(outline) = self.id() else {
            return;
        };

        spawn_local(async move {
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

        spawn_local(async move {
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

        spawn_local(async move {
            match service::remove(&outline, &section).await {
                Ok(removed) => self.adopt(removed),
                Err(failure) => self.problems.report(failure),
            }
        });
    }

    async fn refresh(self) {
        match service::open(&self.project()).await {
            Ok(opened) => self.adopt(opened),
            Err(failure) => self.problems.report(failure),
        }
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
