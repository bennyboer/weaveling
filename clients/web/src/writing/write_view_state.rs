use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::projects::model::ProjectId;
use crate::scenes::prose::SyncConnection;
use crate::scenes::service as scenes;
use crate::statistics::service as statistics;
use crate::writing::beginnings::Beginnings;
use crate::writing::labels::Labels;
use crate::writing::left_off::LeftOff;
use crate::writing::loose_scenes::LooseScenes;
use crate::writing::margin::Margin;
use crate::writing::problems::Problems;
use crate::writing::saving::Saving;
use crate::writing::scene_ideas::SceneIdeas;
use crate::writing::scene_tray::SceneTray;
use crate::writing::side::Side;
use crate::writing::structure::Structure;
use crate::writing::viewport::Viewport;
use crate::writing::walk::Walk;
use crate::writing::word_count::WordCount;

#[derive(Clone, Copy)]
pub struct WriteViewState {
    pub problems: Problems,
    pub structure: Structure,
    pub walk: Walk,
    pub beginnings: Beginnings,
    pub labels: Labels,
    pub margin: Margin,
    pub saving: Saving,
    pub word_count: WordCount,
    pub left_off: LeftOff,
    pub viewport: Viewport,
    pub scene_tray: SceneTray,
    pub scene_ideas: SceneIdeas,
    pub side: Side,
    pub loose: LooseScenes,
    pub connection: StoredValue<SyncConnection, LocalStorage>,
}

impl WriteViewState {
    pub fn open(project: &ProjectId, side: Side) -> Self {
        let problems = Problems::none();
        let saving = Saving::nothing();
        let structure = Structure::open(project, problems, saving);
        let viewport = Viewport::around_the_window(saving);
        let loose = LooseScenes::of(structure, problems, saving);
        let walk = Walk::through(structure, viewport, side, loose);

        let state = Self {
            problems,
            structure,
            walk,
            beginnings: Beginnings::of(structure, problems),
            labels: Labels::of(problems, saving),
            margin: Margin::closed(),
            saving,
            word_count: WordCount::of(structure),
            left_off: LeftOff::in_project(project, structure, walk, viewport),
            viewport,
            scene_tray: SceneTray::as_last_left(),
            scene_ideas: SceneIdeas::of(project, problems, saving),
            side,
            loose,
            connection: StoredValue::new_local(SyncConnection::new()),
        };
        let listing = project.clone();
        let counting = project.clone();

        spawn_local(async move {
            match scenes::in_project(&listing).await {
                Ok(listed) => {
                    state.labels.remember(&listed);
                    state.loose.remember(&listed);
                }
                Err(failure) => state.problems.report(failure),
            }
        });
        spawn_local(async move {
            match statistics::word_counts(&counting).await {
                Ok(counted) => state.word_count.remember(counted),
                Err(failure) => state.problems.report(failure),
            }
        });

        state
    }
}
