use fluent_bundle::FluentArgs;
use js_sys::Array;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;
use web_sys::File;

use crate::assets::service as assets;
use crate::http::{ApiError, Refusal};
use crate::kept;
use crate::problem;
use crate::scenes::model::SceneId;
use crate::scenes::prose::ProseEditor;
use crate::speech::Speech;
use crate::writing::beginnings::{Carried, CarriedImage};
use crate::writing::write_view_state::WriteViewState;

/// Where this browser keeps the assets the author copied out of a scene of their own project.
/// Only these are copied into another project when pasted there, in this tab or another.
const COPIED: &str = "weaveling.copied-assets";

/// How many copied assets are kept, the latest ones.
const COPIED_KEPT: usize = 100;

/// The refusal told when a figure pasted from elsewhere is left out.
const FROM_ELSEWHERE: &str = "image-from-elsewhere";

/// Remembers the figures the author copied or cut out of a scene, those of the scene's own project,
/// so pasting them into another project may copy them there.
pub fn figures_copied(state: WriteViewState, copied: Array) {
    let project = state.structure.project().to_string();
    let copied: Vec<String> = copied
        .iter()
        .filter_map(|asset| asset.as_string())
        .collect();
    spawn_local(async move {
        for asset in copied {
            if assets::asset(&asset)
                .await
                .ok()
                .and_then(|told| told.project)
                .as_deref()
                == Some(project.as_str())
            {
                remember_copied(asset);
            }
        }
    });
}

/// Takes in the figures pasted or dropped into a scene.
/// A figure of an asset the project holds stays as it is. One the author copied out of another project of theirs
/// is copied into this project, and the figure shows the copy once it is made.
/// Any other figure, such as one in a page from elsewhere, or one that cannot be copied, is left out of the text,
/// and the author told so, so no image of a project is carried into another unasked.
pub fn figures_pasted(scene: SceneId, state: WriteViewState, pasted: Array) {
    let project = state.structure.project().to_string();
    let pasted: Vec<String> = pasted
        .iter()
        .filter_map(|asset| asset.as_string())
        .collect();
    spawn_local(async move {
        for asset in pasted {
            if assets::asset(&asset)
                .await
                .ok()
                .and_then(|told| told.project)
                .as_deref()
                == Some(project.as_str())
            {
                continue;
            }
            let copied = match copied_here(&asset) {
                true => assets::copy_into(&asset, &project, None).await,
                false => Err(from_elsewhere()),
            };
            match copied {
                Ok(copy) => {
                    state
                        .editors
                        .told(&scene, |editor| editor.replace_asset(&asset, &copy));
                }
                Err(failure) => {
                    state
                        .editors
                        .told(&scene, |editor| editor.leave_out(&asset));
                    state.problems.report(failure);
                }
            }
        }
    });
}

fn copied_here(asset: &str) -> bool {
    kept::recall(COPIED).is_some_and(|copied| copied.split(' ').any(|kept| kept == asset))
}

fn remember_copied(asset: String) {
    let mut copied: Vec<String> = kept::recall(COPIED)
        .unwrap_or_default()
        .split(' ')
        .filter(|kept| !kept.is_empty() && *kept != asset)
        .map(str::to_owned)
        .collect();
    copied.push(asset);
    let from = copied.len().saturating_sub(COPIED_KEPT);
    kept::keep(COPIED, &copied[from..].join(" "));
}

fn from_elsewhere() -> ApiError {
    ApiError::Rejected(Refusal {
        kind: Some(FROM_ELSEWHERE.to_owned()),
        detail: String::new(),
        particulars: Vec::new(),
    })
}

/// Takes into a scene just begun what it carries over from the line it began on:
/// the words typed there, then the images given there, uploaded or chosen, after them.
/// The scene then has the focus, as it does after typing.
pub fn carried_in(
    editor: &ProseEditor,
    scene: SceneId,
    state: WriteViewState,
    speech: Speech,
    carried: Carried,
) {
    if let Some(words) = carried.words.filter(|words| !words.is_empty()) {
        editor.seed(&words);
    }
    let with_images = !carried.images.is_empty();
    for image in carried.images {
        let ticket = editor.asked_for_image();
        match image {
            CarriedImage::Files(files) => given(scene.clone(), state, speech, files, ticket),
            CarriedImage::Asset(asset) => editor.insert_chosen(&asset, ticket),
        }
    }
    if with_images {
        editor.focus();
    }
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
pub fn given(scene: SceneId, state: WriteViewState, speech: Speech, files: Vec<File>, ticket: u32) {
    let project = state.structure.project().to_string();
    for (nth, file) in (0u32..).zip(&files) {
        let label = speech.say_with("image-uploading", &named(file));
        state.editors.told(&scene, |editor| {
            editor.upload_started(ticket, nth, &file.name(), &label)
        });
    }
    spawn_local(async move {
        let progressing = scene.clone();
        assets::upload_each_into(
            &project,
            files,
            None,
            move |nth, fraction| {
                state.editors.told(&progressing, |editor| {
                    editor.upload_progress(ticket, nth as u32, fraction)
                });
            },
            |nth, told| match told {
                Ok(asset) => {
                    state.editors.told(&scene, |editor| {
                        editor.insert_figure(&asset, ticket);
                        editor.upload_done(ticket, nth as u32);
                    });
                }
                Err(failure) => refuse(&scene, state, speech, ticket, nth as u32, &failure),
            },
        )
        .await;
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
