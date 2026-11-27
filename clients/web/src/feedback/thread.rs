use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::feedback::column;
use crate::feedback::composer;
use crate::feedback::model::{CommentBlock, CommentRun, FeedbackId, Reply};
use crate::feedback::service;
use crate::http::ApiError;
use crate::icons;
use crate::icons::Icon;
use crate::problem;
use crate::speech;

/// A thread under a piece given, its root, and what is going on in it:
/// its replies once asked for, how many there are, whether it is open, and which of yours is being changed.
#[derive(Clone, Copy)]
struct Thread {
    root: StoredValue<FeedbackId>,
    replies: RwSignal<Vec<Reply>>,
    counted: RwSignal<u32>,
    open: RwSignal<bool>,
    writing: RwSignal<bool>,
    /// What a reply being begun holds at first: nothing, or the reply it answers, quoted.
    quoting: RwSignal<Vec<CommentBlock>>,
    changing: RwSignal<Option<FeedbackId>>,
    failed: RwSignal<Option<ApiError>>,
}

/// The replies on a piece given, under it: how many there are, opened to read them, and a reply written there.
/// A reply is sent at once; yours may be changed, and deleted once asked.
pub fn thread(root: FeedbackId, counted: u32) -> impl IntoView {
    let thread = Thread {
        root: StoredValue::new(root),
        replies: RwSignal::new(Vec::new()),
        counted: RwSignal::new(counted),
        open: RwSignal::new(false),
        writing: RwSignal::new(false),
        quoting: RwSignal::new(Vec::new()),
        changing: RwSignal::new(None),
        failed: RwSignal::new(None),
    };
    let asked = StoredValue::new(false);
    Effect::new(move |_| {
        if !thread.open.get() || asked.get_value() {
            return;
        }
        asked.set_value(true);
        let root = thread.root.get_value();
        leptos::task::spawn_local(async move {
            match service::thread(&root).await {
                Ok(replies) => {
                    _ = thread
                        .counted
                        .try_set(u32::try_from(replies.len()).unwrap_or(u32::MAX));
                    _ = thread.replies.try_set(replies);
                }
                Err(failure) => _ = thread.failed.try_set(Some(failure)),
            }
        });
    });
    let sending = Action::new_local(move |comment: &Vec<CommentBlock>| {
        let root = thread.root.get_value();
        let comment = comment.clone();

        async move {
            match service::reply(&root, &comment).await {
                Ok(sent) => {
                    thread.failed.set(None);
                    thread.replies.update(|replies| replies.push(sent));
                    thread.counted.update(|counted| *counted += 1);
                    thread.writing.set(false);
                }
                Err(failure) => thread.failed.set(Some(failure)),
            }
        }
    });

    html::section()
        .class("thread")
        .attr("aria-label", speech::said("feedback-replies-named"))
        .child((
            html::div().class("thread-head").child((
                move || {
                    let counted = thread.counted.get();

                    (counted > 0).then(|| {
                        html::button()
                            .r#type("button")
                            .class("replies-toggle")
                            .attr("aria-expanded", move || thread.open.get().to_string())
                            .on(ev::click, move |_| {
                                thread.open.update(|open| *open = !*open)
                            })
                            .child(move || match thread.open.get() {
                                true => speech::say("feedback-hide-replies"),
                                false => replies_counted(thread.counted.get()),
                            })
                    })
                },
                move || {
                    (!thread.writing.get())
                        .then(|| replying(thread, Vec::new(), "feedback-reply-to-comment"))
                },
            )),
            move || thread.failed.get().map(problem::problem),
            move || {
                thread.open.get().then(|| {
                    For(ForProps::builder()
                        .each(move || thread.replies.get())
                        .key(Clone::clone)
                        .children(move |reply| a_reply(reply, thread))
                        .build())
                })
            },
            move || {
                thread.writing.get().then(|| {
                    html::div().class("reply").child(composer::composer(
                        thread.quoting.get(),
                        "feedback-send-reply",
                        sending.pending().into(),
                        move |comment| _ = sending.dispatch(comment),
                        move || thread.writing.set(false),
                    ))
                })
            },
        ))
}

/// What begins a reply: on the comment itself, or on a reply, quoting it, as the thread is flat.
fn replying(
    thread: Thread,
    quoting: Vec<CommentBlock>,
    labelled: &'static str,
) -> impl IntoView + use<> {
    html::button()
        .r#type("button")
        .class("reply-to")
        .attr("aria-label", speech::said(labelled))
        .on(ev::click, move |_| {
            thread.quoting.set(quoting.clone());
            thread.open.set(true);
            thread.writing.set(true);
        })
        .child((
            icons::mark(Icon::Reply),
            html::span().child(speech::said("feedback-reply")),
        ))
}

/// A reply quoted at the start of one answering it: who wrote it, and what it says of its own,
/// leaving out what it quoted in turn, shortened as passages are; then a line to write on.
fn quoted(reply: &Reply) -> Vec<CommentBlock> {
    let said = reply
        .comment
        .iter()
        .flat_map(|block| match block {
            CommentBlock::Paragraph(runs) => vec![runs.clone()],
            CommentBlock::List { items, .. } => items.clone(),
            CommentBlock::Quote(_) => Vec::new(),
        })
        .map(|runs| {
            runs.iter()
                .map(|run| match run {
                    CommentRun::Text { text, .. } => text.as_str(),
                    CommentRun::Break => " ",
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(" ");
    let run = |text: String, strong: bool| CommentRun::Text {
        text,
        emphasis: false,
        strong,
        link: None,
    };

    vec![
        CommentBlock::Quote(vec![
            run(column::named(reply.by_you, reply.written_by.as_ref()), true),
            run(": ".to_owned(), false),
            run(column::shortened(said.trim()), false),
        ]),
        CommentBlock::Paragraph(Vec::new()),
    ]
}

fn replies_counted(counted: u32) -> String {
    let mut args = FluentArgs::new();
    args.set("count", counted);

    speech::say_with("feedback-replies", &args)
}

/// A reply in its thread, with who sent it and when; yours may be changed, and deleted once asked.
fn a_reply(reply: Reply, thread: Thread) -> impl IntoView {
    let id = reply.id.clone();
    let deleting = Action::new_local({
        let id = id.clone();

        move |_: &()| {
            let id = id.clone();

            async move {
                match service::delete(&id).await {
                    Ok(()) => {
                        thread
                            .replies
                            .update(|replies| replies.retain(|reply| reply.id != id));
                        thread
                            .counted
                            .update(|counted| *counted = counted.saturating_sub(1));
                    }
                    Err(failure) => thread.failed.set(Some(failure)),
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
                match service::rewrite_reply(&id, &comment).await {
                    Ok(rewritten) => {
                        thread.failed.set(None);
                        thread.replies.update(|replies| {
                            if let Some(held) = replies.iter_mut().find(|reply| reply.id == id) {
                                *held = rewritten;
                            }
                        });
                        thread.changing.set(None);
                    }
                    Err(failure) => thread.failed.set(Some(failure)),
                }
            }
        }
    });
    let being_changed = {
        let id = id.clone();

        Memo::new(move |_| thread.changing.get().as_ref() == Some(&id))
    };
    let held = reply.comment.clone();
    let yours = reply.by_you;
    let quoting = quoted(&reply);

    html::article()
        .class("reply")
        .attr("data-feedback", id.to_string())
        .child((
            column::who_wrote(
                column::named(reply.by_you, reply.written_by.as_ref()),
                reply.written_at,
                reply.edited,
            ),
            move || match being_changed.get() {
                true => composer::composer(
                    held.clone(),
                    "feedback-save-reply",
                    rewriting.pending().into(),
                    move |comment| _ = rewriting.dispatch(comment),
                    move || thread.changing.set(None),
                )
                .into_any(),
                false => {
                    let id = id.clone();

                    (
                        html::div().class("comment").child(column::comment(&held)),
                        html::div().class("note-actions").child((
                            replying(thread, quoting.clone(), "feedback-reply-to-reply"),
                            yours.then(|| {
                                (
                                    html::button()
                                        .r#type("button")
                                        .attr("aria-label", speech::said("feedback-change-reply"))
                                        .on(ev::click, move |_| {
                                            thread.changing.set(Some(id.clone()));
                                        })
                                        .child(speech::said("feedback-change")),
                                    column::deleting_given(
                                        &speech::say("feedback-delete-reply-asked"),
                                        "feedback-delete-reply",
                                        deleting,
                                    ),
                                )
                            }),
                        )),
                    )
                        .into_any()
                }
            },
        ))
}
