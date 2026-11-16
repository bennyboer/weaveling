use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::{IntoView, ev, html};
use time::OffsetDateTime;

use crate::accounts::{CurrentSession, SignInState};
use crate::assets::service as assets;
use crate::assets::service::UploadsSorted;
use crate::blocks;
use crate::dialog;
use crate::http::ApiError;
use crate::icons;
use crate::icons::Icon;
use crate::moment;
use crate::problem;
use crate::scenes::model::SceneId;
use crate::speech::Speech;
use crate::writing::images;
use crate::writing::write_view_state::WriteViewState;

/// The width the images are fetched at to be offered as small tiles.
const OFFERED_WIDE: u32 = 384;

/// Which scene asked for an image, and the place in its text the image goes to.
/// While it is set, the dialog for choosing an image is open.
#[derive(Clone, Copy)]
pub struct ImageChoice {
    asked: RwSignal<Option<(SceneId, u32)>>,
}

impl ImageChoice {
    pub fn nothing() -> Self {
        Self {
            asked: RwSignal::new(None),
        }
    }

    /// Opens the dialog for an image to go to the place the ticket stands for.
    pub fn ask(self, scene: SceneId, ticket: u32) {
        self.asked.set(Some((scene, ticket)));
    }

    fn closed(self) {
        self.asked.set(None);
    }
}

/// Whose images the dialog offers: the project's, or everything the signed-in account uploaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Whose {
    Project,
    Mine,
}

/// Where an offered image is kept, as far as this project is concerned.
/// Only an image kept here goes into the text as it is. Any other is copied into the project first.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Kept {
    Here,
    Own,
    Elsewhere(Option<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Offered {
    id: String,
    uploaded_at: OffsetDateTime,
    kept: Kept,
}

/// The dialog for choosing an image to put into a scene, shown while a scene asks for one.
/// It offers the project's images, or with an account the account's own uploads too,
/// the newest first, and uploading a new one.
pub fn dialog(state: WriteViewState) -> impl IntoView {
    move || {
        state
            .image_choice
            .asked
            .get()
            .map(|(scene, ticket)| chooser(scene, ticket, state))
    }
}

fn chooser(scene: SceneId, ticket: u32, state: WriteViewState) -> impl IntoView {
    let speech = expect_context::<Speech>();
    let signed_in = matches!(
        expect_context::<CurrentSession>().state(),
        SignInState::SignedIn(_)
    );
    let scene = StoredValue::new(scene);
    let project = StoredValue::new(state.structure.project().to_string());
    let whose = RwSignal::new(Whose::Project);
    let offered = RwSignal::new(Vec::<Offered>::new());
    let next = RwSignal::new(None::<String>);
    let loading = RwSignal::new(true);
    let failed = RwSignal::new(None::<ApiError>);

    let load = move |after: Option<String>| {
        let asked = whose.get_untracked();
        let fresh = after.is_none();
        loading.set(true);
        spawn_local(async move {
            let page = page_of(asked, &project.get_value(), after.as_deref()).await;
            if whose.get_untracked() != asked {
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

    let close = move || {
        state.image_choice.closed();
        state.editors.told(&scene.get_value(), |editor| {
            editor.forget_waiting(ticket);
            editor.focus();
        });
    };
    let choose = move |chosen: Offered| {
        state.image_choice.closed();
        let scene = scene.get_value();
        if chosen.kept == Kept::Here {
            state.editors.told(&scene, |editor| {
                editor.insert_figure(&chosen.id, ticket);
                editor.forget_waiting(ticket);
                editor.focus();
            });
        } else {
            images::copied(scene, state, speech, chosen.id, ticket);
        }
    };
    let upload_new = move |_| {
        state.image_choice.closed();
        images::pick(&scene.get_value(), ticket);
    };

    dialog::modal(
        Signal::derive(move || speech.say("image-choice-title")),
        "image-choice",
        close,
        (
            signed_in.then(|| filter(whose, speech)),
            move || failed.get().map(problem::problem),
            html::ul().class("image-grid").child((
                html::li().child(
                    html::button()
                        .r#type("button")
                        .class("upload-new")
                        .on(ev::click, upload_new)
                        .child((
                            icons::mark(Icon::Plus),
                            html::span().child(move || speech.say("image-choice-upload")),
                        )),
                ),
                For(ForProps::builder()
                    .each(move || offered.get())
                    .key(|image: &Offered| (image.id.clone(), image.kept.clone()))
                    .children(move |image| tile(image, speech, choose))
                    .build()),
            )),
            move || {
                (!loading.get() && failed.get().is_none() && offered.with(Vec::is_empty)).then(
                    || {
                        html::p()
                            .class("hint")
                            .child(move || speech.say("image-choice-none"))
                    },
                )
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
                    .on(ev::click, move |_| close())
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

fn tile(
    image: Offered,
    speech: Speech,
    choose: impl Fn(Offered) + Copy + 'static,
) -> impl IntoView {
    let label = described(&image, speech);
    let source = assets::content(&image.id, OFFERED_WIDE);

    html::li().child(
        html::button()
            .r#type("button")
            .class("image-offered")
            .attr("aria-label", label.clone())
            .attr("title", label)
            .on(ev::click, move |_| choose(image.clone()))
            .child(html::img().src(source).alt("").attr("loading", "lazy")),
    )
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
    project: &str,
    after: Option<&str>,
) -> Result<(Vec<Offered>, Option<String>), ApiError> {
    match whose {
        Whose::Project => {
            let page = assets::images_in(project, after).await?;
            let offered = page
                .images
                .into_iter()
                .map(|image| Offered {
                    id: image.id,
                    uploaded_at: image.uploaded_at,
                    kept: Kept::Here,
                })
                .collect();

            Ok((offered, page.next))
        }
        Whose::Mine => {
            let page = assets::uploads(UploadsSorted::Newest, after).await?;
            let offered = page
                .uploads
                .into_iter()
                .map(|upload| Offered {
                    kept: match upload.project {
                        None => Kept::Own,
                        Some(kept) if kept == project => Kept::Here,
                        Some(_) => Kept::Elsewhere(upload.project_name),
                    },
                    id: upload.id,
                    uploaded_at: upload.uploaded_at,
                })
                .collect();

            Ok((offered, page.next))
        }
    }
}
