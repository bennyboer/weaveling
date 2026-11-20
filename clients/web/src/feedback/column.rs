use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::drawer;
use crate::drawer::{Drawer, Edge, Inside};
use crate::feedback::composer;
use crate::feedback::model::{
    About, CommentBlock, CommentRun, Feedback, FeedbackId, ReadingCopyPlace,
};
use crate::feedback::service;
use crate::http::ApiError;
use crate::icons;
use crate::icons::Icon;
use crate::problem;
use crate::readings::model::Book;
use crate::readings::reading_copies::ReadingCopyId;
use crate::readings::view::safe_link;
use crate::speech;

pub const FEEDBACK: &str = "reading-feedback";

/// What is being written in the column, if anything: a new draft about something, or one of your drafts changed.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Writing {
    New(About),
    Changed(FeedbackId),
}

/// The feedback on a reading copy as the column holds it, what is being written in it, and what last failed.
#[derive(Clone, Copy)]
struct Column {
    feedback: RwSignal<Vec<Feedback>>,
    writing: RwSignal<Option<Writing>>,
    failed: RwSignal<Option<ApiError>>,
    book: RwSignal<Option<Book>>,
}

/// The feedback on a reading copy, in a drawer on the right: for now your drafts, which only you see,
/// and a comment on the whole copy started there.
pub fn on_reading_copy(
    copy: ReadingCopyId,
    book: RwSignal<Option<Book>>,
    drawer: Drawer,
) -> impl IntoView {
    let column = Column {
        feedback: RwSignal::new(Vec::new()),
        writing: RwSignal::new(None),
        failed: RwSignal::new(None),
        book,
    };
    {
        let copy = copy.clone();
        leptos::task::spawn_local(async move {
            match service::on_reading_copy(&copy).await {
                Ok(found) => _ = column.feedback.try_set(found),
                Err(failure) => _ = column.failed.try_set(Some(failure)),
            }
        });
    }
    let drafts = Memo::new(move |_| {
        column.feedback.with(|feedback| {
            feedback
                .iter()
                .filter(|piece| piece.draft && piece.by_you)
                .cloned()
                .collect::<Vec<_>>()
        })
    });
    let whole = About::ReadingCopy {
        reading_copy: copy,
        place: ReadingCopyPlace::Whole,
    };

    drawer::drawer(
        drawer,
        Edge::Right,
        || speech::say("feedback"),
        Inside {
            id: FEEDBACK,
            named: speech::said("feedback").into(),
            called: "feedback",
            head: html::p().class("tally").child(speech::said("feedback")),
            body: (
                move || match column.writing.get() {
                    Some(Writing::New(about)) => a_new_draft(about, column).into_any(),
                    _ => {
                        let whole = whole.clone();

                        html::button()
                            .r#type("button")
                            .class("comment-on")
                            .on(ev::click, move |_| {
                                column.failed.set(None);
                                column.writing.set(Some(Writing::New(whole.clone())));
                            })
                            .child((
                                icons::mark(Icon::Comment),
                                html::span().child(speech::said("feedback-comment-on-whole")),
                            ))
                            .into_any()
                    }
                },
                move || column.failed.get().map(problem::problem),
                move || match drafts.with(Vec::len) {
                    0 => (column.writing.with(Option::is_none))
                        .then(|| {
                            html::p()
                                .class("nothing-yet")
                                .child(speech::said("feedback-nothing-yet"))
                        })
                        .into_any(),
                    count => html::section()
                        .class("drafts")
                        .attr("aria-label", speech::said("feedback-your-drafts-named"))
                        .child((
                            html::div().class("drafts-head").child((
                                html::b().child(speech::say_about(
                                    "feedback-your-drafts",
                                    "count",
                                    &count.to_string(),
                                )),
                                html::span()
                                    .class("hint")
                                    .child(speech::said("feedback-drafts-only-you")),
                            )),
                            For(ForProps::builder()
                                .each(move || drafts.get())
                                .key(|draft| draft.clone())
                                .children(move |draft| a_draft(draft, column))
                                .build()),
                        ))
                        .into_any(),
                },
            ),
        },
    )
}

/// A draft being started, about what it says above the field.
fn a_new_draft(about: About, column: Column) -> impl IntoView {
    let drafting = Action::new_local({
        let about = about.clone();

        move |comment: &Vec<CommentBlock>| {
            let about = about.clone();
            let comment = comment.clone();

            async move {
                match service::draft(&about, &comment).await {
                    Ok(drafted) => {
                        column.failed.set(None);
                        column.feedback.update(|feedback| feedback.push(drafted));
                        column.writing.set(None);
                    }
                    Err(failure) => column.failed.set(Some(failure)),
                }
            }
        }
    });

    html::section()
        .class("note draft writing")
        .attr("aria-label", speech::said("feedback-new-draft"))
        .child((
            about_shown(&about, column.book),
            composer::composer(
                Vec::new(),
                drafting.pending().into(),
                move |comment| _ = drafting.dispatch(comment),
                move || column.writing.set(None),
            ),
        ))
}

fn a_draft(draft: Feedback, column: Column) -> impl IntoView {
    let id = draft.id.clone();
    let deleting = Action::new_local({
        let id = id.clone();

        move |_: &()| {
            let id = id.clone();

            async move {
                match service::delete(&id).await {
                    Ok(()) => column
                        .feedback
                        .update(|feedback| feedback.retain(|piece| piece.id != id)),
                    Err(failure) => column.failed.set(Some(failure)),
                }
            }
        }
    });
    let rewriting = Action::new_local({
        let id = id.clone();

        move |comment: &Vec<CommentBlock>| {
            let id = id.clone();
            let comment = comment.clone();

            async move {
                match service::rewrite(&id, &comment).await {
                    Ok(rewritten) => {
                        column.failed.set(None);
                        column.feedback.update(|feedback| {
                            if let Some(held) = feedback.iter_mut().find(|piece| piece.id == id) {
                                *held = rewritten;
                            }
                        });
                        column.writing.set(None);
                    }
                    Err(failure) => column.failed.set(Some(failure)),
                }
            }
        }
    });
    let being_changed = {
        let id = id.clone();

        Memo::new(move |_| column.writing.get() == Some(Writing::Changed(id.clone())))
    };
    let held = draft.comment.clone();

    html::article()
        .class("note draft")
        .child((
            about_shown(&draft.about, column.book),
            move || match being_changed.get() {
                true => composer::composer(
                    held.clone(),
                    rewriting.pending().into(),
                    move |comment| _ = rewriting.dispatch(comment),
                    move || column.writing.set(None),
                )
                .into_any(),
                false => {
                    let id = id.clone();

                    (
                        html::div().class("comment").child(comment(&held)),
                        html::div().class("note-actions").child((
                            html::button()
                                .r#type("button")
                                .attr("aria-label", speech::said("feedback-change-draft"))
                                .on(ev::click, move |_| {
                                    column.failed.set(None);
                                    column.writing.set(Some(Writing::Changed(id.clone())));
                                })
                                .child(speech::said("feedback-change")),
                            html::button()
                                .r#type("button")
                                .attr("aria-label", speech::said("feedback-delete-draft"))
                                .disabled(move || deleting.pending().get())
                                .on(ev::click, move |_| _ = deleting.dispatch(()))
                                .child(speech::said("feedback-delete")),
                        )),
                    )
                        .into_any()
                }
            },
        ))
}

/// What a piece of feedback is about: the passage it quotes, or the scene, section or whole copy it is on.
fn about_shown(about: &About, book: RwSignal<Option<Book>>) -> AnyView {
    let About::ReadingCopy { place, .. } = about;
    match place {
        ReadingCopyPlace::Passage { quote, .. } => html::blockquote()
            .class("quote")
            .child(format!("\u{201c}{quote}\u{201d}"))
            .into_any(),
        ReadingCopyPlace::Section(section) => {
            let section = section.clone();
            let title = move || {
                book.with(|held| {
                    held.iter()
                        .flat_map(|held| &held.sections)
                        .find(|outlined| outlined.section == section)
                        .map(|outlined| outlined.title.clone())
                })
                .unwrap_or_default()
            };

            html::p()
                .class("on")
                .child(move || speech::say_about("feedback-on-section", "section", &title()))
                .into_any()
        }
        ReadingCopyPlace::Scene(scene) => {
            let scene = scene.clone();
            let title = move || {
                book.with(|held| {
                    held.iter()
                        .flat_map(|held| &held.sections)
                        .find(|outlined| outlined.scenes.contains(&scene))
                        .map(|outlined| outlined.title.clone())
                })
                .unwrap_or_default()
            };

            html::p()
                .class("on")
                .child(move || speech::say_about("feedback-on-scene", "section", &title()))
                .into_any()
        }
        ReadingCopyPlace::Whole => html::p()
            .class("on")
            .child(speech::said("feedback-on-whole"))
            .into_any(),
    }
}

/// What a comment says, drawn as a reading copy's text is: never as markup it brought along.
pub fn comment(blocks: &[CommentBlock]) -> Vec<AnyView> {
    blocks
        .iter()
        .map(|block| match block {
            CommentBlock::Paragraph(runs) => html::p().child(runs_of(runs)).into_any(),
            CommentBlock::List { ordered, items } => {
                let items = items
                    .iter()
                    .map(|item| html::li().child(runs_of(item)))
                    .collect::<Vec<_>>();

                if *ordered {
                    html::ol().child(items).into_any()
                } else {
                    html::ul().child(items).into_any()
                }
            }
        })
        .collect()
}

fn runs_of(runs: &[CommentRun]) -> Vec<AnyView> {
    runs.iter()
        .map(|run| match run {
            CommentRun::Text {
                text,
                emphasis,
                strong,
                link,
            } => {
                let mut shown = text.clone().into_any();
                if *emphasis {
                    shown = html::em().child(shown).into_any();
                }
                if *strong {
                    shown = html::strong().child(shown).into_any();
                }
                match link.as_deref().filter(|link| safe_link(link)) {
                    Some(link) => html::a()
                        .href(link.to_owned())
                        .target("_blank")
                        .rel("noopener noreferrer")
                        .child(shown)
                        .into_any(),
                    None => shown,
                }
            }
            CommentRun::Break => html::br().into_any(),
        })
        .collect()
}
