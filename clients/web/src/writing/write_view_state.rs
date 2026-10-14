use leptos::task::spawn_local;

use crate::projects::model::ProjectId;
use crate::scenes::service as scenes;
use crate::statistics::service as statistics;
use crate::writing::beginnings::Beginnings;
use crate::writing::labels::Labels;
use crate::writing::left_off::LeftOff;
use crate::writing::margin::Margin;
use crate::writing::problems::Problems;
use crate::writing::saving::Saving;
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
}

impl WriteViewState {
    pub fn open(project: &ProjectId) -> Self {
        let problems = Problems::none();
        let saving = Saving::nothing();
        let structure = Structure::open(project, problems, saving);
        let walk = Walk::through(structure);
        let viewport = Viewport::around_the_window();

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
        };
        let listing = project.clone();
        let counting = project.clone();

        spawn_local(async move {
            match scenes::in_project(&listing).await {
                Ok(listed) => state.labels.remember(&listed),
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
