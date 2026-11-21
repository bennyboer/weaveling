use fluent_bundle::FluentArgs;
use leptos::portal::Portal;
use leptos::prelude::*;
use leptos::{IntoView, ev, html, view};

use crate::dialog;
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
use crate::moment;
use crate::problem;
use crate::readings;
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

/// Begins comments on places in a reading copy, from wherever in the reading view a place is offered:
/// it opens the feedback and starts a draft there about the place.
#[derive(Clone, Copy)]
pub struct Commenting {
    copy: StoredValue<ReadingCopyId>,
    feedback: RwSignal<Vec<Feedback>>,
    writing: RwSignal<Option<Writing>>,
    drawer: Drawer,
}

impl Commenting {
    pub fn new(copy: ReadingCopyId, drawer: Drawer) -> Self {
        Self {
            copy: StoredValue::new(copy),
            feedback: RwSignal::new(Vec::new()),
            writing: RwSignal::new(None),
            drawer,
        }
    }

    /// Where you have a draft about the place already, that draft is opened to be changed and pointed at in the column instead,
    /// so one place gathers what you say about it.
    pub fn begin(self, place: ReadingCopyPlace) {
        self.drawer.open();
        match self.draft_on(&place) {
            Some(draft) => {
                self.writing.set(Some(Writing::Changed(draft.clone())));
                _ = request_animation_frame(move || pointed_at_in_the_column(&draft));
            }
            None => self.writing.set(Some(Writing::New(About::ReadingCopy {
                reading_copy: self.copy.get_value(),
                place,
            }))),
        }
    }

    /// Whether you have a draft about this very place, so what offers to comment on it can say so.
    pub fn drafted_on(self, place: &ReadingCopyPlace) -> bool {
        self.draft_on(place).is_some()
    }

    /// Your first draft about this very place, if any.
    fn draft_on(self, place: &ReadingCopyPlace) -> Option<FeedbackId> {
        self.feedback.with(|feedback| {
            feedback
                .iter()
                .find(|piece| {
                    let About::ReadingCopy { place: about, .. } = &piece.about;

                    piece.draft && piece.by_you && about == place
                })
                .map(|piece| piece.id.clone())
        })
    }
}

/// Brings a piece of feedback in the column into sight and makes it pulse a moment, as places in the text do.
fn pointed_at_in_the_column(feedback: &FeedbackId) {
    if let Ok(Some(found)) =
        document().query_selector(&format!("#{FEEDBACK} [data-feedback=\"{feedback}\"]"))
    {
        readings::pointing::pulse(found);
    }
}

/// The feedback on a reading copy as the column holds it, what is being written in it, and what last failed.
#[derive(Clone, Copy)]
struct Column {
    feedback: RwSignal<Vec<Feedback>>,
    writing: RwSignal<Option<Writing>>,
    failed: RwSignal<Option<ApiError>>,
    book: RwSignal<Option<Book>>,
    drawer: Drawer,
}

/// The feedback on a reading copy, in a drawer on the right: your drafts, which only you see until you submit them,
/// the feedback everyone submitted, and a comment on the whole copy started there.
pub fn on_reading_copy(commenting: Commenting, book: RwSignal<Option<Book>>) -> impl IntoView {
    let column = Column {
        feedback: commenting.feedback,
        writing: commenting.writing,
        failed: RwSignal::new(None),
        book,
        drawer: commenting.drawer,
    };
    Effect::new(move |_| {
        if column.writing.with(Option::is_some) {
            column.failed.set(None);
        }
    });
    let copy = commenting.copy.get_value();
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
    marked_in_the_text(drafts, book);
    let given = Memo::new(move |_| {
        column.feedback.with(|feedback| {
            feedback
                .iter()
                .filter(|piece| !piece.draft)
                .cloned()
                .collect::<Vec<_>>()
        })
    });
    let submitting = Action::new_local({
        let copy = copy.clone();

        move |_: &()| {
            let copy = copy.clone();

            async move {
                match service::submit_on_reading_copy(&copy).await {
                    Ok(submitted) => column.feedback.update(|feedback| {
                        for sent in submitted {
                            if let Some(held) =
                                feedback.iter_mut().find(|piece| piece.id == sent.id)
                            {
                                *held = sent;
                            }
                        }
                    }),
                    Err(failure) => column.failed.set(Some(failure)),
                }
            }
        }
    });
    let any_drafts = Memo::new(move |_| !drafts.with(Vec::is_empty));
    let any_given = Memo::new(move |_| !given.with(Vec::is_empty));
    let whole = About::ReadingCopy {
        reading_copy: copy,
        place: ReadingCopyPlace::Whole,
    };

    drawer::drawer(
        commenting.drawer,
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
                move || {
                    let nothing = drafts.with(Vec::is_empty)
                        && given.with(Vec::is_empty)
                        && column.writing.with(Option::is_none);

                    nothing.then(|| {
                        html::p()
                            .class("nothing-yet")
                            .child(speech::said("feedback-nothing-yet"))
                    })
                },
                move || {
                    any_drafts
                        .get()
                        .then(|| your_drafts(drafts, submitting, column))
                },
                move || any_given.get().then(|| given_feedback(given, column)),
            ),
        },
    )
}

/// Your drafts in a dashed box, with the button that submits them all together.
fn your_drafts(
    drafts: Memo<Vec<Feedback>>,
    submitting: Action<(), ()>,
    column: Column,
) -> impl IntoView {
    let asking = RwSignal::new(false);

    html::section()
        .class("drafts")
        .attr("aria-label", speech::said("feedback-your-drafts-named"))
        .child((
            html::div().class("drafts-head").child((
                html::b().child(move || {
                    speech::say_about(
                        "feedback-your-drafts",
                        "count",
                        &drafts.with(Vec::len).to_string(),
                    )
                }),
                html::span()
                    .class("hint")
                    .child(speech::said("feedback-drafts-only-you")),
            )),
            For(ForProps::builder()
                .each(move || drafts.get())
                .key(|draft| draft.clone())
                .children(move |draft| a_note(draft, column))
                .build()),
            html::button()
                .r#type("button")
                .class("submit")
                .disabled(move || submitting.pending().get())
                .on(ev::click, move |_| asking.set(true))
                .child(move || counted("feedback-submit", drafts.with(Vec::len))),
            html::p()
                .class("hint")
                .child(speech::said("feedback-submit-hint")),
            move || {
                asking.get().then(|| {
                    view! { <Portal>{submitting_asked(drafts, submitting, asking)}</Portal> }
                })
            },
        ))
}

/// Asks before your drafts are submitted, as once submitted everyone in the project sees them.
fn submitting_asked(
    drafts: Memo<Vec<Feedback>>,
    submitting: Action<(), ()>,
    asking: RwSignal<bool>,
) -> impl IntoView {
    let close = move || asking.set(false);

    dialog::modal(
        Signal::derive(speech::said("feedback-submit-asked")),
        "submit-feedback",
        close,
        (
            html::p().child(move || counted("feedback-submit-fate", drafts.with(Vec::len))),
            html::div().class("dialog-actions").child((
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| close())
                    .child(speech::said("feedback-submit-cancel")),
                html::button()
                    .r#type("button")
                    .class("primary")
                    .on(ev::click, move |_| {
                        close();
                        submitting.dispatch(());
                    })
                    .child(move || counted("feedback-submit", drafts.with(Vec::len))),
            )),
        ),
    )
}

/// A message told with a count, so the language may choose its plural.
fn counted(message: &str, count: usize) -> String {
    let mut args = FluentArgs::new();
    args.set("count", count);

    speech::say_with(message, &args)
}

/// How the feedback everyone submitted is shown: what is left to do, what is done, or each reader's together.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Showing {
    NotDone,
    Done,
    ByReader,
}

/// The feedback everyone submitted, under its count, shown as chosen.
fn given_feedback(given: Memo<Vec<Feedback>>, column: Column) -> impl IntoView {
    let showing = RwSignal::new(Showing::NotDone);
    let done =
        Memo::new(move |_| given.with(|given| given.iter().filter(|piece| piece.done).count()));
    let choice = move |shown: Showing, said: Signal<String>| {
        html::button()
            .r#type("button")
            .class(("here", move || showing.get() == shown))
            .attr("aria-pressed", move || (showing.get() == shown).to_string())
            .on(ev::click, move |_| showing.set(shown))
            .child(said)
    };

    html::section()
        .class("given")
        .attr("aria-label", speech::said("feedback-given-named"))
        .child((
            html::h2().child(move || {
                speech::say_about("feedback-given", "count", &given.with(Vec::len).to_string())
            }),
            html::div()
                .class("showing")
                .role("group")
                .attr("aria-label", speech::said("feedback-showing"))
                .child((
                    choice(
                        Showing::NotDone,
                        Signal::derive(move || {
                            counted("feedback-not-done", given.with(Vec::len) - done.get())
                        }),
                    ),
                    choice(
                        Showing::Done,
                        Signal::derive(move || counted("feedback-done", done.get())),
                    ),
                    choice(
                        Showing::ByReader,
                        Signal::derive(|| speech::say("feedback-by-reader")),
                    ),
                )),
            move || match showing.get() {
                Showing::ByReader => by_reader(given, column).into_any(),
                kept => {
                    let wanted = kept == Showing::Done;
                    let shown = Memo::new(move |_| {
                        given.with(|given| {
                            given
                                .iter()
                                .filter(|piece| piece.done == wanted)
                                .cloned()
                                .collect::<Vec<_>>()
                        })
                    });

                    (
                        move || {
                            shown.with(Vec::is_empty).then(|| {
                                html::p()
                                    .class("nothing-yet")
                                    .child(speech::said(if wanted {
                                        "feedback-none-done"
                                    } else {
                                        "feedback-none-not-done"
                                    }))
                            })
                        },
                        notes(shown, column),
                    )
                        .into_any()
                }
            },
        ))
}

fn notes(shown: Memo<Vec<Feedback>>, column: Column) -> impl IntoView {
    For(ForProps::builder()
        .each(move || shown.get())
        .key(|piece| piece.clone())
        .children(move |piece| a_note(piece, column))
        .build())
}

/// Who wrote a piece of feedback, as the column names them: you, their name, or a member where it is not known.
fn writer_of(piece: &Feedback) -> String {
    match (&piece.written_by, piece.by_you) {
        (_, true) => speech::say("feedback-you"),
        (Some(name), false) => name.clone(),
        (None, false) => speech::say("feedback-a-member"),
    }
}

/// Each reader's feedback together, under their name, in the order they first gave any.
fn by_reader(given: Memo<Vec<Feedback>>, column: Column) -> impl IntoView {
    let readers = Memo::new(move |_| {
        given.with(|given| {
            let mut readers: Vec<String> = Vec::new();
            for piece in given {
                let writer = writer_of(piece);
                if !readers.contains(&writer) {
                    readers.push(writer);
                }
            }

            readers
        })
    });

    For(ForProps::builder()
        .each(move || readers.get())
        .key(Clone::clone)
        .children(move |reader| {
            let named = reader.clone();
            let theirs = Memo::new(move |_| {
                given.with(|given| {
                    given
                        .iter()
                        .filter(|piece| writer_of(piece) == named)
                        .cloned()
                        .collect::<Vec<_>>()
                })
            });

            html::section()
                .class("reader")
                .attr("aria-label", reader.clone())
                .child((html::h3().child(reader), notes(theirs, column)))
        })
        .build())
}

/// The name the drafts' passages are painted under in the text, as `::highlight(feedback-draft)` styles them.
const DRAFTS_IN_THE_TEXT: &str = "feedback-draft";

/// Marks the passages of your drafts in the copy's text, whenever the drafts or the text change, and no more once the copy is left.
/// They are painted, not wrapped, so nothing of the text as it is drawn is touched.
fn marked_in_the_text(drafts: Memo<Vec<Feedback>>, book: RwSignal<Option<Book>>) {
    Effect::new(move |_| {
        let places: Vec<ReadingCopyPlace> = drafts.with(|drafts| {
            drafts
                .iter()
                .map(|draft| {
                    let About::ReadingCopy { place, .. } = &draft.about;
                    place.clone()
                })
                .collect()
        });
        book.track();
        _ = request_animation_frame(move || {
            let words: Vec<_> = places
                .iter()
                .filter_map(readings::pointing::words_of)
                .collect();
            if words.is_empty() {
                readings::pointing::unhighlighted(DRAFTS_IN_THE_TEXT);
            } else {
                readings::pointing::highlighted(DRAFTS_IN_THE_TEXT, &words);
            }
        });
    });
    on_cleanup(|| readings::pointing::unhighlighted(DRAFTS_IN_THE_TEXT));
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
            about_shown(&about, column),
            composer::composer(
                Vec::new(),
                drafting.pending().into(),
                move |comment| _ = drafting.dispatch(comment),
                move || column.writing.set(None),
            ),
        ))
}

/// A piece of feedback in the column: a draft of yours, or feedback submitted, with who gave it and when;
/// your own may be changed and deleted.
fn a_note(piece: Feedback, column: Column) -> impl IntoView {
    let id = piece.id.clone();
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
    let held = piece.comment.clone();
    let yours = piece.by_you;
    let (change, delete) = match piece.draft {
        true => ("feedback-change-draft", "feedback-delete-draft"),
        false => ("feedback-change-comment", "feedback-delete-comment"),
    };

    html::article()
        .class("note")
        .class(("draft", piece.draft))
        .attr("data-feedback", id.to_string())
        .child((
            (!piece.draft).then(|| who(&piece)),
            about_shown(&piece.about, column),
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
                        yours.then(|| {
                            html::div().class("note-actions").child((
                                html::button()
                                    .r#type("button")
                                    .attr("aria-label", speech::said(change))
                                    .on(ev::click, move |_| {
                                        column.writing.set(Some(Writing::Changed(id.clone())));
                                    })
                                    .child(speech::said("feedback-change")),
                                html::button()
                                    .r#type("button")
                                    .attr("aria-label", speech::said(delete))
                                    .disabled(move || deleting.pending().get())
                                    .on(ev::click, move |_| _ = deleting.dispatch(()))
                                    .child(speech::said("feedback-delete")),
                            ))
                        }),
                    )
                        .into_any()
                }
            },
        ))
}

/// Who gave a piece of feedback and when it was submitted, and whether it was edited since.
fn who(piece: &Feedback) -> impl IntoView + use<> {
    let writer = writer_of(piece);
    let initial = writer.chars().next().map(String::from).unwrap_or_default();
    let when = moment::human_time(piece.submitted_at.unwrap_or(piece.written_at));
    let edited = piece.edited;

    html::div().class("who").child((
        html::span()
            .class("initial")
            .attr("aria-hidden", "true")
            .child(initial),
        html::b().child(writer),
        html::small().child((
            when,
            edited.then(|| (" \u{b7} ", speech::said("feedback-edited"))),
        )),
    ))
}

/// What a piece of feedback is about, as a link to that place in the copy.
/// Where the feedback lies over the text, it is put away so the place can be seen.
fn about_shown(about: &About, column: Column) -> impl IntoView + use<> {
    let About::ReadingCopy { place, .. } = about;
    let pointed = place.clone();

    html::button()
        .r#type("button")
        .class("about")
        .attr("title", speech::said("feedback-show-in-the-text"))
        .on(ev::click, move |_| {
            if readings::view::lying_over_the_text() {
                column.drawer.close();
            }
            readings::pointing::point_at(&pointed);
        })
        .child(what_about(place, column.book))
}

fn what_about(place: &ReadingCopyPlace, book: RwSignal<Option<Book>>) -> AnyView {
    match place {
        ReadingCopyPlace::Passage { quote, .. } => {
            let whole = quote.clone();
            let shortened = shortened(quote);

            html::span()
                .class("quote")
                .attr("title", move || {
                    speech::say_about("feedback-quote", "quote", &whole)
                })
                .child(move || speech::say_about("feedback-quote", "quote", &shortened))
                .into_any()
        }
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

            html::span()
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

            html::span()
                .class("on")
                .child(move || speech::say_about("feedback-on-scene", "section", &title()))
                .into_any()
        }
        ReadingCopyPlace::Whole => html::span()
            .class("on")
            .child(speech::said("feedback-on-whole"))
            .into_any(),
    }
}

/// How long a quoted passage may be before it is shortened to its beginning and its end.
const QUOTED_WHOLE_UP_TO: usize = 160;
const QUOTED_FROM_ITS_START: usize = 100;
const QUOTED_FROM_ITS_END: usize = 40;

/// A passage as its feedback quotes it: whole if it is short, else its beginning and its end with an ellipsis between,
/// each cut at a word, so it still says where it begins and where it ends.
fn shortened(quote: &str) -> String {
    let chars: Vec<char> = quote.chars().collect();
    if chars.len() <= QUOTED_WHOLE_UP_TO {
        return quote.to_owned();
    }
    let start: String = chars[..QUOTED_FROM_ITS_START].iter().collect();
    let end: String = chars[chars.len() - QUOTED_FROM_ITS_END..].iter().collect();
    let start = start
        .rsplit_once(char::is_whitespace)
        .map_or(start.as_str(), |(words, _)| words)
        .trim_end();
    let end = end
        .split_once(char::is_whitespace)
        .map_or(end.as_str(), |(_, words)| words)
        .trim_start();

    format!("{start} \u{2026} {end}")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_passage_is_quoted_whole() {
        // expect: a passage of a few words is quoted as it is
        assert_eq!(shortened("The boats came in"), "The boats came in");
    }

    #[test]
    fn a_long_passage_is_quoted_by_its_beginning_and_its_end_cut_at_words() {
        // given: a passage far longer than is quoted whole
        let passage = "word ".repeat(60) + "and the last words here.";

        // when: it is quoted
        let quoted = shortened(passage.trim());

        // then: it keeps its beginning and its end, whole words only, an ellipsis between
        assert!(quoted.starts_with("word word"), "{quoted}");
        assert!(quoted.ends_with("and the last words here."), "{quoted}");
        assert!(quoted.contains(" \u{2026} "), "{quoted}");
        assert!(
            !quoted.contains("wor \u{2026}") && !quoted.contains("\u{2026} ord"),
            "{quoted}"
        );
        assert!(quoted.chars().count() < 150, "{quoted}");
    }
}
