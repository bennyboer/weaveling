use leptos::prelude::*;
use leptos::{IntoView, ev, html};
use leptos_router::NavigateOptions;
use leptos_router::hooks;
use time::{Duration, OffsetDateTime, UtcOffset};

use crate::act::{Act, Here};
use crate::dialog;
use crate::http::ApiError;
use crate::icons;
use crate::icons::Icon;
use crate::moment;
use crate::problem;
use crate::readings::reading_copies::ListedReadingCopy;
use crate::readings::service;
use crate::readings::share_dialog;
use crate::readings::sharing;
use crate::readings::sharing::Extending;
use crate::readings::switcher;
use crate::route;
use crate::speech;

const MENU: &str = "copy-tools";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Asking {
    Rename,
    Extend,
    Delete,
}

/// What may be done with a copy, for whoever made it or an owner: renaming it, extending its times, or deleting it.
pub fn copy_tools(
    slug: String,
    chosen: RwSignal<Option<ListedReadingCopy>>,
    copies: LocalResource<Vec<ListedReadingCopy>>,
) -> impl IntoView {
    let open = RwSignal::new(false);
    let asking = RwSignal::new(None::<Asking>);
    let button: NodeRef<html::Button> = NodeRef::new();
    let close = move || {
        open.set(false);
        if let Some(button) = button.get_untracked() {
            _ = button.focus();
        }
    };
    let ask = move |what: Asking| {
        open.set(false);
        asking.set(Some(what));
    };

    let holder: NodeRef<html::Div> = NodeRef::new();
    switcher::closed_when_clicked_away(holder, open);

    html::div().class("copy-tools").node_ref(holder).child((
        html::button()
            .node_ref(button)
            .r#type("button")
            .class("tools-button")
            .attr("aria-label", speech::said("copy-tools"))
            .attr("title", speech::said("copy-tools"))
            .attr("aria-haspopup", "menu")
            .attr("aria-controls", MENU)
            .attr("aria-expanded", move || open.get().to_string())
            .on(ev::click, move |_| open.update(|shown| *shown = !*shown))
            .child(icons::mark(Icon::More)),
        move || {
            open.get().then(|| {
                html::div()
                    .id(MENU)
                    .class("tools-menu")
                    .role("menu")
                    .on(ev::keydown, move |pressed| {
                        if pressed.key() == "Escape" {
                            close();
                        }
                    })
                    .child((
                        html::button()
                            .r#type("button")
                            .role("menuitem")
                            .on(ev::click, move |_| ask(Asking::Rename))
                            .child(speech::said("copy-rename")),
                        html::button()
                            .r#type("button")
                            .role("menuitem")
                            .on(ev::click, move |_| ask(Asking::Extend))
                            .child(speech::said("copy-extend")),
                        html::button()
                            .r#type("button")
                            .role("menuitem")
                            .class("danger")
                            .on(ev::click, move |_| ask(Asking::Delete))
                            .child(speech::said("copy-delete")),
                    ))
            })
        },
        move || {
            let done = move || asking.set(None);
            match asking.get() {
                Some(Asking::Rename) => Some(rename_dialog(chosen, copies, done).into_any()),
                Some(Asking::Extend) => Some(extend_dialog(chosen, copies, done).into_any()),
                Some(Asking::Delete) => Some(delete_dialog(slug.clone(), chosen, done).into_any()),
                None => None,
            }
        },
    ))
}

fn rename_dialog(
    chosen: RwSignal<Option<ListedReadingCopy>>,
    copies: LocalResource<Vec<ListedReadingCopy>>,
    close: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let name = RwSignal::new(
        chosen
            .with_untracked(|held| held.as_ref().map(|copy| copy.name.clone()))
            .unwrap_or_default(),
    );
    let failed = RwSignal::new(None::<ApiError>);
    let renaming = Act::new(move |(), here: Here| {
        let copy = chosen.with_untracked(|held| held.as_ref().map(|copy| copy.id.clone()));
        let named = name.get_untracked();

        async move {
            let Some(copy) = copy else {
                return;
            };
            match service::rename_copy(&copy, named.trim()).await {
                Ok(renamed) => {
                    if here.still() {
                        _ = chosen.try_set(Some(renamed));
                        copies.refetch();
                        close();
                    }
                }
                Err(failure) => _ = failed.try_set(Some(failure)),
            }
        }
    });

    dialog::modal(
        Signal::derive(speech::said("copy-rename-title")),
        "copy-rename",
        close,
        (
            html::label().class("field").child((
                html::span()
                    .class("label")
                    .child(speech::said("share-name")),
                html::input()
                    .r#type("text")
                    .prop("value", move || name.get())
                    .on(ev::input, move |event| name.set(event_target_value(&event)))
                    .on(ev::keydown, move |event| {
                        if event.key() == "Enter" && !name.with(|named| named.trim().is_empty()) {
                            renaming.start(());
                        }
                    }),
            )),
            move || failed.get().map(problem::problem),
            html::div().class("dialog-actions").child((
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| close())
                    .child(speech::said("share-cancel")),
                html::button()
                    .r#type("button")
                    .class("primary")
                    .disabled(move || name.with(|named| named.trim().is_empty()))
                    .attr("aria-busy", move || renaming.busy())
                    .on(ev::click, move |_| renaming.start(()))
                    .child(speech::said("copy-rename-confirm")),
            )),
        ),
    )
}

fn delete_dialog(
    slug: String,
    chosen: RwSignal<Option<ListedReadingCopy>>,
    close: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let failed = RwSignal::new(None::<ApiError>);
    let navigate = hooks::use_navigate();
    let named = chosen
        .with_untracked(|held| held.as_ref().map(|copy| copy.name.clone()))
        .unwrap_or_default();
    let deleting = Act::new(move |(), here: Here| {
        let copy = chosen.with_untracked(|held| held.as_ref().map(|copy| copy.id.clone()));
        let slug = slug.clone();
        let navigate = navigate.clone();

        async move {
            let Some(copy) = copy else {
                return;
            };
            match service::delete_copy(&copy).await {
                Ok(()) => {
                    if here.still() {
                        close();
                        navigate(&route::reading(&slug), NavigateOptions::default());
                    }
                }
                Err(failure) => _ = failed.try_set(Some(failure)),
            }
        }
    });

    dialog::modal(
        Signal::derive(speech::said("copy-delete-title")),
        "copy-delete",
        close,
        (
            html::p().child(speech::say_about("copy-delete-told", "name", &named)),
            move || failed.get().map(problem::problem),
            html::div().class("dialog-actions").child((
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| close())
                    .child(speech::said("share-cancel")),
                html::button()
                    .r#type("button")
                    .class("danger")
                    .attr("aria-busy", move || deleting.busy())
                    .on(ev::click, move |_| deleting.start(()))
                    .child(speech::said("copy-delete-confirm")),
            )),
        ),
    )
}

fn extend_dialog(
    chosen: RwSignal<Option<ListedReadingCopy>>,
    copies: LocalResource<Vec<ListedReadingCopy>>,
    close: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let now = OffsetDateTime::now_utc();
    let here = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    let stands = chosen
        .with_untracked(|held| held.as_ref().map(|copy| (copy.open_until, copy.deleted_on)))
        .unwrap_or((None, None));
    let open = RwSignal::new(Extending::AsItIs);
    let kept = RwSignal::new(Extending::AsItIs);
    let moved = Memo::new(move |_| sharing::extended(now, here, stands, open.get(), kept.get()));
    let failed = RwSignal::new(None::<ApiError>);
    let extending = Act::new(move |(), here_still: Here| {
        let copy = chosen.with_untracked(|held| held.as_ref().map(|copy| copy.id.clone()));
        let (open_until, deleted_on) = moved.get_untracked();

        async move {
            let Some(copy) = copy else {
                return;
            };
            match service::extend_copy(&copy, open_until, deleted_on).await {
                Ok(extended) => {
                    if here_still.still() {
                        _ = chosen.try_set(Some(extended));
                        copies.refetch();
                        close();
                    }
                }
                Err(failure) => _ = failed.try_set(Some(failure)),
            }
        }
    });
    let later_day = |stands: Option<OffsetDateTime>, days: i64| {
        stands
            .map_or(now, |stands| stands.max(now))
            .to_offset(here)
            .date()
            + Duration::days(days)
    };

    dialog::modal(
        Signal::derive(speech::said("copy-extend-title")),
        "copy-extend share-copy",
        close,
        (
            html::div().class("field").child((
                html::span()
                    .class("label")
                    .child(speech::said("share-open")),
                stands.0.map(|_| {
                    share_dialog::chosen_among(
                        "share-open",
                        open,
                        vec![
                            ("extend-as-is", Extending::AsItIs),
                            ("extend-7", Extending::Days(7)),
                            ("extend-30", Extending::Days(30)),
                            ("share-open-no-end", Extending::NoEnd),
                            (
                                "share-open-until",
                                Extending::Until(later_day(stands.0, 30)),
                            ),
                        ],
                        same_choice,
                    )
                }),
                share_dialog::date_field(
                    "share-open-on",
                    move || match open.get() {
                        Extending::Until(day) => Some(day),
                        _ => None,
                    },
                    move |day| open.set(Extending::Until(day)),
                ),
                html::p().class("hint").child(move || match moved.get().0 {
                    Some(until) => share_dialog::said_with(
                        "share-open-hint",
                        &[("day", moment::human_day(until))],
                    ),
                    None => speech::say("share-open-hint-no-end"),
                }),
            )),
            html::div().class("field").child((
                html::span()
                    .class("label")
                    .child(speech::said("share-kept")),
                stands.1.map(|_| {
                    share_dialog::chosen_among(
                        "share-kept",
                        kept,
                        vec![
                            ("extend-as-is", Extending::AsItIs),
                            ("extend-90", Extending::Days(90)),
                            ("extend-365", Extending::Days(365)),
                            ("share-kept-ever", Extending::NoEnd),
                            (
                                "share-kept-until",
                                Extending::Until(later_day(stands.1, 90)),
                            ),
                        ],
                        same_choice,
                    )
                }),
                share_dialog::date_field(
                    "share-kept-on",
                    move || match kept.get() {
                        Extending::Until(day) => Some(day),
                        _ => None,
                    },
                    move |day| kept.set(Extending::Until(day)),
                ),
                html::p().class("hint").child(move || match moved.get().1 {
                    Some(on) => share_dialog::said_with(
                        "share-kept-hint",
                        &[("day", moment::human_day(on))],
                    ),
                    None => speech::say("share-kept-hint-ever"),
                }),
            )),
            move || failed.get().map(problem::problem),
            html::div().class("dialog-actions").child((
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| close())
                    .child(speech::said("share-cancel")),
                html::button()
                    .r#type("button")
                    .class("primary")
                    .disabled(move || moved.get() == stands)
                    .attr("aria-busy", move || extending.busy())
                    .on(ev::click, move |_| extending.start(()))
                    .child(speech::said("copy-extend-confirm")),
            )),
        ),
    )
}

/// Whether a choice is the one shown pressed.
/// A day of one's own counts as one choice, whichever day it is.
fn same_choice(chosen: Extending, option: Extending) -> bool {
    match (chosen, option) {
        (Extending::Until(_), Extending::Until(_)) => true,
        (chosen, option) => chosen == option,
    }
}
