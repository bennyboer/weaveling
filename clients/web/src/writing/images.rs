use fluent_bundle::FluentArgs;
use js_sys::Array;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;
use web_sys::File;

use crate::assets::service as assets;
use crate::http::ApiError;
use crate::problem;
use crate::scenes::model::SceneId;
use crate::speech::Speech;
use crate::writing::write_view_state::WriteViewState;

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
pub fn given(scene: SceneId, state: WriteViewState, speech: Speech, files: Vec<File>, ticket: u32) {
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
                    refuse(&scene, state, speech, ticket, nth, &assets::too_large(most));
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

async fn uploaded(
    project: &str,
    file: &File,
    scene: SceneId,
    state: WriteViewState,
    ticket: u32,
    nth: u32,
) -> Result<String, ApiError> {
    assets::upload_into(
        project,
        file,
        move |fraction| {
            state.editors.told(&scene, |editor| {
                editor.upload_progress(ticket, nth, fraction)
            });
        },
        None,
    )
    .await
}
