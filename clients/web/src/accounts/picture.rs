use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::{IntoView, ev, html};
use wasm_bindgen::JsCast;
use web_sys::{File, HtmlInputElement};

use crate::accounts::{CurrentSession, SignedInAccount, service};
use crate::act::{Act, Here};
use crate::assets::service as assets;
use crate::assets::squaring;
use crate::avatar;
use crate::dialog;
use crate::http::ApiError;
use crate::problem;
use crate::speech;

/// The width asked for the picture on the account page, which shows it 192 pixels wide.
const SHOWN: u32 = 384;

/// The image types a picture may be chosen from.
const ACCEPTED: &str = "image/jpeg,image/png,image/gif,image/webp";

/// What the picture section is asking the account.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Asking {
    Nothing,
    /// A new picture is uploaded. Should the one shown before be kept as a file, or deleted?
    Replacing {
        picture: String,
    },
    /// Should the picture be kept as a file once it is no longer shown, or deleted?
    Removing,
}

/// What the picture section sends to the server.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Change {
    Show {
        picture: String,
        drop_previous: bool,
    },
    Remove {
        drop: bool,
    },
}

/// The profile card on the account's own page: its picture, name and address.
/// The picture is shown, chosen, replaced and removed here.
pub fn profile_card(session: CurrentSession, account: SignedInAccount) -> impl IntoView {
    let shown = RwSignal::new(account.picture.clone());
    let asking = RwSignal::new(Asking::Nothing);
    let refused = RwSignal::new(None::<ApiError>);
    let unreadable = RwSignal::new(false);
    let me = account.account.clone();

    let changing = Act::new(move |change: Change, _: Here| async move {
        let done = match &change {
            Change::Show {
                picture,
                drop_previous,
            } => service::change_picture(picture, *drop_previous).await,
            Change::Remove { drop } => service::remove_picture(*drop).await,
        };
        match done {
            Ok(()) => {
                let now = match change {
                    Change::Show { picture, .. } => Some(picture),
                    Change::Remove { .. } => None,
                };
                shown.set(now.clone());
                session.picture_shown(now);
                asking.set(Asking::Nothing);
            }
            Err(failed) => refused.set(Some(failed)),
        }
    });

    let uploading = RwSignal::new(false);
    let me_choosing = me.clone();
    let chosen = move |input: HtmlInputElement| {
        let me = me_choosing.clone();
        refused.set(None);
        unreadable.set(false);
        let Some(file) = input.files().and_then(|files| files.get(0)) else {
            return;
        };
        input.set_value("");
        uploading.set(true);
        spawn_local(async move {
            let next = uploaded(&file, shown.get_untracked(), &me).await;
            uploading.set(false);
            match next {
                Ok(Next::Ask(picture)) => asking.set(Asking::Replacing { picture }),
                Ok(Next::Show(picture)) => changing.start(Change::Show {
                    picture,
                    drop_previous: false,
                }),
                Err(Failed::Unreadable) => unreadable.set(true),
                Err(Failed::Refused(failed)) => refused.set(Some(failed)),
            }
        });
    };

    let me_removing = me.clone();
    let removing = Act::new(move |(), _: Here| {
        let me = me_removing.clone();
        async move {
            refused.set(None);
            let Some(picture) = shown.get_untracked() else {
                return;
            };
            if assets::used_only_as_picture_of(&picture, &me)
                .await
                .unwrap_or(false)
            {
                asking.set(Asking::Removing);
            } else {
                changing.start(Change::Remove { drop: false });
            }
        }
    });

    let name = account.name.clone();
    let tint = avatar::tint_of(&account.account);

    let email = account.email.clone();
    let heading = account.name.clone();

    html::section().class("card profile").child((
        move || match shown.get() {
            Some(picture) => html::img()
                .class("account-picture")
                .src(assets::content(&picture, SHOWN))
                .alt("")
                .into_any(),
            None => html::span()
                .class(format!("account-picture initial {tint}"))
                .attr("aria-hidden", "true")
                .child(avatar::initial_of(&name))
                .into_any(),
        },
        html::h2().child(heading),
        html::p().class("email").child(email),
        html::div().class("picture-actions").child((
            html::label()
                .class("button")
                .attr("aria-busy", move || {
                    (uploading.get() || changing.running()).then_some("true")
                })
                .child((
                    move || {
                        if shown.with(Option::is_some) {
                            speech::say("your-account-picture-change")
                        } else {
                            speech::say("your-account-picture-choose")
                        }
                    },
                    html::input()
                        .r#type("file")
                        .class("visually-hidden")
                        .attr("accept", ACCEPTED)
                        .on(ev::change, move |event| {
                            if let Some(input) = event
                                .target()
                                .and_then(|target| target.dyn_into::<HtmlInputElement>().ok())
                            {
                                chosen(input);
                            }
                        }),
                )),
            move || {
                shown.with(Option::is_some).then(|| {
                    html::button()
                        .r#type("button")
                        .attr("aria-busy", move || removing.busy())
                        .on(ev::click, move |_| removing.start(()))
                        .child(speech::said("your-account-picture-remove"))
                })
            },
        )),
        html::p()
            .class("hint")
            .child(speech::said("your-account-picture-hint")),
        move || {
            unreadable.get().then(|| {
                html::p()
                    .class("problem")
                    .role("alert")
                    .child(speech::said("your-account-picture-unreadable"))
            })
        },
        move || {
            asking
                .with(|asking| *asking == Asking::Nothing)
                .then(|| refused.get().map(problem::problem))
        },
        move || match asking.get() {
            Asking::Nothing => None,
            Asking::Replacing { picture } => {
                Some(replacing(picture, changing, asking, refused).into_any())
            }
            Asking::Removing => Some(removing_dialog(changing, asking, refused).into_any()),
        },
    ))
}

/// What follows an upload: asking whether to keep the picture shown before, or showing the new one at once.
enum Next {
    Ask(String),
    Show(String),
}

enum Failed {
    Unreadable,
    Refused(ApiError),
}

/// Squares the chosen file, uploads it, and decides what follows.
/// The account is asked about the picture shown before only if nothing but its picture uses it.
async fn uploaded(file: &File, previous: Option<String>, me: &str) -> Result<Next, Failed> {
    let bytes = squaring::squared(file)
        .await
        .map_err(|_| Failed::Unreadable)?;
    let picture = assets::upload_own(&bytes).await.map_err(Failed::Refused)?;
    let ask = match previous {
        Some(previous) => assets::used_only_as_picture_of(&previous, me)
            .await
            .unwrap_or(false),
        None => false,
    };

    Ok(if ask {
        Next::Ask(picture)
    } else {
        Next::Show(picture)
    })
}

fn replacing(
    picture: String,
    changing: Act<Change>,
    asking: RwSignal<Asking>,
    refused: RwSignal<Option<ApiError>>,
) -> impl IntoView {
    let close = move || {
        refused.set(None);
        asking.set(Asking::Nothing);
    };
    let keeping = picture.clone();

    dialog::modal(
        Signal::derive(|| speech::say("picture-replaced-heading")),
        "replacing-picture",
        close,
        (
            html::p().child(speech::said("picture-replaced-hint")),
            move || refused.get().map(problem::problem),
            html::div().class("dialog-actions").child((
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| close())
                    .child(speech::said("picture-cancel")),
                html::button()
                    .r#type("button")
                    .attr("aria-busy", move || changing.busy())
                    .on(ev::click, move |_| {
                        changing.start(Change::Show {
                            picture: keeping.clone(),
                            drop_previous: false,
                        })
                    })
                    .child(speech::said("picture-keep")),
                html::button()
                    .r#type("button")
                    .class("danger")
                    .attr("aria-busy", move || changing.busy())
                    .on(ev::click, move |_| {
                        changing.start(Change::Show {
                            picture: picture.clone(),
                            drop_previous: true,
                        })
                    })
                    .child(speech::said("picture-delete")),
            )),
        ),
    )
}

fn removing_dialog(
    changing: Act<Change>,
    asking: RwSignal<Asking>,
    refused: RwSignal<Option<ApiError>>,
) -> impl IntoView {
    let close = move || {
        refused.set(None);
        asking.set(Asking::Nothing);
    };

    dialog::modal(
        Signal::derive(|| speech::say("picture-removed-heading")),
        "removing-picture",
        close,
        (
            html::p().child(speech::said("picture-removed-hint")),
            move || refused.get().map(problem::problem),
            html::div().class("dialog-actions").child((
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| close())
                    .child(speech::said("picture-cancel")),
                html::button()
                    .r#type("button")
                    .attr("aria-busy", move || changing.busy())
                    .on(ev::click, move |_| {
                        changing.start(Change::Remove { drop: false })
                    })
                    .child(speech::said("picture-remove-keep")),
                html::button()
                    .r#type("button")
                    .class("danger")
                    .attr("aria-busy", move || changing.busy())
                    .on(ev::click, move |_| {
                        changing.start(Change::Remove { drop: true })
                    })
                    .child(speech::said("picture-remove-delete")),
            )),
        ),
    )
}
