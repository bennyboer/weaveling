use leptos::prelude::*;
use leptos::task::spawn_local;
use scenes_contract::SceneSummaryDTO;
use statistics_contract::WordCountDTO;

use crate::live::{Following, Told};
use crate::projects::model::ProjectId;
use crate::scenes::model::SceneSummary;
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

        let scenes_told_meanwhile = StoredValue::new(Vec::<SceneSummary>::new());
        let listing_scenes = move || {
            let project = listing.clone();
            scenes_told_meanwhile.set_value(Vec::new());

            spawn_local(async move {
                match scenes::in_project(&project).await {
                    Ok(listed) => {
                        state.labels.remember(&listed);
                        state.loose.remember(&listed);
                        for told in scenes_told_meanwhile.try_get_value().unwrap_or_default() {
                            state.labels.told(&told);
                            state.loose.include(vec![told.id]);
                        }
                    }
                    Err(failure) => state.problems.report(failure),
                }
            });
        };
        StoredValue::new_local(Following::start(
            &format!("/api/scenes/live?project={project}"),
            move |told: Told| {
                if let Ok(found) = serde_json::from_value::<SceneSummaryDTO>(told.event) {
                    let summary = SceneSummary::from(found);
                    _ = scenes_told_meanwhile.try_update_value(|told| told.push(summary.clone()));
                    state.labels.told(&summary);
                    state.loose.include(vec![summary.id]);
                }
            },
            listing_scenes,
        ));
        let counts_told_meanwhile = StoredValue::new(Vec::<SceneWordCount>::new());
        let asking = {
            let project = counting.clone();

            move || {
                let project = project.clone();
                counts_told_meanwhile.set_value(Vec::new());

                spawn_local(async move {
                    match statistics::word_counts(&project).await {
                        Ok(counted) => {
                            state.word_count.told(counted);
                            state
                                .word_count
                                .told(counts_told_meanwhile.try_get_value().unwrap_or_default());
                        }
                        Err(failure) => state.problems.report(failure),
                    }
                });
            }
        };
        StoredValue::new_local(Following::start(
            &format!("/api/statistics/word-counts/live?project={counting}"),
            move |told: Told| {
                if let Ok(found) = serde_json::from_value::<WordCountDTO>(told.event) {
                    let count = SceneWordCount::from(found);
                    _ = counts_told_meanwhile.try_update_value(|told| told.push(count.clone()));
                    state.word_count.told(vec![count]);
                }
            },
            asking,
        ));

        state
    }
}
