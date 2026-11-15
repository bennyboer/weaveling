use js_sys::{Array, Uint8Array};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::{IntoView, ev, html};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{File, HtmlInputElement};

use crate::assets::service as assets;
use crate::http::ApiError;
use crate::scenes::model::SceneId;
use crate::writing::write_view_state::WriteViewState;

/// The image types that may be put into a scene.
const ACCEPTED: &str = "image/jpeg,image/png,image/gif,image/webp";

/// A hidden field for choosing image files, one in each scene's row.
/// It is always there, so choosing a file still finds it after the toolbar that asked for it has gone.
/// The place in the text the images go to is kept on the field while the files are chosen.
pub fn picker(scene: SceneId, state: WriteViewState) -> impl IntoView {
    html::input()
        .r#type("file")
        .class("image-picker visually-hidden")
        .attr("accept", ACCEPTED)
        .attr("multiple", true)
        .attr("tabindex", "-1")
        .attr("aria-hidden", "true")
        .on(ev::change, move |event| {
            let Some(input) = event
                .target()
                .and_then(|target| target.dyn_into::<HtmlInputElement>().ok())
            else {
                return;
            };
            let ticket = input
                .get_attribute("data-ticket")
                .and_then(|ticket| ticket.parse::<u32>().ok());
            let files: Vec<File> = input
                .files()
                .map(|files| {
                    (0..files.length())
                        .filter_map(|nth| files.get(nth))
                        .collect()
                })
                .unwrap_or_default();
            input.set_value("");
            if let Some(ticket) = ticket {
                given(scene.clone(), state, files, ticket);
            }
        })
}

/// Opens the scene's field for choosing images, to put them at the place the ticket stands for.
pub fn pick(scene: &SceneId, ticket: u32) {
    if let Some(input) = document()
        .query_selector(&format!("[data-scene-row=\"{scene}\"] .image-picker"))
        .ok()
        .flatten()
        .and_then(|found| found.dyn_into::<HtmlInputElement>().ok())
    {
        let _ = input.set_attribute("data-ticket", &ticket.to_string());
        input.click();
    }
}

/// Takes the files the editor was given by pasting or dropping.
pub fn given_by_editor(scene: SceneId, state: WriteViewState, files: Array, ticket: u32) {
    let files = files
        .iter()
        .filter_map(|file| file.dyn_into::<File>().ok())
        .collect();
    given(scene, state, files, ticket);
}

/// Uploads image files into the project, one after the other, and puts each into the scene where the ticket stands.
/// A file the server refuses is told as a problem, and the others still go in.
fn given(scene: SceneId, state: WriteViewState, files: Vec<File>, ticket: u32) {
    let project = state.structure.project().to_string();
    spawn_local(async move {
        for file in files {
            match uploaded(&project, &file).await {
                Ok(asset) => {
                    state
                        .editors
                        .told(&scene, |editor| editor.insert_figure(&asset, ticket));
                }
                Err(failure) => state.problems.report(failure),
            }
        }
        state
            .editors
            .told(&scene, |editor| editor.forget_waiting(ticket));
    });
}

async fn uploaded(project: &str, file: &File) -> Result<String, ApiError> {
    let buffer = JsFuture::from(file.array_buffer())
        .await
        .map_err(|_| ApiError::Unexpected)?;

    assets::upload_into(project, &Uint8Array::new(&buffer).to_vec()).await
}
