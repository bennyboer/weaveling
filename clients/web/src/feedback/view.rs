use std::collections::HashMap;

use feedback_contract::{order, status};
use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::feedback::column;
use crate::feedback::model::{About, Feedback, FeedbackId, ReadingCopyPlace};
use crate::feedback::service;
use crate::feedback::service::Asked;
use crate::icons;
use crate::icons::Icon;
use crate::members::roles::Roles;
use crate::outline::model::SectionId;
use crate::projects::model::ProjectId;
use crate::readings::model::Book;
use crate::readings::reading_copies::{ListedReadingCopy, ReadingCopyId};
use crate::readings::service as readings;
use crate::route;
use crate::scenes::model::SceneId;
use crate::speech;

/// How many pieces a page of the feedback view holds.
const A_PAGE: usize = 50;

/// What the feedback shown is about: anything, or one thing, a copy, a section with everything in it, or a scene.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Narrowed {
    Anything,
    ReadingCopy(ReadingCopyId),
    Section(SectionId),
    Scene(SceneId),
}

impl Narrowed {
    fn value(&self) -> String {
        match self {
            Self::Anything => String::new(),
            Self::ReadingCopy(copy) => format!("copy:{copy}"),
            Self::Section(section) => format!("section:{section}"),
            Self::Scene(scene) => format!("scene:{scene}"),
        }
    }

    fn read(value: &str) -> Self {
        match value.split_once(':') {
            Some(("copy", copy)) => Self::ReadingCopy(ReadingCopyId::from(copy.to_owned())),
            Some(("section", section)) => Self::Section(SectionId::from(section.to_owned())),
            Some(("scene", scene)) => Self::Scene(SceneId::from(scene.to_owned())),
            _ => Self::Anything,
        }
    }

    /// Which of the project's feedback is paged through: a section with the sections under it and every scene in them.
    fn asked(&self, project: &ProjectId, book: Option<&Book>) -> Asked {
        let in_project = Asked::in_project(project);
        match self {
            Self::Anything => in_project,
            Self::ReadingCopy(copy) => Asked {
                reading_copies: vec![copy.clone()],
                ..in_project
            },
            Self::Scene(scene) => Asked {
                scenes: vec![scene.clone()],
                ..in_project
            },
            Self::Section(section) => {
                let (scenes, sections) = held_in(section, book);
                Asked {
                    scenes,
                    sections,
                    ..in_project
                }
            }
        }
    }
}

/// A section, the sections under it, and every scene in them, as the book reads now.
fn held_in(section: &SectionId, book: Option<&Book>) -> (Vec<SceneId>, Vec<SectionId>) {
    let mut scenes = Vec::new();
    let mut sections = vec![section.clone()];
    let Some(book) = book else {
        return (scenes, sections);
    };
    let mut outlined = book
        .sections
        .iter()
        .skip_while(|outlined| &outlined.section != section);
    let Some(top) = outlined.next() else {
        return (scenes, sections);
    };
    scenes.extend(top.scenes.iter().cloned());
    for under in outlined.take_while(|outlined| outlined.depth > top.depth) {
        sections.push(under.section.clone());
        scenes.extend(under.scenes.iter().cloned());
    }

    (scenes, sections)
}

/// Which feedback the view shows: what is still to be worked through, what is done, or all of it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Showing {
    Pending,
    Done,
    All,
}

impl Showing {
    fn status(self) -> Option<&'static str> {
        match self {
            Self::Pending => Some(status::PENDING),
            Self::Done => Some(status::DONE),
            Self::All => None,
        }
    }
}

/// The feedback view: every piece submitted in the project, in every copy, newest first unless asked,
/// what is pending unless asked, a page at a time; each with who gave it, when, in which copy and on what,
/// marked done there by whoever may write, and opened at its place in its copy.
pub fn feedback_view(project: String) -> impl IntoView {
    let id = route::project_id(&project);
    let showing = RwSignal::new(Showing::Pending);
    let newest_first = RwSignal::new(true);
    let narrowed = RwSignal::new(Narrowed::Anything);
    let book = RwSignal::new(None::<Book>);
    let pieces = RwSignal::new(Vec::<Feedback>::new());
    let more = RwSignal::new(false);
    let loaded = RwSignal::new(false);
    let none_at_all = RwSignal::new(false);
    let paging = Action::new_local({
        let id = id.clone();

        move |going_on: &bool| {
            let id = id.clone();
            let after = going_on
                .then(|| {
                    pieces.with_untracked(|pieces| pieces.last().map(|piece| piece.id.clone()))
                })
                .flatten();
            let status = showing.get_untracked().status();
            let asked = book.with_untracked(|book| {
                narrowed.with_untracked(|narrowed| narrowed.asked(&id, book.as_ref()))
            });
            let order = if newest_first.get_untracked() {
                order::NEWEST
            } else {
                order::OLDEST
            };
            let going_on = *going_on;

            async move {
                let Ok(found) =
                    service::page_in(&asked, status, order, after.as_ref(), A_PAGE).await
                else {
                    return;
                };
                let nothing_shown = !going_on && found.is_empty();
                let everything = Asked::in_project(&id);
                let none_given = nothing_shown
                    && ((status.is_none() && asked == everything)
                        || service::page_in(&everything, None, order, None, 1)
                            .await
                            .is_ok_and(|any| any.is_empty()));
                _ = none_at_all.try_set(none_given);
                _ = more.try_set(found.len() == A_PAGE);
                _ = pieces.try_update(|pieces| {
                    if !going_on {
                        pieces.clear();
                    }
                    pieces.extend(found);
                });
                _ = loaded.try_set(true);
            }
        }
    });
    Effect::new(move |_| {
        showing.track();
        newest_first.track();
        narrowed.track();
        paging.dispatch(false);
    });
    let copies = LocalResource::new({
        let id = id.clone();

        move || {
            let id = id.clone();

            async move {
                readings::copies(&id)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|copy| (copy.id.clone(), copy))
                    .collect::<HashMap<ReadingCopyId, ListedReadingCopy>>()
            }
        }
    });
    {
        let id = id.clone();
        leptos::task::spawn_local(async move {
            if let Ok(read) = readings::live_book(&id).await {
                _ = book.try_set(Some(read));
            }
        });
    }
    let slug = project.clone();
    let marking = {
        let roles = expect_context::<Roles>();
        let project = project.clone();

        Signal::derive(move || roles.may_write(&project))
    };
    let pending = LocalResource::new({
        let id = id.clone();

        move || {
            pieces.track();
            let id = id.clone();
            let narrowed = narrowed.get();
            let held = match &narrowed {
                Narrowed::Section(section) => book.with(|book| held_in(section, book.as_ref())),
                _ => (Vec::new(), Vec::new()),
            };

            async move {
                match narrowed {
                    Narrowed::ReadingCopy(copy) => {
                        service::counted_on_reading_copies(std::slice::from_ref(&copy))
                            .await
                            .ok()
                            .and_then(|counted| counted.get(&copy).map(|count| count.not_done))
                            .unwrap_or(0)
                    }
                    narrowed => service::pending_in(&id)
                        .await
                        .map(|pending| match narrowed {
                            Narrowed::Scene(scene) => {
                                pending.scenes.get(&scene).copied().unwrap_or(0)
                            }
                            Narrowed::Section(_) => {
                                let (scenes, sections) = &held;
                                scenes
                                    .iter()
                                    .filter_map(|scene| pending.scenes.get(scene))
                                    .chain(
                                        sections
                                            .iter()
                                            .filter_map(|section| pending.sections.get(section)),
                                    )
                                    .sum()
                            }
                            _ => pending.total,
                        })
                        .unwrap_or(0),
                }
            }
        }
    });
    let choice = move |shown: Showing, said: &'static str| {
        html::button()
            .r#type("button")
            .class(("here", move || showing.get() == shown))
            .attr("aria-pressed", move || (showing.get() == shown).to_string())
            .on(ev::click, move |_| showing.set(shown))
            .child(move || match (shown, pending.get()) {
                (Showing::Pending, Some(count)) if count > 0 => {
                    format!("{} · {count}", speech::say(said))
                }
                _ => speech::say(said),
            })
    };

    html::div().class("column feedback-view").child((
        html::h1().child(speech::said("view-feedback")),
        html::div().class("feedback-view-bar").child((
            html::div()
                .class("showing")
                .role("group")
                .attr("aria-label", speech::said("feedback-showing"))
                .child((
                    choice(Showing::Pending, "feedback-filter-pending"),
                    choice(Showing::Done, "feedback-filter-done"),
                    choice(Showing::All, "feedback-filter-all"),
                )),
            html::div().class("narrowing").child((
                about_picker(narrowed, copies, book),
                html::button()
                    .r#type("button")
                    .class("ordering")
                    .on(ev::click, move |_| {
                        newest_first.update(|newest| *newest = !*newest)
                    })
                    .child(move || {
                        speech::say(if newest_first.get() {
                            "feedback-newest-first"
                        } else {
                            "feedback-oldest-first"
                        })
                    }),
            )),
        )),
        move || {
            let nothing = loaded.get() && pieces.with(Vec::is_empty);

            nothing.then(|| none_here(showing.get(), none_at_all.get(), &project))
        },
        html::div()
            .class("feedback-list")
            .child(For(ForProps::builder()
                .each(move || pieces.get())
                .key(Clone::clone)
                .children(move |piece| {
                    a_piece(piece, pieces, showing, copies, book, marking, slug.clone())
                })
                .build())),
        move || {
            more.get().then(|| {
                html::button()
                    .r#type("button")
                    .class("show-more")
                    .disabled(move || paging.pending().get())
                    .on(ev::click, move |_| _ = paging.dispatch(true))
                    .child(speech::said("feedback-show-more"))
            })
        },
    ))
}

/// The picker narrowing the feedback to what is about one thing: a copy, or a section or scene of the book as it reads now.
fn about_picker(
    narrowed: RwSignal<Narrowed>,
    copies: LocalResource<HashMap<ReadingCopyId, ListedReadingCopy>>,
    book: RwSignal<Option<Book>>,
) -> impl IntoView {
    let option = move |value: Narrowed, said: String| {
        let chosen = value.clone();

        html::option()
            .value(value.value())
            .selected(move || narrowed.with(|narrowed| narrowed == &chosen))
            .child(said)
    };

    html::label().class("about-picker").child((
        html::span().child(speech::said("feedback-about")),
        html::select()
            .on(ev::change, move |event| {
                narrowed.set(Narrowed::read(&event_target_value(&event)))
            })
            .child((
                option(Narrowed::Anything, speech::say("feedback-about-anything")),
                move || {
                    let mut listed: Vec<ListedReadingCopy> = copies
                        .get()
                        .map(|copies| copies.into_values().collect())
                        .unwrap_or_default();
                    listed.sort_by(|one, other| one.name.cmp(&other.name));

                    (!listed.is_empty()).then(|| {
                        html::optgroup()
                            .label(speech::say("feedback-about-copies"))
                            .child(
                                listed
                                    .into_iter()
                                    .map(|copy| option(Narrowed::ReadingCopy(copy.id), copy.name))
                                    .collect::<Vec<_>>(),
                            )
                    })
                },
                move || {
                    book.with(|book| {
                        book.iter()
                            .flat_map(|book| &book.sections)
                            .map(|outlined| {
                                let title = if outlined.title.trim().is_empty() {
                                    speech::say("heading-untitled")
                                } else {
                                    outlined.title.clone()
                                };

                                html::optgroup().label(title).child((
                                    option(
                                        Narrowed::Section(outlined.section.clone()),
                                        speech::say("feedback-about-section"),
                                    ),
                                    outlined
                                        .scenes
                                        .iter()
                                        .enumerate()
                                        .map(|(at, scene)| {
                                            option(
                                                Narrowed::Scene(scene.clone()),
                                                speech::say_about(
                                                    "feedback-about-scene",
                                                    "number",
                                                    &(at + 1).to_string(),
                                                ),
                                            )
                                        })
                                        .collect::<Vec<_>>(),
                                ))
                            })
                            .collect::<Vec<_>>()
                    })
                },
            )),
    ))
}

/// What the view says when it shows nothing: none given at all yet, and how to get some; or nothing left to do; or nothing done yet.
fn none_here(showing: Showing, none_at_all: bool, project: &str) -> AnyView {
    match showing {
        _ if none_at_all => html::div()
            .class("nothing-yet")
            .child((
                html::p().child(speech::said("feedback-none-yet")),
                html::a()
                    .href(route::reading(project))
                    .child(speech::said("feedback-to-reading")),
            ))
            .into_any(),
        Showing::Pending | Showing::All => html::p()
            .class("nothing-yet")
            .child(speech::said("feedback-none-not-done"))
            .into_any(),
        Showing::Done => html::p()
            .class("nothing-yet")
            .child(speech::said("feedback-none-done"))
            .into_any(),
    }
}

#[allow(clippy::too_many_arguments)]
fn a_piece(
    piece: Feedback,
    pieces: RwSignal<Vec<Feedback>>,
    showing: RwSignal<Showing>,
    copies: LocalResource<HashMap<ReadingCopyId, ListedReadingCopy>>,
    book: RwSignal<Option<Book>>,
    marking: Signal<bool>,
    project: String,
) -> impl IntoView {
    let id: FeedbackId = piece.id.clone();
    let done = piece.done;
    let shown_done = RwSignal::new(done);
    let leaving = RwSignal::new(false);
    let marking_done = Action::new_local(move |marked_done: &bool| {
        let id = id.clone();
        let marked_done = *marked_done;
        shown_done.set(marked_done);

        async move {
            match service::mark_done(&id, marked_done).await {
                Ok(marked) => {
                    let goes = showing.get_untracked() != Showing::All;
                    if goes {
                        _ = set_timeout(
                            move || _ = leaving.try_set(true),
                            column::SETTLED_BEFORE_LEAVING,
                        );
                    }
                    _ = set_timeout(
                        move || {
                            _ = pieces.try_update(|pieces| {
                                if goes {
                                    pieces.retain(|piece| piece.id != id);
                                } else if let Some(held) =
                                    pieces.iter_mut().find(|piece| piece.id == id)
                                {
                                    *held = marked;
                                }
                            });
                        },
                        column::SETTLED_BEFORE_LEAVING + column::LEAVING_FOR,
                    );
                }
                Err(_) => shown_done.set(done),
            }
        }
    });
    let (about, said_on, opened) = told_of(&piece, &project, copies, book);

    html::article()
        .class("note")
        .class(("leaving", move || leaving.get()))
        .child((
            html::div()
                .class("meta")
                .child((column::who(&piece), about)),
            said_on,
            html::div()
                .class("comment")
                .child(column::comment(&piece.comment)),
            html::div().class("note-foot").child((
                opened,
                column::done_mark(done, shown_done, marking_done, marking),
            )),
        ))
}

/// What a piece is about, told as every kind of subject tells it, so the view itself knows none of them:
/// the line naming the kind, the thing and where in it; what in it was said on, as a quote of the text; and where it opens.
fn told_of(
    piece: &Feedback,
    project: &str,
    copies: LocalResource<HashMap<ReadingCopyId, ListedReadingCopy>>,
    book: RwSignal<Option<Book>>,
) -> (AnyView, AnyView, AnyView) {
    match &piece.about {
        About::ReadingCopy {
            reading_copy,
            place,
        } => {
            let copy = reading_copy.clone();
            let named = {
                let place = place.clone();

                move || named_in_the_book(&place, book)
            };
            let about = html::div()
                .class("about-line")
                .child((
                    html::span()
                        .class("kind")
                        .attr("role", "img")
                        .attr("aria-label", speech::said("feedback-kind-text"))
                        .attr("title", speech::said("feedback-kind-text"))
                        .child(icons::mark(Icon::Scene)),
                    html::b().child(named),
                    html::span().class("in").child(move || {
                        copies.get().and_then(|copies| {
                            copies.get(&copy).map(|listed| {
                                speech::say_about("feedback-in-copy", "copy", &listed.name)
                            })
                        })
                    }),
                ))
                .into_any();
            let said_on = match place {
                ReadingCopyPlace::Passage { quote, .. } => html::p()
                    .class("quote")
                    .attr("title", speech::say_about("feedback-quote", "quote", quote))
                    .child(speech::say_about(
                        "feedback-quote",
                        "quote",
                        &column::shortened(quote),
                    ))
                    .into_any(),
                ReadingCopyPlace::Scene(_) => html::p()
                    .class("on")
                    .child(speech::said("feedback-on-the-scene"))
                    .into_any(),
                ReadingCopyPlace::Section(_) => html::p()
                    .class("on")
                    .child(speech::said("feedback-on-the-section"))
                    .into_any(),
                ReadingCopyPlace::Whole => ().into_any(),
            };
            let opened = html::a()
                .class("open-in-copy")
                .href(format!(
                    "{}?feedback={}",
                    route::reading_copy(project, &reading_copy.to_string()),
                    piece.id
                ))
                .child(speech::said("feedback-open-in-copy"))
                .into_any();

            (about, said_on, opened)
        }
    }
}

/// The name of a place in the book, as it reads now: a section's title, the section a scene lies in, or the whole work.
fn named_in_the_book(place: &ReadingCopyPlace, book: RwSignal<Option<Book>>) -> String {
    let titled = |found: Option<String>| match found {
        Some(title) if !title.trim().is_empty() => title,
        _ => speech::say("heading-untitled"),
    };

    match place {
        ReadingCopyPlace::Whole => speech::say("feedback-whole-work"),
        ReadingCopyPlace::Section(section) => titled(book.with(|held| {
            held.iter()
                .flat_map(|held| &held.sections)
                .find(|outlined| &outlined.section == section)
                .map(|outlined| outlined.title.clone())
        })),
        ReadingCopyPlace::Scene(scene) | ReadingCopyPlace::Passage { scene, .. } => {
            titled(book.with(|held| {
                held.iter()
                    .flat_map(|held| &held.sections)
                    .find(|outlined| outlined.scenes.contains(scene))
                    .map(|outlined| outlined.title.clone())
            }))
        }
    }
}
