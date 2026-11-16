use assets_contract::{bound, refused};
use fluent_bundle::FluentArgs;
use js_sys::{Array, Uint8Array};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::{IntoView, ev, html};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{File, HtmlInputElement};

use crate::assets::service as assets;
use crate::http::{ApiError, Particular, Refusal};
use crate::problem;
use crate::scenes::model::SceneId;
use crate::speech::Speech;
use crate::writing::write_view_state::WriteViewState;

/// The image types that may be put into a scene.
const ACCEPTED: &str = "image/jpeg,image/png,image/gif,image/webp";

/// A hidden field for choosing image files, one in each scene's row.
/// It is always there, so choosing a file still finds it after the toolbar that asked for it has gone.
/// The place in the text the images go to is kept on the field while the files are chosen.
pub fn picker(scene: SceneId, state: WriteViewState) -> impl IntoView {
    let speech = expect_context::<Speech>();

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
                given(scene.clone(), state, speech, files, ticket);
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

/// Copies an image kept elsewhere, the account's own or another project's, into the scene's project,
/// and puts the copy where the ticket stands.
/// While it is copied, the text shows it in its place, and a refusal stays there until dismissed.
pub fn copied(scene: SceneId, state: WriteViewState, speech: Speech, image: String, ticket: u32) {
    let project = state.structure.project().to_string();
    let label = speech.say("image-copying");
    state.editors.told(&scene, |editor| {
        editor.upload_started(ticket, 0, &label, &label);
        editor.focus();
    });
    spawn_local(async move {
        match assets::copy_into(&image, &project).await {
            Ok(copy) => {
                state.editors.told(&scene, |editor| {
                    editor.insert_figure(&copy, ticket);
                    editor.upload_done(ticket, 0);
                });
            }
            Err(failure) => refuse(&scene, state, speech, ticket, 0, &failure),
        }
        state
            .editors
            .told(&scene, |editor| editor.forget_waiting(ticket));
    });
}

/// Takes the files the editor was given by pasting or dropping.
pub fn given_by_editor(
    scene: SceneId,
    state: WriteViewState,
    speech: Speech,
    files: Array,
    ticket: u32,
) {
    let files = files
        .iter()
        .filter_map(|file| file.dyn_into::<File>().ok())
        .collect();
    given(scene, state, speech, files, ticket);
}

/// Uploads image files into the project, one after the other, and puts each into the scene where the ticket stands.
/// While a file uploads, the text shows it in its place with how far it got.
/// Every file shows in its place at once. A file larger than an upload may be is refused right away, before anything is sent.
/// A refused file stays in its place saying why, until it is dismissed, and the others still go in.
fn given(scene: SceneId, state: WriteViewState, speech: Speech, files: Vec<File>, ticket: u32) {
    let project = state.structure.project().to_string();
    spawn_local(async move {
        let most = assets::most_each_upload().await.ok().flatten();
        let mut sending = Vec::new();
        for (nth, file) in (0u32..).zip(files) {
            let label = speech.say_with("image-uploading", &named(&file));
            state.editors.told(&scene, |editor| {
                editor.upload_started(ticket, nth, &file.name(), &label)
            });
            match most {
                Some(most) if file.size() > most as f64 => {
                    refuse(&scene, state, speech, ticket, nth, &too_large(most));
                }
                _ => sending.push((nth, file)),
            }
        }
        for (nth, file) in sending {
            match uploaded(&project, &file, scene.clone(), state, ticket, nth).await {
                Ok(asset) => {
                    state.editors.told(&scene, |editor| {
                        editor.insert_figure(&asset, ticket);
                        editor.upload_done(ticket, nth);
                    });
                }
                Err(failure) => refuse(&scene, state, speech, ticket, nth, &failure),
            }
        }
        state
            .editors
            .told(&scene, |editor| editor.forget_waiting(ticket));
    });
}

fn refuse(
    scene: &SceneId,
    state: WriteViewState,
    speech: Speech,
    ticket: u32,
    nth: u32,
    failure: &ApiError,
) {
    let told = problem::worded(failure, speech);
    state
        .editors
        .told(scene, |editor| editor.upload_refused(ticket, nth, &told));
}

fn named(file: &File) -> FluentArgs<'static> {
    let mut args = FluentArgs::new();
    args.set("name", file.name());

    args
}

/// The same refusal the server gives for an upload past its limit, told before anything is sent.
fn too_large(most: u64) -> ApiError {
    ApiError::Rejected(Refusal {
        kind: Some(refused::LIMIT_REACHED.to_owned()),
        detail: String::new(),
        particulars: vec![
            (
                "bound".to_owned(),
                Particular::Text(bound::UPLOAD.to_owned()),
            ),
            (
                "most".to_owned(),
                Particular::Count(i64::try_from(most).unwrap_or(i64::MAX)),
            ),
        ],
    })
}

async fn uploaded(
    project: &str,
    file: &File,
    scene: SceneId,
    state: WriteViewState,
    ticket: u32,
    nth: u32,
) -> Result<String, ApiError> {
    let buffer = JsFuture::from(file.array_buffer())
        .await
        .map_err(|_| ApiError::Unexpected)?;

    assets::upload_into(
        project,
        &Uint8Array::new(&buffer).to_vec(),
        move |fraction| {
            state.editors.told(&scene, |editor| {
                editor.upload_progress(ticket, nth, fraction)
            });
        },
    )
    .await
}
