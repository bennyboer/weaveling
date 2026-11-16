use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::{IntoView, component, ev, html};

use crate::accounts::{CurrentSession, SignInState};
use crate::act::{Act, Here};
use crate::assets::paged::Paged;
use crate::assets::service;
use crate::assets::service::{Purpose, Upload, UploadsSorted, Usage, Use};
use crate::assets::uses as assets_uses;
use crate::blocks;
use crate::dialog;
use crate::http::ApiError;
use crate::moment;
use crate::problem;
use crate::shell;
use crate::size;
use crate::speech;
use crate::speech::Speech;

/// The width asked for the thumbnails, in pixels.
/// They are shown 64 pixels wide, so this stays sharp on screens with three times the pixel density.
const THUMBNAIL: u32 = 192;

/// The width asked for an upload shown bigger.
/// It is shown at most about 760 pixels wide, so this stays sharp on screens with twice the pixel density.
const VIEWED: u32 = 1_536;

/// What the storage page shows and is doing.
#[derive(Clone, Copy)]
struct StoragePageState {
    sorted: RwSignal<UploadsSorted>,
    uploads: Paged<Upload>,
    usage: RwSignal<Option<Usage>>,
    refused: RwSignal<Option<ApiError>>,
    deleting: RwSignal<Option<Upload>>,
    viewing: RwSignal<Option<Viewed>>,
}

/// An upload shown bigger in a dialog: its id, the words that say whose it is,
/// the project it is in, if any, and what uses it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Viewed {
    id: String,
    whose: String,
    project: Option<String>,
    uses: Vec<Use>,
}

impl StoragePageState {
    fn open() -> Self {
        let shelf = Self {
            sorted: RwSignal::new(UploadsSorted::Newest),
            uploads: Paged::new(),
            usage: RwSignal::new(None),
            refused: RwSignal::new(None),
            deleting: RwSignal::new(None),
            viewing: RwSignal::new(None),
        };
        shelf.load(None);
        shelf.measure();

        shelf
    }

    /// Loads a page of uploads.
    /// Without a cursor it replaces the list, with one it adds the next page to it.
    fn load(self, after: Option<String>) {
        let sorted = self.sorted.get_untracked();
        let refused = self.refused;
        self.uploads.load(
            after,
            move |failed| refused.set(Some(failed)),
            move |after| async move {
                service::uploads(sorted, None, after.as_deref())
                    .await
                    .map(|page| (page.uploads, page.next))
            },
        );
    }

    fn measure(self) {
        spawn_local(async move {
            match service::usage().await {
                Ok(usage) => self.usage.set(Some(usage)),
                Err(failed) => self.refused.set(Some(failed)),
            }
        });
    }

    fn sort(self, sorted: UploadsSorted) {
        self.sorted.set(sorted);
        self.load(None);
    }

    fn gone(self, id: &str) {
        self.uploads
            .listed
            .update(|shown| shown.retain(|upload| upload.id != id));
        self.measure();
    }
}

#[component]
pub fn StoragePage() -> impl IntoView {
    let session = expect_context::<CurrentSession>();

    (
        shell::masthead(None),
        blocks::page(
            "storage",
            speech::said("storage-heading"),
            Some("storage-lead"),
            (),
            move || match session.state() {
                SignInState::SignedIn(_) => Some(storage()),
                _ => None,
            },
        ),
    )
}

fn storage() -> impl IntoView {
    let shelf = StoragePageState::open();
    let speech = expect_context::<Speech>();

    (
        move || {
            shelf
                .usage
                .get()
                .map(|usage| blocks::usage_card(used(usage, speech), speech::said("storage-hint")))
        },
        move || {
            shelf
                .deleting
                .with(Option::is_none)
                .then(|| shelf.refused.get().map(problem::problem))
        },
        blocks::list_card(
            "uploads",
            speech::said("storage-media"),
            sorting(shelf),
            (
                move || {
                    shelf
                        .uploads
                        .nothing()
                        .then(|| html::p().class("hint").child(speech::said("storage-none")))
                },
                blocks::rows(move || {
                    shelf
                        .uploads
                        .listed
                        .get()
                        .into_iter()
                        .map(|upload| listed(shelf, upload, speech))
                        .collect_view()
                }),
            ),
            move || {
                shelf.uploads.next.get().map(|next| {
                    blocks::more(
                        "storage-more",
                        move || shelf.uploads.loading.get(),
                        move |_| shelf.load(Some(next.clone())),
                    )
                })
            },
        ),
        move || shelf.deleting.get().map(|upload| deletion(shelf, upload)),
        move || shelf.viewing.get().map(|viewed| viewing(shelf, viewed)),
    )
}

fn sorting(shelf: StoragePageState) -> impl IntoView {
    html::label().class("sorting").child((
        html::span()
            .class("visually-hidden")
            .child(speech::said("storage-sort")),
        html::select()
            .on(ev::change, move |event| {
                if let Some(sorted) = UploadsSorted::of(&event_target_value(&event)) {
                    shelf.sort(sorted);
                }
            })
            .child((
                html::option()
                    .value(UploadsSorted::Newest.named())
                    .selected(move || shelf.sorted.get() == UploadsSorted::Newest)
                    .child(speech::said("storage-sort-newest")),
                html::option()
                    .value(UploadsSorted::Largest.named())
                    .selected(move || shelf.sorted.get() == UploadsSorted::Largest)
                    .child(speech::said("storage-sort-largest")),
            )),
    ))
}

pub(crate) fn used(usage: Usage, speech: Speech) -> impl IntoView {
    let told = move || {
        let mut args = FluentArgs::new();
        args.set("used", size::told(usage.used, speech.language()));
        match usage.most {
            Some(most) => {
                args.set("most", size::told(most, speech.language()));
                speech.say_with("storage-used", &args)
            }
            None => speech.say_with("storage-used-unbounded", &args),
        }
    };

    blocks::usage(told, usage.used, usage.most, move || {
        speech.say("storage-heading")
    })
}

fn listed(shelf: StoragePageState, upload: Upload, speech: Speech) -> impl IntoView {
    let id = upload.id.clone();
    let whose = {
        let upload = upload.clone();
        move || match (&upload.project, &upload.project_name) {
            (None, _) => speech.say("storage-own"),
            (Some(_), Some(name)) => {
                let mut args = FluentArgs::new();
                args.set("project", name.clone());
                speech.say_with("storage-in-project", &args)
            }
            (Some(_), None) => speech.say("storage-in-a-project"),
        }
    };
    let detail = move || {
        let mut args = FluentArgs::new();
        args.set("size", size::told(upload.total_size, speech.language()));
        args.set("when", moment::human_day(upload.uploaded_at));
        speech.say_with("storage-detail", &args)
    };
    let deletable = upload.deletable;
    let asked = upload.clone();
    let telling = whose.clone();
    let viewed = move || Viewed {
        id: upload.id.clone(),
        whose: telling(),
        project: upload.project.clone(),
        uses: upload.uses.clone(),
    };
    let marked = viewed.clone();
    let mark = not_deletable_because(&asked.uses);
    let in_use = !asked.uses.is_empty();

    blocks::row(
        html::button()
            .r#type("button")
            .class("thumbnail-button")
            .attr("aria-label", move || speech.say("storage-view"))
            .on(ev::click, move |_| shelf.viewing.set(Some(viewed())))
            .child(
                html::img()
                    .class("thumbnail")
                    .src(service::content(&id, THUMBNAIL))
                    .alt("")
                    .attr("loading", "lazy"),
            ),
        whose,
        detail,
        (!deletable).then(|| match in_use {
            true => html::button()
                .r#type("button")
                .class("mark")
                .attr("aria-haspopup", "dialog")
                .on(ev::click, move |_| shelf.viewing.set(Some(marked())))
                .child(speech::said(mark))
                .into_any(),
            false => blocks::mark(speech::said(mark)).into_any(),
        }),
        deletable.then(|| {
            html::button()
                .r#type("button")
                .class("danger")
                .on(ev::click, move |_| {
                    shelf.refused.set(None);
                    shelf.deleting.set(Some(asked.clone()));
                })
                .child(speech::said("storage-delete"))
        }),
    )
}

/// The mark that says why an upload cannot be deleted.
/// An upload in use says what it is used for. Otherwise it is in a project the account may only read.
fn not_deletable_because(uses: &[Use]) -> &'static str {
    if uses
        .iter()
        .any(|used| used.purpose == Purpose::AccountPicture)
    {
        "storage-mark-picture"
    } else if !uses.is_empty() {
        "storage-mark-in-use"
    } else {
        "storage-mark-read-only"
    }
}

/// A dialog that shows an upload bigger, as large as the window allows, and what uses it.
fn viewing(shelf: StoragePageState, viewed: Viewed) -> impl IntoView {
    let close = move || shelf.viewing.set(None);
    let whose = viewed.whose;
    let used_in = used_in(viewed.id.clone(), viewed.project, viewed.uses);

    dialog::modal(
        Signal::derive(move || whose.clone()),
        "viewing-upload",
        close,
        (
            html::img()
                .class("viewed")
                .src(service::content(&viewed.id, VIEWED))
                .alt(""),
            used_in,
            html::div().class("dialog-actions").child(
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| close())
                    .child(speech::said("storage-view-close")),
            ),
        ),
    )
}

/// Lists what uses an upload, under a heading, each a link to where it is used.
fn used_in(id: String, project: Option<String>, uses: Vec<Use>) -> impl IntoView {
    (!uses.is_empty()).then(|| {
        html::section().class("used-in").child((
            html::h3().child(speech::said("storage-used-in")),
            assets_uses::listed(id, project, uses, None),
        ))
    })
}

fn deletion(shelf: StoragePageState, upload: Upload) -> impl IntoView {
    let close = move || {
        shelf.refused.set(None);
        shelf.deleting.set(None);
    };
    let deleting = Act::new(move |id: String, _: Here| async move {
        match service::delete(&id).await {
            Ok(()) => {
                shelf.gone(&id);
                shelf.deleting.set(None);
            }
            Err(failed) => shelf.refused.set(Some(failed)),
        }
    });
    let id = upload.id;

    dialog::modal(
        Signal::derive(|| speech::say("storage-delete-heading")),
        "deleting-upload",
        close,
        (
            html::p().child(speech::said("storage-delete-hint")),
            move || shelf.refused.get().map(problem::problem),
            html::div().class("dialog-actions").child((
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| close())
                    .child(speech::said("storage-delete-cancel")),
                html::button()
                    .r#type("button")
                    .class("danger")
                    .attr("aria-busy", move || deleting.busy())
                    .on(ev::click, move |_| deleting.start(id.clone()))
                    .child(speech::said("storage-delete-confirm")),
            )),
        ),
    )
}

#[cfg(test)]
mod tests {
    use crate::assets::service::Holder;

    use super::*;

    fn used_for(purpose: Purpose) -> Use {
        Use {
            purpose,
            by: Holder::Account("account_ada".to_owned()),
        }
    }

    #[test]
    fn an_upload_that_cannot_be_deleted_says_why() {
        // expect: a picture says it is one
        assert_eq!(
            not_deletable_because(&[used_for(Purpose::Other), used_for(Purpose::AccountPicture)]),
            "storage-mark-picture"
        );
        // and: a use this client does not know says only that it is in use
        assert_eq!(
            not_deletable_because(&[used_for(Purpose::Other)]),
            "storage-mark-in-use"
        );
        // and: an upload used for nothing is in a project the account may only read
        assert_eq!(not_deletable_because(&[]), "storage-mark-read-only");
    }
}
