use leptos::prelude::*;
use leptos::task::spawn_local;
use statistics_contract::WordCountDTO;

use crate::live::{Following, Told};
use crate::projects::model::ProjectId;
use crate::scenes::prose::SyncConnection;
use crate::scenes::service as scenes;
use crate::statistics::model::SceneWordCount;
use crate::statistics::service as statistics;
use crate::writing::beginnings::Beginnings;
use crate::writing::book_rail::BookRail;
use crate::writing::choosing::Choosing;
use crate::writing::copying::Copying;
use crate::writing::drawers::keep_apart;
use crate::writing::editors::Editors;
use crate::writing::focus::Focus;
use crate::writing::inspector::Inspector;
use crate::writing::labels::Labels;
use crate::writing::left_off::LeftOff;
use crate::writing::loose_scenes::LooseScenes;
use crate::writing::margin::Margin;
use crate::writing::moving::Moving;
use crate::writing::problems::Problems;
use crate::writing::quiet_note::QuietNote;
use crate::writing::saving::Saving;
use crate::writing::scene_ideas::SceneIdeas;
use crate::writing::settings::Settings;
use crate::writing::side::Side;
use crate::writing::structure::Structure;
use crate::writing::viewport::Viewport;
use crate::writing::walk::Walk;
use crate::writing::word_count::WordCount;
use crate::writing::writing_in::WritingIn;

#[derive(Clone, Copy)]
pub struct WriteViewState {
    pub problems: Problems,
    pub structure: Structure,
    pub walk: Walk,
    pub editors: Editors,
    pub beginnings: Beginnings,
    pub labels: Labels,
    pub margin: Margin,
    pub saving: Saving,
    pub writing_in: WritingIn,
    pub word_count: WordCount,
    pub left_off: LeftOff,
    pub viewport: Viewport,
    pub inspector: Inspector,
    pub book_rail: BookRail,
    pub scene_ideas: SceneIdeas,
    pub choosing: Choosing,
    pub copying: Copying,
    pub moving: Moving,
    pub settings: Settings,
    pub focus: Focus,
    pub side: Side,
    pub loose: LooseScenes,
    pub connection: StoredValue<SyncConnection, LocalStorage>,
    pub quiet_note: QuietNote,
}

impl WriteViewState {
    pub fn open(project: &ProjectId, side: Side) -> Self {
        let problems = Problems::none();
        let saving = Saving::nothing();
        let structure = Structure::open(project, problems, saving);
        let viewport = Viewport::around_the_window(saving);
        let loose = LooseScenes::of(structure, problems, saving);
        let editors = Editors::none();
        let walk = Walk::through(structure, viewport, side, loose, editors);
        let writing_in = WritingIn::of(structure);
        let choosing = Choosing::nothing(structure, loose, side);

        let state = Self {
            problems,
            structure,
            walk,
            editors,
            beginnings: Beginnings::of(structure, problems),
            labels: Labels::of(problems, saving),
            margin: Margin::closed(),
            saving,
            writing_in,
            word_count: WordCount::of(structure, writing_in),
            left_off: LeftOff::in_project(project, side, structure, loose, walk, viewport),
            viewport,
            inspector: Inspector::as_last_left(),
            book_rail: BookRail::as_last_left(),
            scene_ideas: SceneIdeas::of(project, problems, saving),
            choosing,
            copying: Copying::of(choosing, structure, editors, problems),
            moving: Moving::nothing(choosing, structure),
            settings: Settings::as_last_left(),
            focus: Focus::off(),
            side,
            loose,
            connection: StoredValue::new_local(SyncConnection::new()),
            quiet_note: QuietNote::none(),
        };
        keep_apart(state.book_rail.drawer(), state.inspector.drawer());
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
        let told_since = StoredValue::new(0_u64);
        let asking = {
            let project = counting.clone();

            move || {
                let project = project.clone();
                let asked_at = told_since.get_value();

                spawn_local(async move {
                    match statistics::word_counts(&project).await {
                        Ok(counted) if told_since.try_get_value() == Some(asked_at) => {
                            state.word_count.told(counted)
                        }
                        Ok(_) => {}
                        Err(failure) => state.problems.report(failure),
                    }
                });
            }
        };
        StoredValue::new_local(Following::start(
            &format!("/api/statistics/word-counts/live?project={counting}"),
            move |told: Told| {
                if let Ok(found) = serde_json::from_value::<WordCountDTO>(told.event) {
                    _ = told_since.try_update_value(|since| *since += 1);
                    state.word_count.told(vec![SceneWordCount::from(found)]);
                }
            },
            asking,
        ));

        state
    }
}
