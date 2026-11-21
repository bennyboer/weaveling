use fluent_bundle::FluentArgs;
use leptos::portal::Portal;
use leptos::prelude::*;
use leptos::{IntoView, html, view};
use leptos_router::NavigateOptions;
use leptos_router::hooks;
use readings_contract::SharedDTO;
use time::{OffsetDateTime, UtcOffset};

use crate::act::{Act, Here};
use crate::dialog;
use crate::drawer::Drawer;
use crate::feedback::column;
use crate::feedback::column::Commenting;
use crate::feedback::composer;
use crate::feedback::model::{About, CommentBlock, ReadingCopyPlace};
use crate::feedback::service as feedback;
use crate::http::ApiError;
use crate::moment;
use crate::problem;
use crate::quiet_note::QuietNote;
use crate::readings::model::{Book, Text};
use crate::readings::service;
use crate::readings::sharing;
use crate::readings::sharing::{KeptFor, OpenFor};
use crate::readings::view;
use crate::route;
use crate::speech;

/// How long a copy made for a comment is open for feedback, and kept before it is deleted with it, as when sharing one.
const OPEN_FOR_DAYS: i64 = 30;
const KEPT_FOR_DAYS: i64 = 90;

fn said_with(key: &'static str, args: &[(&'static str, String)]) -> String {
    let mut told = FluentArgs::new();
    for (name, value) in args {
        told.set(*name, value.clone());
    }

    speech::say_with(key, &told)
}

/// Once a comment is begun on the live work, asks to make a reading copy of the whole work for it,
/// as comments are kept in a copy: the live work keeps changing.
pub fn asked(slug: String, commenting: Commenting, book: RwSignal<Option<Book>>) -> impl IntoView {
    let begun = commenting.begun_on_the_live_work();

    move || {
        begun.get().map(|place| {
            let slug = slug.clone();

            view! { <Portal>{making_a_copy(slug.clone(), place.clone(), begun, book)}</Portal> }
        })
    }
}

fn making_a_copy(
    slug: String,
    place: ReadingCopyPlace,
    begun: RwSignal<Option<ReadingCopyPlace>>,
    book: RwSignal<Option<Book>>,
) -> impl IntoView {
    let now = OffsetDateTime::now_utc();
    let here = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    let (open_until, deleted_on) = sharing::times(
        now,
        here,
        OpenFor::Days(OPEN_FOR_DAYS),
        KeptFor::Days(KEPT_FOR_DAYS),
    );
    let name = RwSignal::new(said_with(
        "share-name-default",
        &[("day", moment::human_day(now))],
    ));
    let seen = match &place {
        ReadingCopyPlace::Passage { scene, .. } => {
            book.with_untracked(|held| held.as_ref().and_then(|held| held.text_of(scene)))
        }
        _ => None,
    };
    let failed = RwSignal::new(None::<ApiError>);
    let close = move || begun.set(None);
    let navigate = hooks::use_navigate();
    let note = expect_context::<QuietNote>();
    let making = Act::new({
        let place = place.clone();

        move |comment: Vec<CommentBlock>, here_still: Here| {
            let slug = slug.clone();
            let place = place.clone();
            let navigate = navigate.clone();
            let named = name.get_untracked();
            let seen = seen.clone();

            async move {
                let made = service::make_copy(
                    &route::project_id(&slug),
                    named.trim(),
                    SharedDTO::Book,
                    open_until,
                    deleted_on,
                )
                .await;
                let made = match made {
                    Ok(made) => made,
                    Err(failure) => {
                        _ = failed.try_set(Some(failure));
                        return;
                    }
                };
                let frozen = service::reading_copy(&made.id)
                    .await
                    .map(|(_, frozen)| frozen)
                    .ok();
                let (place, moved) = kept_where(place, seen.as_ref(), frozen.as_ref());
                let about = About::ReadingCopy {
                    reading_copy: made.id.clone(),
                    place,
                };
                if let Err(failure) = feedback::draft(&about, &comment).await {
                    _ = failed.try_set(Some(failure));
                    return;
                }
                if here_still.still() {
                    close();
                    Drawer::as_last_left(view::FEEDBACK_KEPT, true).open();
                    if moved {
                        note.tell(speech::say("comment-on-live-passage-changed"));
                    }
                    navigate(
                        &route::reading_copy(&slug, &made.id.to_string()),
                        NavigateOptions::default(),
                    );
                }
            }
        }
    });

    dialog::modal(
        Signal::derive(speech::said("comment-on-live")),
        "comment-on-live",
        close,
        (
            html::p().child(speech::said("comment-on-live-why")),
            html::div()
                .class("about")
                .child(column::what_about(&place, book)),
            html::label().class("field").child((
                html::span()
                    .class("label")
                    .child(speech::said("comment-on-live-name")),
                html::input()
                    .r#type("text")
                    .prop("value", move || name.get())
                    .on(leptos::ev::input, move |typed| {
                        name.set(event_target_value(&typed))
                    }),
            )),
            html::div().class("times").child((
                html::div().class("time").child((
                    html::b().child(speech::said("comment-on-live-open")),
                    html::span().child(open_until.map(|until| {
                        said_with(
                            "comment-on-live-open-until",
                            &[("day", moment::human_day(until))],
                        )
                    })),
                )),
                html::div().class("time").child((
                    html::b().child(speech::said("comment-on-live-deleted")),
                    html::span().child(deleted_on.map(|on| {
                        said_with(
                            "comment-on-live-deleted-on",
                            &[("day", moment::human_day(on))],
                        )
                    })),
                )),
            )),
            html::p()
                .class("hint")
                .child(speech::said("comment-on-live-later")),
            move || failed.get().map(problem::problem),
            composer::composer(
                Vec::new(),
                "comment-on-live-make",
                Signal::derive(move || making.running()),
                move |comment| making.start(comment),
                close,
            ),
        ),
    )
}

/// Where a comment on the live work is kept in the copy made for it: where it was begun,
/// unless the scene of a passage changed between its selecting and the copy freezing it, so its words may lie elsewhere;
/// then it is kept on the scene, and whether it moved is told, so the passage can be selected again.
fn kept_where(
    place: ReadingCopyPlace,
    seen: Option<&Text>,
    frozen: Option<&Book>,
) -> (ReadingCopyPlace, bool) {
    let ReadingCopyPlace::Passage { scene, .. } = &place else {
        return (place, false);
    };
    let unchanged = seen
        .zip(frozen.and_then(|frozen| frozen.text_of(scene)))
        .is_some_and(|(seen, frozen)| seen.says_as(&frozen));
    if unchanged {
        return (place, false);
    }
    let scene = scene.clone();

    (ReadingCopyPlace::Scene(scene), true)
}
