use leptos::prelude::*;
use leptos::{IntoView, ev, html};
use leptos_router::hooks::use_navigate;

use crate::dialog::modal;
use crate::outline::model::Section;
use crate::route;
use crate::scenes::model::SceneId;
use crate::speech::{said, say, say_about};
use crate::writing::write_view_state::WriteViewState;

pub fn placing(state: WriteViewState, project: String) -> impl IntoView {
    let project = StoredValue::new(project);

    move || {
        state
            .margin
            .placing()
            .map(|scene| section_chooser(scene, project, state))
    }
}

fn section_chooser(
    scene: SceneId,
    project: StoredValue<String>,
    state: WriteViewState,
) -> impl IntoView {
    let named = untrack(|| state.labels.of_scene(&scene)).shown;
    let told = match named.trim() {
        "" => say("placing-told-unnamed"),
        named => say_about("placing-told", "scene", named),
    };

    modal(
        Signal::derive(said("placing")),
        "placing",
        move || state.margin.return_to_writing(),
        (
            html::p().child(told),
            html::ul()
                .class("sections")
                .attr("aria-label", said("placing-sections"))
                .child(
                    untrack(|| state.structure.sections())
                        .into_iter()
                        .map(|(depth, section)| {
                            section_choice(depth, section, scene.clone(), project, state)
                        })
                        .collect::<Vec<_>>(),
                ),
            html::div().class("dialog-actions").child(
                html::button()
                    .on(ev::click, move |_| state.margin.return_to_writing())
                    .child(said("cancel")),
            ),
        ),
    )
}

fn section_choice(
    depth: usize,
    section: Section,
    scene: SceneId,
    project: StoredValue<String>,
    state: WriteViewState,
) -> impl IntoView {
    let navigate = use_navigate();
    let titled = match section.title.trim() {
        "" => say("heading-untitled"),
        title => title.to_owned(),
    };
    let to = section.id.clone();

    html::li().style(("--depth", depth.to_string())).child(
        html::button()
            .r#type("button")
            .on(ev::click, move |_| {
                let after = untrack(|| state.structure.scenes_of(&to)).last().cloned();
                let at = project.with_value(|project| route::writing_at(project, &scene));
                let navigate = navigate.clone();

                state.margin.return_to_writing();
                state
                    .structure
                    .bring_back_then(scene.clone(), to.clone(), after, move || {
                        navigate(&at, Default::default())
                    });
            })
            .child(titled),
    )
}
