use std::collections::HashMap;

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::http::ApiError;
use crate::outline::model::Outline;
use crate::outline::service;
use crate::projects::model::ProjectId;
use crate::scenes::model::SceneId;
use crate::scenes::prose::ProseEditor;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Towards {
    Before,
    After,
}

#[derive(Clone, Copy)]
pub struct WriteState {
    outline: RwSignal<Option<Outline>>,
    problem: RwSignal<Option<ApiError>>,
    editors: StoredValue<HashMap<SceneId, ProseEditor>, LocalStorage>,
}

impl WriteState {
    pub fn open(project: &ProjectId) -> Self {
        let state = Self {
            outline: RwSignal::new(None),
            problem: RwSignal::new(None),
            editors: StoredValue::new_local(HashMap::new()),
        };
        let project = project.clone();

        spawn_local(async move {
            match service::open(&project).await {
                Ok(opened) => state.outline.set(Some(opened)),
                Err(failure) => state.problem.set(Some(failure)),
            }
        });

        state
    }

    pub fn outline(self) -> Option<Outline> {
        self.outline.get()
    }

    pub fn problem(self) -> Option<ApiError> {
        self.problem.get()
    }

    pub fn writing_in(self, scene: SceneId, editor: ProseEditor) {
        self.editors
            .update_value(|editors| _ = editors.insert(scene, editor));
    }

    pub fn done_with(self, scene: &SceneId) -> Option<ProseEditor> {
        self.editors
            .try_update_value(|editors| editors.remove(scene))
            .flatten()
    }

    pub fn leave(self, scene: &SceneId, towards: Towards) -> bool {
        let Some(next) = self.neighbour(scene, towards) else {
            return false;
        };

        self.editors.with_value(|editors| match editors.get(&next) {
            Some(editor) => {
                match towards {
                    Towards::Before => editor.focus_end(),
                    Towards::After => editor.focus_start(),
                }

                true
            }
            None => false,
        })
    }

    fn neighbour(self, scene: &SceneId, towards: Towards) -> Option<SceneId> {
        let read = self.outline.with_untracked(|outline| {
            outline
                .as_ref()
                .map(Outline::scenes_in_reading_order)
                .unwrap_or_default()
        });
        let here = read.iter().position(|held| held == scene)?;

        match towards {
            Towards::Before => here.checked_sub(1).and_then(|back| read.get(back)),
            Towards::After => read.get(here + 1),
        }
        .cloned()
    }
}
