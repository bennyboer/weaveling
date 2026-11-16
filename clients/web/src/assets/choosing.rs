use assets_contract::refused;
use fluent_bundle::FluentArgs;
use leptos::html::{Button, Input};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::{IntoView, ev, html};
use time::OffsetDateTime;
use wasm_bindgen::JsCast;
use web_sys::{AbortController, AbortSignal, File, HtmlElement, HtmlInputElement};

use crate::accounts::{CurrentSession, SignInState};
use crate::act::{Act, Here};
use crate::assets::paged::Paged;
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

/// What the dialog for choosing an image shows and is doing:
/// whose assets it offers, the offered assets a page at a time, the files being uploaded,
/// the asset being copied in or deleted, which tile asks about deleting, and an asset that is still in use.
#[derive(Clone, Copy)]
struct ChoiceDialogState {
    project: StoredValue<String>,
    of_type: AssetType,
    whose: RwSignal<Whose>,
    offered: Paged<Offered>,
    failed: RwSignal<Option<ApiError>>,
    sending: RwSignal<Vec<Sending>>,
    counted: StoredValue<u32>,
    copying: Act<Offered>,
    copy_refused: RwSignal<Option<(String, ApiError)>>,
    asking: RwSignal<Option<String>>,
    deleting: Act<Offered>,
    in_use: RwSignal<Option<InUse>>,
    removal_failed: RwSignal<Option<ApiError>>,
    halting: StoredValue<Halting, LocalStorage>,
}

impl ChoiceDialogState {
    /// Opens the dialog's state for a project, and reads the first page of what it offers.
    /// `chosen` is told the id of the project's asset that was chosen, a copy made here if it was kept elsewhere.
    fn open(
        project: String,
        of_type: AssetType,
        chosen: impl Fn(String) + Copy + Send + Sync + 'static,
    ) -> Self {
        let halting = StoredValue::new_local(Halting(AbortController::new().ok()));
        let copy_refused = RwSignal::new(None);
        let project = StoredValue::new(project);
        let copying = Act::new(move |offered: Offered, here: Here| async move {
            let Some(halted) = halting.try_with_value(Halting::signal).flatten() else {
                return;
            };
            let copied = service::copy_into(&offered.id, &project.get_value(), Some(&halted)).await;
            if halted.aborted() || !here.still() {
                return;
            }
            match copied {
                Ok(copy) => chosen(copy),
                Err(failure) => copy_refused.set(Some((offered.id, failure))),
            }
        });
        let offered = Paged::new();
        let in_use = RwSignal::new(None);
        let removal_failed = RwSignal::new(None);
        let deleting = Act::new(move |asset: Offered, here: Here| async move {
            let at = offered.listed.with_untracked(|listed: &Vec<Offered>| {
                listed.iter().position(|other| other.id == asset.id)
            });
            let deleted = service::delete(&asset.id).await;
            if !here.still() {
                return;
            }
            match deleted {
                Ok(()) => {
                    offered
                        .listed
                        .update(|listed| listed.retain(|other| other.id != asset.id));
                    focus_tile_at(at.unwrap_or(0));
                }
                Err(ApiError::Rejected(refusal))
                    if refusal.kind.as_deref() == Some(refused::IN_USE) =>
                {
                    match service::asset(&asset.id).await {
                        Ok(told) => in_use.set(Some(InUse {
                            id: asset.id,
                            project: asset.project,
                            uses: told.uses,
                        })),
                        Err(failure) => removal_failed.set(Some(failure)),
                    }
                }
                Err(failure) => removal_failed.set(Some(failure)),
            }
        });
        let state = Self {
            project,
            of_type,
            whose: RwSignal::new(Whose::Project),
            offered,
            failed: RwSignal::new(None),
            sending: RwSignal::new(Vec::new()),
            counted: StoredValue::new(0),
            copying,
            copy_refused,
            asking: RwSignal::new(None),
            deleting,
            in_use,
            removal_failed,
            halting,
        };
        Effect::new(move || {
            state.whose.track();
            state.failed.set(None);
            state.load(None);
        });

        state
    }

    /// Loads a page of what the dialog offers.
    /// Without a cursor it replaces what is shown, so switching whose assets are offered shows the new ones at once.
    fn load(self, after: Option<String>) {
        let whose = self.whose.get_untracked();
        let of_type = self.of_type;
        let project = self.project.get_value();
        let offered = self.offered;
        let failed = self.failed;
        let fresh = after.is_none();
        self.offered.load(
            after,
            move |failure| {
                if fresh {
                    offered.listed.set(Vec::new());
                    offered.next.set(None);
                }
                failed.set(Some(failure));
            },
            move |after| async move { page_of(whose, of_type, &project, after.as_deref()).await },
        );
    }

    /// Uploads files into the project, each shown first in the grid with how far it got.
    /// An uploaded file is offered like any other asset of the project; a refused one says why until dismissed.
    fn take(self, files: Vec<File>) {
        let tiles: Vec<Sending> = files
            .iter()
            .map(|file| {
                self.counted.update_value(|counted| *counted += 1);
                Sending {
                    key: self.counted.get_value(),
                    name: StoredValue::new(file.name()),
                    done: RwSignal::new(0.0),
                    refused: RwSignal::new(None),
                }
            })
            .collect();
        self.sending
            .update(|sending| _ = sending.splice(0..0, tiles.iter().copied()));
        let Some(halted) = self.halting.with_value(Halting::signal) else {
            return;
        };
        let project = self.project.get_value();
        let progressing = tiles.clone();
        spawn_local(async move {
            service::upload_each_into(
                &project,
                files,
                Some(halted),
                move |nth, fraction| {
                    progressing[nth].done.try_set(fraction);
                },
                |nth, told| match told {
                    Ok(id) => {
                        self.sending
                            .update(|sending| sending.retain(|other| other.key != tiles[nth].key));
                        self.offered.listed.update(|offered| {
                            offered.insert(
                                0,
                                Offered {
                                    id,
                                    uploaded_at: OffsetDateTime::now_utc(),
                                    kept: Kept::Here,
                                    project: Some(project.clone()),
                                },
                            );
                        });
                    }
                    Err(failure) => {
                        tiles[nth].refused.try_set(Some(failure));
                    }
                },
            )
            .await;
        });
    }

    fn dismiss(self, key: u32) {
        self.sending
            .update(|sending| sending.retain(|other| other.key != key));
    }

    fn delete(self, asset: Offered) {
        self.asking.set(None);
        self.in_use.set(None);
        self.removal_failed.set(None);
        self.deleting.start(asset);
    }
}

/// Moves the focus to the tile now at a place in the grid, or the last one, or to uploading a new image.
fn focus_tile_at(at: usize) {
    _ = request_animation_frame(move || {
        let tiles = document()
            .query_selector_all(".dialog.image-choice .image-offered")
            .ok();
        let tile = tiles.and_then(|tiles| match tiles.length() {
            0 => None,
            many => tiles.get((at as u32).min(many - 1)),
        });
        let focused = tile.or_else(|| {
            document()
                .query_selector(".dialog.image-choice .upload-new")
                .ok()
                .flatten()
                .map(Into::into)
        });
        if let Some(focused) = focused.and_then(|focused| focused.dyn_into::<HtmlElement>().ok()) {
            let _ = focused.focus();
        }
    });
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
    let roles = expect_context::<Roles>();
    let here = project.clone();
    let state = ChoiceDialogState::open(project, of_type, chosen);
    let picking = NodeRef::<Input>::new();

    let choose = move |offered: Offered| match offered.kept {
        Kept::Here => chosen(offered.id),
        Kept::Own | Kept::Elsewhere(_) => {
            state.copy_refused.set(None);
            state.copying.start(offered);
        }
    };
    let deletable = move |asset: &Offered| match &asset.project {
        Some(project) => roles.may_write(project),
        None => asset.kept == Kept::Own,
    };
    let follow = Callback::new(move |scene: SceneId| {
        if let Some(asset) = state.in_use.get_untracked() {
            followed(scene, asset.id);
        }
    });

    dialog::modal(
        Signal::derive(move || speech.say("image-choice-title")),
        "image-choice",
        closed,
        (
            signed_in.then(|| filter(state.whose, speech)),
            move || state.failed.get().map(problem::problem),
            move || state.removal_failed.get().map(problem::problem),
            move || {
                state.in_use.get().map(|asset| {
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
                            .on(ev::click, move |_| state.in_use.set(None))
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
                    state.take(files);
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
                    .each(move || state.sending.get())
                    .key(|shown: &Sending| shown.key)
                    .children(move |shown| sending_tile(shown, state, speech))
                    .build()),
                For(ForProps::builder()
                    .each(move || state.offered.listed.get())
                    .key(|image: &Offered| (image.id.clone(), image.kept.clone()))
                    .children(move |image| {
                        let may_delete = deletable(&image);
                        tile(image, may_delete, state, speech, choose)
                    })
                    .build()),
            )),
            move || {
                let nothing = state.offered.nothing()
                    && state.failed.with(Option::is_none)
                    && state.sending.with(Vec::is_empty);
                nothing.then(|| {
                    html::p()
                        .class("hint")
                        .child(move || speech.say("image-choice-none"))
                })
            },
            move || {
                state.offered.next.get().is_some().then(|| {
                    blocks::more(
                        "image-choice-more",
                        move || state.offered.loading.get(),
                        move |_| state.load(state.offered.next.get_untracked()),
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

fn sending_tile(shown: Sending, state: ChoiceDialogState, speech: Speech) -> impl IntoView {
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
                            .role("alert")
                            .child(problem::worded(&failure, speech)),
                        html::button()
                            .r#type("button")
                            .class("dismiss")
                            .attr("aria-label", move || speech.say("problem-dismiss"))
                            .on(ev::click, move |_| state.dismiss(shown.key))
                            .child("×"),
                    )
                        .into_any(),
                },
            )),
    )
}

fn tile(
    image: Offered,
    may_delete: bool,
    state: ChoiceDialogState,
    speech: Speech,
    choose: impl Fn(Offered) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let label = described(&image, speech);
    let source = service::content(&image.id, OFFERED_WIDE);
    let id = StoredValue::new(image.id.clone());
    let asked = move || {
        state
            .asking
            .with(|asking| asking.as_deref() == Some(&id.get_value()))
    };
    let deleted = image.clone();
    let busy = image.clone();
    let offered: NodeRef<Button> = NodeRef::new();

    html::li().class("offered").child((
        html::button()
            .node_ref(offered)
            .r#type("button")
            .class("image-offered")
            .attr("aria-label", label.clone())
            .attr("title", label)
            .attr("aria-busy", move || {
                state
                    .copying
                    .busy_on(&busy)
                    .or_else(|| state.deleting.busy_on(&busy))
                    .unwrap_or("false")
            })
            .on(ev::click, move |_| choose(image.clone()))
            .child((
                html::img().src(source).alt("").attr("loading", "lazy"),
                {
                    let copying = deleted.clone();
                    move || {
                        state.copying.busy_on(&copying).map(|_| {
                            html::span()
                                .class("tile-note")
                                .child(move || speech.say("image-copying"))
                        })
                    }
                },
                move || {
                    state
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
                .on(ev::click, move |_| state.asking.set(Some(id.get_value())))
                .child("×")
        }),
        move || {
            asked().then(|| {
                let deleted = deleted.clone();
                let confirm: NodeRef<Button> = NodeRef::new();
                Effect::new(move || {
                    if let Some(confirm) = confirm.get() {
                        let _ = confirm.focus();
                    }
                });
                html::div()
                    .class("tile-asking")
                    .role("group")
                    .attr("aria-label", move || {
                        speech.say("image-choice-delete-asking")
                    })
                    .child((
                        html::span().child(move || speech.say("image-choice-delete-asking")),
                        html::button()
                            .node_ref(confirm)
                            .r#type("button")
                            .class("danger")
                            .on(ev::click, move |_| state.delete(deleted.clone()))
                            .child(move || speech.say("image-choice-delete-confirm")),
                        html::button()
                            .r#type("button")
                            .on(ev::click, move |_| {
                                state.asking.set(None);
                                if let Some(offered) = offered.get_untracked() {
                                    let _ = offered.focus();
                                }
                            })
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
