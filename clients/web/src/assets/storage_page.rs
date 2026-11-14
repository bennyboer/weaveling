use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::{IntoView, component, ev, html};

use crate::accounts::{CurrentSession, SignInState};
use crate::act::{Act, Here};
use crate::assets::service;
use crate::assets::service::{Purpose, Upload, UploadsSorted, Usage};
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

/// What the storage page shows and is doing.
/// `asked` counts the lists asked for, so an answer to an older one is ignored.
#[derive(Clone, Copy)]
struct Shelf {
    sorted: RwSignal<UploadsSorted>,
    uploads: RwSignal<Vec<Upload>>,
    next: RwSignal<Option<String>>,
    usage: RwSignal<Option<Usage>>,
    loading: RwSignal<bool>,
    refused: RwSignal<Option<ApiError>>,
    deleting: RwSignal<Option<Upload>>,
    asked: StoredValue<u64>,
}

impl Shelf {
    fn asked() -> Self {
        let shelf = Self {
            sorted: RwSignal::new(UploadsSorted::Newest),
            uploads: RwSignal::new(Vec::new()),
            next: RwSignal::new(None),
            usage: RwSignal::new(None),
            loading: RwSignal::new(false),
            refused: RwSignal::new(None),
            deleting: RwSignal::new(None),
            asked: StoredValue::new(0),
        };
        shelf.load(None);
        shelf.measure();

        shelf
    }

    /// Loads a page of uploads.
    /// Without a cursor it replaces the list, with one it adds the next page to it.
    fn load(self, after: Option<String>) {
        self.asked.update_value(|asked| *asked += 1);
        let asking = self.asked.get_value();
        let sorted = self.sorted.get_untracked();
        self.loading.set(true);

        spawn_local(async move {
            let told = service::uploads(sorted, after.as_deref()).await;
            if self.asked.get_value() != asking {
                return;
            }
            match told {
                Ok(page) => {
                    match after {
                        Some(_) => self.uploads.update(|shown| shown.extend(page.uploads)),
                        None => self.uploads.set(page.uploads),
                    }
                    self.next.set(page.next);
                }
                Err(failed) => self.refused.set(Some(failed)),
            }
            self.loading.set(false);
        });
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
            .update(|shown| shown.retain(|upload| upload.id != id));
        self.measure();
    }
}

#[component]
pub fn StoragePage() -> impl IntoView {
    let session = expect_context::<CurrentSession>();

    (
        shell::masthead(None),
        html::main()
            .class("operating storage")
            .child(move || match session.state() {
                SignInState::SignedIn(_) => Some(storage()),
                _ => None,
            }),
    )
}

fn storage() -> impl IntoView {
    let shelf = Shelf::asked();
    let speech = expect_context::<Speech>();

    (
        html::h1().child(speech::said("storage-heading")),
        move || shelf.usage.get().map(|usage| used(usage, speech)),
        html::p().class("hint").child(speech::said("storage-hint")),
        html::label().class("field sorting").child((
            html::span().child(speech::said("storage-sort")),
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
        )),
        move || {
            shelf
                .deleting
                .with(Option::is_none)
                .then(|| shelf.refused.get().map(problem::problem))
        },
        move || {
            let nothing = shelf.uploads.with(Vec::is_empty)
                && shelf.next.with(Option::is_none)
                && !shelf.loading.get();
            nothing.then(|| html::p().class("hint").child(speech::said("storage-none")))
        },
        html::ul().class("uploads").child(move || {
            shelf
                .uploads
                .get()
                .into_iter()
                .map(|upload| listed(shelf, upload, speech))
                .collect_view()
        }),
        move || {
            shelf.next.get().map(|next| {
                html::button()
                    .r#type("button")
                    .class("more")
                    .disabled(move || shelf.loading.get())
                    .on(ev::click, move |_| shelf.load(Some(next.clone())))
                    .child(speech::said("storage-more"))
            })
        },
        move || shelf.deleting.get().map(|upload| deletion(shelf, upload)),
    )
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

    html::div().class("usage").child((
        html::p().child(told),
        usage.most.map(|most| {
            html::meter()
                .attr("min", 0)
                .attr("max", most as f64)
                .attr("value", usage.used as f64)
                .attr("aria-label", move || speech.say("storage-heading"))
        }),
    ))
}

fn listed(shelf: Shelf, upload: Upload, speech: Speech) -> impl IntoView {
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

    html::li().child((
        html::img()
            .class("thumbnail")
            .src(service::content(&id, THUMBNAIL))
            .alt("")
            .attr("loading", "lazy"),
        html::div().class("who").child((
            html::p().class("name").child(whose),
            html::p().class("detail").child(detail),
        )),
        html::div().class("actions").child(if deletable {
            html::button()
                .r#type("button")
                .class("danger")
                .on(ev::click, move |_| {
                    shelf.refused.set(None);
                    shelf.deleting.set(Some(asked.clone()));
                })
                .child(speech::said("storage-delete"))
                .into_any()
        } else {
            html::p()
                .class("hint")
                .child(speech::said(not_deletable_because(&upload.uses)))
                .into_any()
        }),
    ))
}

/// The message that says why an upload cannot be deleted.
/// An upload in use says what it is used for. Otherwise it is in a project the account may not write in.
fn not_deletable_because(uses: &[Purpose]) -> &'static str {
    if uses.contains(&Purpose::AccountPicture) {
        "storage-in-use-as-picture"
    } else if !uses.is_empty() {
        "storage-in-use"
    } else {
        "storage-not-deletable"
    }
}

fn deletion(shelf: Shelf, upload: Upload) -> impl IntoView {
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
    use super::*;

    #[test]
    fn an_upload_that_cannot_be_deleted_says_why() {
        // expect: a picture says it is one
        assert_eq!(
            not_deletable_because(&[Purpose::Other, Purpose::AccountPicture]),
            "storage-in-use-as-picture"
        );
        // and: a use this client does not know says only that it is in use
        assert_eq!(not_deletable_because(&[Purpose::Other]), "storage-in-use");
        // and: an upload used for nothing is in a project the account may not write in
        assert_eq!(not_deletable_because(&[]), "storage-not-deletable");
    }
}
