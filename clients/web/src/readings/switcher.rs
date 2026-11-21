use std::collections::HashMap;

use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::{IntoView, ev, html};
use wasm_bindgen::JsCast;
use web_sys::Node;

use crate::feedback;
use crate::feedback::model::FeedbackCount;
use crate::members::roles::Roles;
use crate::moment;
use crate::readings::copy_tools;
use crate::readings::reading_copies::{Extent, ListedReadingCopy, ReadingCopyId};
use crate::readings::share_dialog;
use crate::route;
use crate::speech;

const PANEL: &str = "what-to-read";

fn said_with(key: &'static str, args: &[(&'static str, String)]) -> String {
    let mut told = FluentArgs::new();
    for (name, value) in args {
        told.set(*name, value.clone());
    }

    speech::say_with(key, &told)
}

fn counted(key: &'static str, count: u32) -> String {
    let mut told = FluentArgs::new();
    told.set("count", count);

    speech::say_with(key, &told)
}

fn extent(extent: Extent) -> String {
    match extent {
        Extent::Book => speech::say("copy-extent-book"),
        Extent::Sections(count) => counted("copy-extent-sections", count),
        Extent::Scenes(count) => counted("copy-extent-scenes", count),
    }
}

fn day(moment: time::OffsetDateTime) -> String {
    moment::human_day(moment)
}

/// The bar above the text: the switcher, which says what is read and when, and the copy's menu for whoever may change it.
pub fn reading_bar(
    slug: String,
    reading: Option<ReadingCopyId>,
    copies: LocalResource<Vec<ListedReadingCopy>>,
    chosen: RwSignal<Option<ListedReadingCopy>>,
) -> impl IntoView {
    let sharing = RwSignal::new(false);
    let shared_from = slug.clone();
    let changed_from = slug.clone();
    let may_change =
        Memo::new(move |_| chosen.with(|held| held.as_ref().is_some_and(|copy| copy.may_change)));
    let tools = move || {
        may_change
            .get()
            .then(|| copy_tools::copy_tools(changed_from.clone(), chosen, copies))
    };

    html::div().class("reading-bar").child((
        html::div()
            .class("what-is-read")
            .child((switcher(slug, reading, copies, chosen, sharing), tools)),
        move || {
            sharing.get().then(|| {
                share_dialog::share_dialog(shared_from.clone(), move || sharing.set(false))
            })
        },
    ))
}

/// Closes what `open` shows once a click lands anywhere outside `holder`.
pub(crate) fn closed_when_clicked_away(holder: NodeRef<html::Div>, open: RwSignal<bool>) {
    let clicking_away = window_event_listener(ev::click, move |event| {
        let inside = holder
            .get_untracked()
            .zip(
                event
                    .target()
                    .and_then(|target| target.dyn_into::<Node>().ok()),
            )
            .is_some_and(|(held, clicked)| held.contains(Some(&clicked)));
        if !inside {
            _ = open.try_set(false);
        }
    });
    on_cleanup(move || clicking_away.remove());
}

/// The mark of the live work: a dot and the word, as it follows the authors' changes.
fn live_mark() -> impl IntoView {
    html::span().class("chip live").child((
        html::span().class("dot").attr("aria-hidden", "true"),
        speech::said("reading-live-mark"),
    ))
}

fn switcher(
    slug: String,
    reading: Option<ReadingCopyId>,
    copies: LocalResource<Vec<ListedReadingCopy>>,
    chosen: RwSignal<Option<ListedReadingCopy>>,
    sharing: RwSignal<bool>,
) -> impl IntoView {
    let open = RwSignal::new(false);
    let roles = expect_context::<Roles>();
    let may_share = {
        let slug = slug.clone();

        Memo::new(move |_| roles.may_write(&slug))
    };
    let button: NodeRef<html::Button> = NodeRef::new();
    let reading_live = reading.is_none();
    let name = move || {
        if reading_live {
            speech::say("reading-live")
        } else {
            chosen
                .with(|held| held.as_ref().map(|copy| copy.name.clone()))
                .unwrap_or_default()
        }
    };
    let close = move || {
        open.set(false);
        if let Some(button) = button.get_untracked() {
            _ = button.focus();
        }
    };

    let holder: NodeRef<html::Div> = NodeRef::new();
    closed_when_clicked_away(holder, open);

    html::div().class("switching").node_ref(holder).child((
        html::button()
            .node_ref(button)
            .r#type("button")
            .class("switcher")
            .attr("aria-haspopup", "dialog")
            .attr("aria-controls", PANEL)
            .attr("aria-expanded", move || open.get().to_string())
            .on(ev::click, move |_| open.update(|shown| *shown = !*shown))
            .child((
                html::small().child(speech::said("reading-reading")),
                html::b().child(name),
                html::span().class("when").child(move || {
                    if reading_live {
                        Some(live_mark().into_any())
                    } else {
                        chosen.get().map(|copy| copy_line(&copy).into_any())
                    }
                }),
            )),
        move || {
            open.get().then(|| {
                let slug = slug.clone();
                let reading = reading.clone();
                let counts = LocalResource::new(move || {
                    let listed: Vec<ReadingCopyId> = copies
                        .get()
                        .map(|listed| listed.iter().map(|copy| copy.id.clone()).collect())
                        .unwrap_or_default();

                    async move {
                        feedback::service::counted_on_reading_copies(&listed)
                            .await
                            .unwrap_or_default()
                    }
                });

                html::div()
                    .id(PANEL)
                    .class("what-to-read")
                    .role("dialog")
                    .attr("aria-label", speech::said("reading-what-to-read"))
                    .on(ev::keydown, move |pressed| {
                        if pressed.key() == "Escape" {
                            close();
                        }
                    })
                    .child((
                        live_entry(&slug, reading.is_none()),
                        move || {
                            copies.get().map(|listed| {
                                copy_entries(
                                    &slug,
                                    reading.as_ref(),
                                    listed.to_vec(),
                                    counts.get().unwrap_or_default(),
                                )
                            })
                        },
                        move || {
                            may_share.get().then(|| {
                                html::button()
                                    .r#type("button")
                                    .class("share")
                                    .on(ev::click, move |_| {
                                        open.set(false);
                                        sharing.set(true);
                                    })
                                    .child(speech::said("share-copy"))
                            })
                        },
                    ))
            })
        },
    ))
}

fn live_entry(slug: &str, here: bool) -> impl IntoView + use<> {
    html::a()
        .href(route::reading(slug))
        .class(if here { "entry here" } else { "entry" })
        .attr("aria-current", here.then_some("true"))
        .child((
            html::span()
                .class("name")
                .child(speech::said("reading-live")),
            html::span().class("chips").child(live_mark()),
        ))
}

fn copy_entries(
    slug: &str,
    reading: Option<&ReadingCopyId>,
    listed: Vec<ListedReadingCopy>,
    counts: HashMap<ReadingCopyId, FeedbackCount>,
) -> impl IntoView + use<> {
    let count = u32::try_from(listed.len()).unwrap_or(u32::MAX);
    let entries = listed
        .into_iter()
        .map(|copy| {
            let here = reading == Some(&copy.id);
            let counted = counts.get(&copy.id).copied().unwrap_or_default();

            html::a()
                .href(route::reading_copy(slug, &copy.id.to_string()))
                .class(if here { "entry here" } else { "entry" })
                .attr("aria-current", here.then_some("true"))
                .child((
                    html::span().class("name").child(copy.name.clone()),
                    html::span().class("meta").child(format!(
                        "{} · {}",
                        maker(&copy),
                        extent(copy.extent)
                    )),
                    html::span()
                        .class("chips")
                        .child((feedback_chips(counted), chips(&copy))),
                ))
        })
        .collect::<Vec<_>>();

    (
        html::h2().child(counted("reading-copies", count)),
        (count == 0).then(|| {
            html::p()
                .class("none")
                .child(speech::said("reading-no-copies"))
        }),
        entries,
    )
}

fn maker(copy: &ListedReadingCopy) -> String {
    if copy.made_by_you {
        speech::say("copy-by-you")
    } else {
        copy.maker
            .clone()
            .unwrap_or_else(|| speech::say("copy-by-a-former-member"))
    }
}

/// How much of a copy's feedback is not done yet, and how many drafts of yours wait on it, where there is any.
fn feedback_chips(held: FeedbackCount) -> impl IntoView {
    (
        (held.not_done > 0).then(|| {
            html::span()
                .class("chip not-done")
                .child(counted("copy-not-done", held.not_done))
        }),
        (held.your_drafts > 0).then(|| {
            html::span()
                .class("chip drafts")
                .child(counted("copy-your-drafts", held.your_drafts))
        }),
    )
}

fn chips(copy: &ListedReadingCopy) -> Vec<impl IntoView + use<>> {
    let open = match (copy.open, copy.open_until) {
        (true, Some(until)) => (
            "chip open",
            said_with("copy-open-until", &[("day", day(until))]),
        ),
        (true, None) => ("chip open", speech::say("copy-open-no-end")),
        (false, until) => (
            "chip closed",
            said_with(
                "copy-closed-since",
                &[("day", until.map(day).unwrap_or_default())],
            ),
        ),
    };
    let kept = match copy.deleted_on {
        Some(on) => said_with("copy-deleted-on", &[("day", day(on))]),
        None => speech::say("copy-kept"),
    };

    vec![
        html::span().class(open.0).child(open.1),
        html::span().class("chip").child(kept),
    ]
}

/// When a copy was frozen, until when it is open, and when it is deleted, in one quiet line.
fn copy_line(copy: &ListedReadingCopy) -> String {
    let frozen = said_with("copy-line-frozen", &[("day", day(copy.made_at))]);
    let open = match (copy.open, copy.open_until) {
        (true, Some(until)) => said_with("copy-line-open-until", &[("day", day(until))]),
        (true, None) => speech::say("copy-line-open-no-end"),
        (false, until) => said_with(
            "copy-line-closed",
            &[("day", until.map(day).unwrap_or_default())],
        ),
    };
    let kept = match copy.deleted_on {
        Some(on) => said_with("copy-line-deleted", &[("day", day(on))]),
        None => speech::say("copy-line-kept"),
    };

    [frozen, open, kept].join(" · ")
}
