use assets_contract::refused;
use fluent_bundle::FluentArgs;
use leptos::html::Input;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::{IntoView, ev, html};
use time::OffsetDateTime;
use wasm_bindgen::JsCast;
use web_sys::{AbortController, AbortSignal, File, HtmlInputElement};

use crate::accounts::{CurrentSession, SignInState};
use crate::assets::service;
use crate::assets::service::{AssetType, UploadsSorted, Use};
use crate::assets::uses;
use crate::blocks;
use crate::dialog;
use crate::http::ApiError;
use crate::icons;
use crate::icons::Icon;
use crate::members::roles::Roles;
use crate::moment;
use crate::problem;
use crate::scenes::model::SceneId;
use crate::speech::Speech;

/// The width the assets are fetched at to be offered as small tiles.
const OFFERED_WIDE: u32 = 384;

/// The image types that may be uploaded from the dialog.
const ACCEPTED: &str = "image/jpeg,image/png,image/gif,image/webp";

/// Whose assets the dialog offers: the project's, or everything the signed-in account uploaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Whose {
    Project,
    Mine,
}

/// Where an offered asset is kept, as far as this project is concerned.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Kept {
    Here,
    Own,
    Elsewhere(Option<String>),
}

/// An asset offered in the grid: its id, when it was uploaded, where it is kept,
/// and the project that holds it, if one does.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Offered {
    id: String,
    uploaded_at: OffsetDateTime,
    kept: Kept,
    project: Option<String>,
}

/// An asset that could not be deleted because it is in use, and what uses it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct InUse {
    id: String,
    project: Option<String>,
    uses: Vec<Use>,
}

/// A file uploaded from the dialog: its name, how far it got, and why it was refused, if it was.
#[derive(Clone, Copy)]
struct Sending {
    key: u32,
    name: StoredValue<String>,
    done: RwSignal<f64>,
    refused: RwSignal<Option<ApiError>>,
}

/// Aborts every upload of the dialog when the dialog goes, so nothing arrives unseen.
struct Halting(Option<AbortController>);

impl Halting {
    fn signal(&self) -> Option<AbortSignal> {
        self.0.as_ref().map(AbortController::signal)
    }
}

impl Drop for Halting {
    fn drop(&mut self) {
        if let Some(controller) = &self.0 {
            controller.abort();
        }
    }
}

/// The dialog for choosing an asset of a project, or uploading new ones into it.
/// It offers the project's assets of the given type, or with an account the account's own uploads too,
/// the newest first. Uploads show in the grid with how far they got, and once uploaded can be chosen.
/// An asset kept elsewhere, the account's own or another project's, is copied into the project when chosen,
/// its tile saying so meanwhile, or saying why it could not be.
/// An asset may be deleted from its tile where the account may delete it, after asking.
/// One still in use is not deleted, and the dialog names where it is used, to be removed there first.
/// `chosen` is told the id of the project's asset that was chosen, `closed` that the dialog was left without choosing,
/// and `followed` that a scene of this project using an asset was followed from the dialog, with that asset.
pub fn dialog(
    project: String,
    of_type: AssetType,
    chosen: impl Fn(String) + Copy + Send + Sync + 'static,
    closed: impl Fn() + Copy + Send + Sync + 'static,
    followed: impl Fn(SceneId, String) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let speech = expect_context::<Speech>();
    let signed_in = matches!(
        expect_context::<CurrentSession>().state(),
        SignInState::SignedIn(_)
    );
    let project = StoredValue::new(project);
    let whose = RwSignal::new(Whose::Project);
    let offered = RwSignal::new(Vec::<Offered>::new());
    let next = RwSignal::new(None::<String>);
    let loading = RwSignal::new(true);
    let failed = RwSignal::new(None::<ApiError>);
    let sending = RwSignal::new(Vec::<Sending>::new());
    let copying = RwSignal::new(None::<String>);
    let copy_refused = RwSignal::new(None::<(String, ApiError)>);
    let asking = RwSignal::new(None::<String>);
    let deleting = RwSignal::new(None::<String>);
    let in_use = RwSignal::new(None::<InUse>);
    let removal_failed = RwSignal::new(None::<ApiError>);
    let roles = expect_context::<Roles>();
    let counted = StoredValue::new(0u32);
    let halting = StoredValue::new_local(Halting(AbortController::new().ok()));
    let picking = NodeRef::<Input>::new();

    let load = move |after: Option<String>| {
        let asked = whose.get_untracked();
        let fresh = after.is_none();
        loading.set(true);
        spawn_local(async move {
            let page = page_of(asked, of_type, &project.get_value(), after.as_deref()).await;
            if whose.try_get_untracked() != Some(asked) {
                return;
            }
            loading.set(false);
            match page {
                Ok((more, then)) if fresh => {
                    offered.set(more);
                    next.set(then);
                }
                Ok((more, then)) => {
                    offered.update(|offered| offered.extend(more));
                    next.set(then);
                }
                Err(failure) => {
                    if fresh {
                        offered.set(Vec::new());
                        next.set(None);
                    }
                    failed.set(Some(failure));
                }
            }
        });
    };
    Effect::new(move || {
        whose.track();
        failed.set(None);
        load(None);
    });

    let take = move |files: Vec<File>| {
        let batch: Vec<(Sending, File)> = files
            .into_iter()
            .map(|file| {
                counted.update_value(|counted| *counted += 1);
                let shown = Sending {
                    key: counted.get_value(),
                    name: StoredValue::new(file.name()),
                    done: RwSignal::new(0.0),
                    refused: RwSignal::new(None),
                };

                (shown, file)
            })
            .collect();
        sending.update(|sending| {
            sending.splice(0..0, batch.iter().map(|(shown, _)| *shown));
        });
        spawn_local(async move {
            let most = service::most_each_upload().await.ok().flatten();
            let mut queued = Vec::new();
            for (shown, file) in batch {
                match most {
                    Some(most) if file.size() > most as f64 => {
                        shown.refused.try_set(Some(service::too_large(most)));
                    }
                    _ => queued.push((shown, file)),
                }
            }
            for (shown, file) in queued {
                let Some(halted) = halting.try_with_value(Halting::signal).flatten() else {
                    return;
                };
                let done = shown.done;
                let told = service::upload_into(
                    &project.get_value(),
                    &file,
                    move |fraction| {
                        done.try_set(fraction);
                    },
                    Some(&halted),
                )
                .await;
                if halted.aborted() {
                    return;
                }
                match told {
                    Ok(id) => {
                        sending.update(|sending| sending.retain(|other| other.key != shown.key));
                        offered.update(|offered| {
                            offered.insert(
                                0,
                                Offered {
                                    id,
                                    uploaded_at: OffsetDateTime::now_utc(),
                                    kept: Kept::Here,
                                    project: Some(project.get_value()),
                                },
                            );
                        });
                    }
                    Err(failure) => {
                        shown.refused.try_set(Some(failure));
                    }
                }
            }
        });
    };
    let dismiss =
        move |key: u32| sending.update(|sending| sending.retain(|other| other.key != key));
    let choose = move |offered: Offered| {
        if offered.kept == Kept::Here {
            chosen(offered.id);
            return;
        }
        if copying.with_untracked(Option::is_some) {
            return;
        }
        copy_refused.set(None);
        copying.set(Some(offered.id.clone()));
        let into = project.get_value();
        spawn_local(async move {
            let Some(halted) = halting.try_with_value(Halting::signal).flatten() else {
                return;
            };
            let copied = service::copy_into(&offered.id, &into, Some(&halted)).await;
            if halted.aborted() {
                return;
            }
            copying.try_set(None);
            match copied {
                Ok(copy) => chosen(copy),
                Err(failure) => {
                    copy_refused.try_set(Some((offered.id, failure)));
                }
            }
        });
    };
    let delete = move |asset: Offered| {
        asking.set(None);
        in_use.set(None);
        removal_failed.set(None);
        deleting.set(Some(asset.id.clone()));
        spawn_local(async move {
            let deleted = service::delete(&asset.id).await;
            deleting.try_set(None);
            match deleted {
                Ok(()) => {
                    offered.try_update(|offered| offered.retain(|other| other.id != asset.id));
                }
                Err(ApiError::Rejected(refusal))
                    if refusal.kind.as_deref() == Some(refused::IN_USE) =>
                {
                    match service::uses_of(&asset.id).await {
                        Ok(used) => {
                            in_use.try_set(Some(InUse {
                                id: asset.id,
                                project: asset.project,
                                uses: used,
                            }));
                        }
                        Err(failure) => {
                            removal_failed.try_set(Some(failure));
                        }
                    }
                }
                Err(failure) => {
                    removal_failed.try_set(Some(failure));
                }
            }
        });
    };
    let deletable = move |asset: &Offered| match &asset.project {
        Some(project) => roles.may_write(project),
        None => asset.kept == Kept::Own,
    };
    let shown = Shown {
        copying,
        copy_refused,
        asking,
        deleting,
    };
    let here = project.get_value();
    let follow = Callback::new(move |scene: SceneId| {
        if let Some(asset) = in_use.get_untracked() {
            followed(scene, asset.id);
        }
    });

    dialog::modal(
        Signal::derive(move || speech.say("image-choice-title")),
        "image-choice",
        closed,
        (
            signed_in.then(|| filter(whose, speech)),
            move || failed.get().map(problem::problem),
            move || removal_failed.get().map(problem::problem),
            move || {
                in_use.get().map(|asset| {
                    let followed =
                        (asset.project.as_deref() == Some(here.as_str())).then_some(follow);
                    html::div().class("in-use").role("alert").child((
                        html::p().child(move || speech.say("image-choice-in-use")),
                        uses::listed(asset.id, asset.project, asset.uses, followed),
                        html::p()
                            .class("hint")
                            .child(move || speech.say("image-choice-in-use-hint")),
                        html::button()
                            .r#type("button")
                            .class("dismiss")
                            .attr("aria-label", move || speech.say("problem-dismiss"))
                            .on(ev::click, move |_| in_use.set(None))
                            .child("×"),
                    ))
                })
            },
            html::input()
                .node_ref(picking)
                .r#type("file")
                .class("visually-hidden")
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
                    let files: Vec<File> = input
                        .files()
                        .map(|files| {
                            (0..files.length())
                                .filter_map(|nth| files.get(nth))
                                .collect()
                        })
                        .unwrap_or_default();
                    input.set_value("");
                    take(files);
                }),
            html::ul().class("image-grid").child((
                html::li().child(
                    html::button()
                        .r#type("button")
                        .class("upload-new")
                        .on(ev::click, move |_| {
                            if let Some(input) = picking.get_untracked() {
                                input.click();
                            }
                        })
                        .child((
                            icons::mark(Icon::Plus),
                            html::span().child(move || speech.say("image-choice-upload")),
                        )),
                ),
                For(ForProps::builder()
                    .each(move || sending.get())
                    .key(|shown: &Sending| shown.key)
                    .children(move |shown| sending_tile(shown, speech, dismiss))
                    .build()),
                For(ForProps::builder()
                    .each(move || offered.get())
                    .key(|image: &Offered| (image.id.clone(), image.kept.clone()))
                    .children(move |image| {
                        let may_delete = deletable(&image);
                        tile(image, may_delete, shown, speech, choose, delete)
                    })
                    .build()),
            )),
            move || {
                let nothing = !loading.get()
                    && failed.get().is_none()
                    && offered.with(Vec::is_empty)
                    && sending.with(Vec::is_empty);
                nothing.then(|| {
                    html::p()
                        .class("hint")
                        .child(move || speech.say("image-choice-none"))
                })
            },
            move || {
                next.get().is_some().then(|| {
                    blocks::more(
                        "image-choice-more",
                        move || loading.get(),
                        move |_| load(next.get_untracked()),
                    )
                })
            },
            html::div().class("dialog-actions").child(
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| closed())
                    .child(move || speech.say("image-choice-cancel")),
            ),
        ),
    )
}

fn filter(whose: RwSignal<Whose>, speech: Speech) -> impl IntoView {
    let choice = move |shown: Whose, label: &'static str| {
        html::button()
            .r#type("button")
            .attr("aria-pressed", move || (whose.get() == shown).to_string())
            .on(ev::click, move |_| whose.set(shown))
            .child(move || speech.say(label))
    };

    html::div()
        .class("image-filter")
        .role("group")
        .attr("aria-label", move || speech.say("image-choice-whose"))
        .child((
            choice(Whose::Project, "image-choice-project"),
            choice(Whose::Mine, "image-choice-mine"),
        ))
}

fn sending_tile(
    shown: Sending,
    speech: Speech,
    dismiss: impl Fn(u32) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let name = shown.name.get_value();
    let uploading = {
        let mut args = FluentArgs::new();
        args.set("name", name.clone());
        speech.say_with("image-uploading", &args)
    };

    html::li().child(
        html::div()
            .class("image-sending")
            .class(("refused", move || shown.refused.with(Option::is_some)))
            .child((
                html::span().class("sending-name").child(name),
                move || match shown.refused.get() {
                    None => html::div()
                        .class("sending-bar")
                        .role("progressbar")
                        .attr("aria-label", uploading.clone())
                        .attr("aria-valuemin", "0")
                        .attr("aria-valuemax", "100")
                        .attr("aria-valuenow", move || {
                            (shown.done.get() * 100.0).round().to_string()
                        })
                        .style(("--done", move || format!("{}%", shown.done.get() * 100.0)))
                        .into_any(),
                    Some(failure) => (
                        html::span()
                            .class("sending-refused")
                            .child(problem::worded(&failure, speech)),
                        html::button()
                            .r#type("button")
                            .class("dismiss")
                            .attr("aria-label", move || speech.say("problem-dismiss"))
                            .on(ev::click, move |_| dismiss(shown.key))
                            .child("×"),
                    )
                        .into_any(),
                },
            )),
    )
}

/// Which offered asset is being copied into the project, which could not be and why,
/// which one the dialog asks about deleting, and which one is being deleted.
#[derive(Clone, Copy)]
struct Shown {
    copying: RwSignal<Option<String>>,
    copy_refused: RwSignal<Option<(String, ApiError)>>,
    asking: RwSignal<Option<String>>,
    deleting: RwSignal<Option<String>>,
}

fn tile(
    image: Offered,
    may_delete: bool,
    shown: Shown,
    speech: Speech,
    choose: impl Fn(Offered) + Copy + Send + Sync + 'static,
    delete: impl Fn(Offered) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let label = described(&image, speech);
    let source = service::content(&image.id, OFFERED_WIDE);
    let id = StoredValue::new(image.id.clone());
    let copying = move || {
        shown
            .copying
            .with(|copying| copying.as_deref() == Some(&id.get_value()))
    };
    let asked = move || {
        shown
            .asking
            .with(|asking| asking.as_deref() == Some(&id.get_value()))
    };
    let deleting = move || {
        shown
            .deleting
            .with(|deleting| deleting.as_deref() == Some(&id.get_value()))
    };
    let deleted = image.clone();

    html::li().class("offered").child((
        html::button()
            .r#type("button")
            .class("image-offered")
            .attr("aria-label", label.clone())
            .attr("title", label)
            .attr("aria-busy", move || (copying() || deleting()).to_string())
            .on(ev::click, move |_| choose(image.clone()))
            .child((
                html::img().src(source).alt("").attr("loading", "lazy"),
                move || {
                    copying().then(|| {
                        html::span()
                            .class("tile-note")
                            .child(move || speech.say("image-copying"))
                    })
                },
                move || {
                    shown
                        .copy_refused
                        .get()
                        .filter(|(refused, _)| *refused == id.get_value())
                        .map(|(_, failure)| {
                            html::span()
                                .class("tile-note refused")
                                .role("alert")
                                .child(problem::worded(&failure, speech))
                        })
                },
            )),
        may_delete.then(|| {
            html::button()
                .r#type("button")
                .class("tile-remove")
                .attr("aria-label", move || speech.say("image-choice-delete"))
                .on(ev::click, move |_| shown.asking.set(Some(id.get_value())))
                .child("×")
        }),
        move || {
            asked().then(|| {
                let deleted = deleted.clone();
                html::div()
                    .class("tile-asking")
                    .role("group")
                    .attr("aria-label", move || {
                        speech.say("image-choice-delete-asking")
                    })
                    .child((
                        html::span().child(move || speech.say("image-choice-delete-asking")),
                        html::button()
                            .r#type("button")
                            .class("danger")
                            .on(ev::click, move |_| delete(deleted.clone()))
                            .child(move || speech.say("image-choice-delete-confirm")),
                        html::button()
                            .r#type("button")
                            .on(ev::click, move |_| shown.asking.set(None))
                            .child(move || speech.say("image-choice-delete-keep")),
                    ))
            })
        },
    ))
}

fn described(image: &Offered, speech: Speech) -> String {
    let mut args = FluentArgs::new();
    args.set("when", moment::human_day(image.uploaded_at));
    match &image.kept {
        Kept::Here => speech.say_with("image-choice-here", &args),
        Kept::Own => speech.say_with("image-choice-own", &args),
        Kept::Elsewhere(Some(name)) => {
            args.set("project", name.clone());
            speech.say_with("image-choice-elsewhere", &args)
        }
        Kept::Elsewhere(None) => speech.say_with("image-choice-elsewhere-unnamed", &args),
    }
}

async fn page_of(
    whose: Whose,
    of_type: AssetType,
    project: &str,
    after: Option<&str>,
) -> Result<(Vec<Offered>, Option<String>), ApiError> {
    match whose {
        Whose::Project => {
            let page = service::assets_in(project, Some(of_type), after).await?;
            let offered = page
                .assets
                .into_iter()
                .map(|asset| Offered {
                    id: asset.id,
                    uploaded_at: asset.uploaded_at,
                    kept: Kept::Here,
                    project: Some(project.to_owned()),
                })
                .collect();

            Ok((offered, page.next))
        }
        Whose::Mine => {
            let page = service::uploads(UploadsSorted::Newest, Some(of_type), after).await?;
            let offered = page
                .uploads
                .into_iter()
                .map(|upload| Offered {
                    kept: match &upload.project {
                        None => Kept::Own,
                        Some(kept) if kept == project => Kept::Here,
                        Some(_) => Kept::Elsewhere(upload.project_name),
                    },
                    project: upload.project,
                    id: upload.id,
                    uploaded_at: upload.uploaded_at,
                })
                .collect();

            Ok((offered, page.next))
        }
    }
}
