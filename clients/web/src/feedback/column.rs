use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::drawer;
use crate::drawer::{Drawer, Edge, Inside};
use crate::feedback::model::{About, CommentBlock, CommentRun, Feedback, ReadingCopyPlace};
use crate::feedback::service;
use crate::http::ApiError;
use crate::problem;
use crate::readings::model::Book;
use crate::readings::reading_copies::ReadingCopyId;
use crate::readings::view::safe_link;
use crate::speech;

pub const FEEDBACK: &str = "reading-feedback";

/// The feedback on a reading copy, in a drawer on the right: for now your drafts, which only you see.
pub fn on_reading_copy(
    copy: ReadingCopyId,
    book: RwSignal<Option<Book>>,
    drawer: Drawer,
) -> impl IntoView {
    let feedback = RwSignal::new(Vec::<Feedback>::new());
    let failed = RwSignal::new(None::<ApiError>);
    leptos::task::spawn_local(async move {
        match service::on_reading_copy(&copy).await {
            Ok(found) => _ = feedback.try_set(found),
            Err(failure) => _ = failed.try_set(Some(failure)),
        }
    });
    let drafts = Memo::new(move |_| {
        feedback.with(|feedback| {
            feedback
                .iter()
                .filter(|piece| piece.draft && piece.by_you)
                .cloned()
                .collect::<Vec<_>>()
        })
    });

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
                move || failed.get().map(problem::problem),
                move || match drafts.with(Vec::len) {
                    0 => html::p()
                        .class("nothing-yet")
                        .child(speech::said("feedback-nothing-yet"))
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
                                .children(move |draft| a_draft(draft, book, feedback, failed))
                                .build()),
                        ))
                        .into_any(),
                },
            ),
        },
    )
}

fn a_draft(
    draft: Feedback,
    book: RwSignal<Option<Book>>,
    feedback: RwSignal<Vec<Feedback>>,
    failed: RwSignal<Option<ApiError>>,
) -> impl IntoView {
    let id = draft.id.clone();
    let deleting = Action::new_local(move |_: &()| {
        let id = id.clone();

        async move {
            match service::delete(&id).await {
                Ok(()) => feedback.update(|feedback| feedback.retain(|piece| piece.id != id)),
                Err(failure) => failed.set(Some(failure)),
            }
        }
    });

    html::article().class("note draft").child((
        about(&draft.about, book),
        html::div().class("comment").child(comment(&draft.comment)),
        html::div().class("note-actions").child(
            html::button()
                .r#type("button")
                .class("quiet")
                .attr("aria-label", speech::said("feedback-delete-draft"))
                .disabled(move || deleting.pending().get())
                .on(ev::click, move |_| _ = deleting.dispatch(()))
                .child(speech::said("feedback-delete")),
        ),
    ))
}

/// What a piece of feedback is about: the passage it quotes, or the scene, section or whole copy it is on.
fn about(about: &About, book: RwSignal<Option<Book>>) -> AnyView {
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
