use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::feedback::column;
use crate::feedback::model::{About, Feedback, ReadingCopyPlace};
use crate::feedback::pending::PendingHere;
use crate::feedback::service;
use crate::feedback::service::Asked;
use crate::members::roles::Roles;
use crate::outline::model::SectionId;
use crate::readings::reading_copies::{ListedReadingCopy, ReadingCopyId};
use crate::readings::service as readings;
use crate::readings::switcher;
use crate::route;
use crate::scenes::model::SceneId;
use crate::speech;

/// What the inspector looks at, whose feedback it shows: a scene, on it or a passage in it, or a section itself.
#[derive(Clone)]
enum Inspected {
    Scene(SceneId),
    Section(SectionId),
}

/// The feedback given on a scene, in the scene's inspector.
pub fn on_scene(scene: SceneId, project: String) -> impl IntoView {
    given_in(Inspected::Scene(scene), project)
}

/// The feedback given on a section itself, in the section's inspector.
pub fn on_section(section: SectionId, project: String) -> impl IntoView {
    given_in(Inspected::Section(section), project)
}

/// The feedback given on what is inspected: grouped by the copy it was given in, each copy named with its times,
/// each piece shown as it was given, marked done there by whoever may write, and opened in its copy.
/// Only what is not done yet is counted and shown; what is done is put away, and shown again when asked for.
fn given_in(inspected: Inspected, project: String) -> impl IntoView {
    let given = RwSignal::new(Vec::<Feedback>::new());
    {
        let inspected = inspected.clone();
        let in_project = Asked::in_project(&route::project_id(&project));
        leptos::task::spawn_local(async move {
            let asked = match &inspected {
                Inspected::Scene(scene) => Asked {
                    scenes: vec![scene.clone()],
                    ..in_project
                },
                Inspected::Section(section) => Asked {
                    sections: vec![section.clone()],
                    ..in_project
                },
            };
            let found = service::submitted(&asked, None).await;
            if let Ok(found) = found {
                _ = given.try_set(found);
            }
        });
    }
    let (none_yet, on_the_whole) = match inspected {
        Inspected::Scene(_) => ("feedback-none-on-scene", "feedback-on-the-scene"),
        Inspected::Section(_) => ("feedback-none-on-section", "feedback-on-the-section"),
    };
    let copies = LocalResource::new({
        let project = route::project_id(&project);

        move || {
            let project = project.clone();

            async move { readings::copies(&project).await.unwrap_or_default() }
        }
    });
    let marking = {
        let roles = expect_context::<Roles>();
        let project = project.clone();

        Signal::derive(move || roles.may_write(&project))
    };
    let not_done =
        Memo::new(move |_| given.with(|given| given.iter().filter(|piece| !piece.done).count()));
    let done = Memo::new(move |_| given.with(Vec::len) - not_done.get());
    let showing_done = RwSignal::new(false);
    let shown = Memo::new(move |_| {
        let showing_done = showing_done.get();
        given.with(|given| {
            given
                .iter()
                .filter(|piece| showing_done || !piece.done)
                .cloned()
                .collect::<Vec<_>>()
        })
    });
    let in_copies = Memo::new(move |_| {
        shown.with(|given| {
            let mut copies: Vec<ReadingCopyId> = Vec::new();
            for piece in given {
                let About::ReadingCopy { reading_copy, .. } = &piece.about;
                if !copies.contains(reading_copy) {
                    copies.push(reading_copy.clone());
                }
            }

            copies
        })
    });

    html::section()
        .class("feedback-on-scene")
        .attr("aria-label", speech::said("feedback"))
        .child((
            html::h3().class("tally").child(move || {
                speech::say_about("feedback-given", "count", &not_done.get().to_string())
            }),
            move || {
                let said = match (given.with(Vec::is_empty), shown.with(Vec::is_empty)) {
                    (true, _) => Some(none_yet),
                    (false, true) => Some("feedback-none-not-done"),
                    (false, false) => None,
                };

                said.map(|said| html::p().class("empty").child(speech::said(said)))
            },
            For(ForProps::builder()
                .each(move || in_copies.get())
                .key(Clone::clone)
                .children(move |copy| {
                    in_copy(
                        copy,
                        given,
                        shown,
                        showing_done,
                        copies,
                        project.clone(),
                        on_the_whole,
                        marking,
                    )
                })
                .build()),
            move || {
                (done.get() > 0).then(|| {
                    html::button()
                        .r#type("button")
                        .class("showing-done")
                        .attr("aria-pressed", move || showing_done.get().to_string())
                        .on(ev::click, move |_| {
                            showing_done.update(|showing| *showing = !*showing)
                        })
                        .child(move || match showing_done.get() {
                            true => speech::say("feedback-hide-done"),
                            false => speech::say_about(
                                "feedback-show-done",
                                "count",
                                &done.get().to_string(),
                            ),
                        })
                })
            },
        ))
}

/// The feedback given in one copy, under the copy's name and times.
#[allow(clippy::too_many_arguments)]
fn in_copy(
    copy: ReadingCopyId,
    given: RwSignal<Vec<Feedback>>,
    shown: Memo<Vec<Feedback>>,
    showing_done: RwSignal<bool>,
    copies: LocalResource<Vec<ListedReadingCopy>>,
    project: String,
    on_the_whole: &'static str,
    marking: Signal<bool>,
) -> impl IntoView {
    let listed = {
        let copy = copy.clone();

        Memo::new(move |_| {
            copies
                .get()
                .and_then(|copies| copies.iter().find(|listed| listed.id == copy).cloned())
        })
    };
    let theirs = {
        let copy = copy.clone();

        Memo::new(move |_| {
            shown.with(|given| {
                given
                    .iter()
                    .filter(|piece| {
                        let About::ReadingCopy { reading_copy, .. } = &piece.about;
                        *reading_copy == copy
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            })
        })
    };
    let opened = route::reading_copy(&project, &copy.to_string());

    html::section()
        .class("copy-group")
        .attr("aria-label", move || {
            listed.get().map(|listed| listed.name).unwrap_or_default()
        })
        .child((
            html::div().class("copy-head").child((
                html::b().child(move || listed.get().map(|listed| listed.name)),
                html::span().child(move || listed.get().map(|listed| switcher::copy_line(&listed))),
            )),
            For(ForProps::builder()
                .each(move || theirs.get())
                .key(Clone::clone)
                .children(move |piece| {
                    let at = format!("{opened}?feedback={}", piece.id);

                    a_piece(piece, given, showing_done, at, on_the_whole, marking)
                })
                .build()),
        ))
}

/// A piece marked done while what is done is put away fades before it goes; otherwise it stays where it is.
fn a_piece(
    piece: Feedback,
    given: RwSignal<Vec<Feedback>>,
    showing_done: RwSignal<bool>,
    opened: String,
    on_the_whole: &'static str,
    marking: Signal<bool>,
) -> impl IntoView {
    let id = piece.id.clone();
    let done = piece.done;
    let shown_done = RwSignal::new(done);
    let leaving = RwSignal::new(false);
    let counted_again = use_context::<PendingHere>();
    let marking_done = Action::new_local(move |marked_done: &bool| {
        let id = id.clone();
        let marked_done = *marked_done;
        shown_done.set(marked_done);

        async move {
            match service::mark_done(&id, marked_done).await {
                Ok(marked) => {
                    if let Some(not_done) = counted_again {
                        not_done.again();
                    }
                    if marked_done && !showing_done.get_untracked() {
                        _ = set_timeout(
                            move || _ = leaving.try_set(true),
                            column::SETTLED_BEFORE_LEAVING,
                        );
                    }
                    _ = set_timeout(
                        move || {
                            _ = given.try_update(|given| {
                                if let Some(held) = given.iter_mut().find(|piece| piece.id == id) {
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
    let About::ReadingCopy { place, .. } = &piece.about;
    let about = match place {
        ReadingCopyPlace::Passage { quote, .. } => html::p()
            .class("quote")
            .attr("title", speech::say_about("feedback-quote", "quote", quote))
            .child(speech::say_about(
                "feedback-quote",
                "quote",
                &column::shortened(quote),
            ))
            .into_any(),
        _ => html::p()
            .class("on")
            .child(speech::said(on_the_whole))
            .into_any(),
    };

    html::article()
        .class("note")
        .class(("leaving", move || leaving.get()))
        .child((
            column::who(&piece),
            about,
            html::div()
                .class("comment")
                .child(column::comment(&piece.comment)),
            html::div().class("note-foot").child((
                html::a()
                    .class("open-in-copy")
                    .href(opened)
                    .child(speech::said("feedback-open-in-copy")),
                column::done_mark(done, shown_done, marking_done, marking),
            )),
        ))
}
