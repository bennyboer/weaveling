use std::collections::HashMap;

use feedback_contract::SubjectDTO;
use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::feedback::column;
use crate::feedback::composer;
use crate::feedback::followed::{FollowedFeedback, Heard};
use crate::feedback::model::{CommentBlock, CommentRun, FeedbackId, Reply};
use crate::feedback::service;
use crate::http::ApiError;
use crate::icons;
use crate::icons::Icon;
use crate::problem;
use crate::speech;

/// The threads shown in one place, the column, the feedback view or an inspector, each kept by the piece it is on.
/// So a thread stays as it was, open and with its replies, while the note it is under is drawn again as its piece changes.
#[derive(Clone, Copy)]
pub struct Threads {
    kept: StoredValue<HashMap<FeedbackId, Thread>>,
    owner: StoredValue<Owner>,
}

impl Threads {
    /// Threads kept for as long as the place they are shown in.
    pub fn here() -> Self {
        Self {
            kept: StoredValue::new(HashMap::new()),
            owner: StoredValue::new(
                Owner::current().expect("threads are kept where something is shown"),
            ),
        }
    }

    /// The thread on a piece, as it was if it was shown before; until its replies are asked for,
    /// it takes the count the piece was listed with.
    fn of(self, root: FeedbackId, counted: u32) -> Thread {
        if let Some(thread) = self.kept.with_value(|kept| kept.get(&root).copied()) {
            if !thread.asked.get_untracked() {
                thread.counted.set(counted);
            }
            return thread;
        }
        let thread = self
            .owner
            .get_value()
            .with(|| Thread::on(root.clone(), counted));
        self.kept.update_value(|kept| {
            kept.insert(root, thread);
        });

        thread
    }
}

/// A thread under a piece given, its root, and what is going on in it:
/// its replies once asked for, how many there are, whether it is open, and which of yours is being changed.
#[derive(Clone, Copy)]
struct Thread {
    root: StoredValue<FeedbackId>,
    /// Whether its replies were asked for, so it holds them all and takes in what changes.
    asked: RwSignal<bool>,
    replies: RwSignal<Vec<Reply>>,
    counted: RwSignal<u32>,
    open: RwSignal<bool>,
    writing: RwSignal<bool>,
    /// What a reply being begun holds at first: nothing, or the reply it answers, quoted.
    quoting: RwSignal<Vec<CommentBlock>>,
    changing: RwSignal<Option<FeedbackId>>,
    failed: RwSignal<Option<ApiError>>,
}

impl Thread {
    fn on(root: FeedbackId, counted: u32) -> Self {
        Self {
            root: StoredValue::new(root),
            asked: RwSignal::new(false),
            replies: RwSignal::new(Vec::new()),
            counted: RwSignal::new(counted),
            open: RwSignal::new(false),
            writing: RwSignal::new(false),
            quoting: RwSignal::new(Vec::new()),
            changing: RwSignal::new(None),
            failed: RwSignal::new(None),
        }
    }

    /// Asks for its replies, all of them, in the order sent.
    fn asked_anew(self) {
        self.asked.set(true);
        let root = self.root.get_value();
        leptos::task::spawn_local(async move {
            match service::thread(&root).await {
                Ok(replies) => {
                    _ = self.replies.try_set(replies);
                    self.recounted();
                }
                Err(failure) => _ = self.failed.try_set(Some(failure)),
            }
        });
    }

    /// Its count, as many replies as it holds.
    fn recounted(self) {
        let held = self.replies.with_untracked(Vec::len);
        _ = self
            .counted
            .try_set(u32::try_from(held).unwrap_or(u32::MAX));
    }

    /// Takes in what was heard on it: once its replies were asked for, those changed are asked for again
    /// and those gone let go of, or all of them again after a break; until then only its count is asked for.
    fn heard(self, heard: &Heard) {
        let on_it = SubjectDTO::Feedback(self.root.get_value().to_string()).to_string();
        let changed = heard.changed_on(&on_it);
        let gone = heard.gone_on(&on_it);
        if changed.is_empty() && gone.is_empty() && !heard.missed {
            return;
        }
        if !self.asked.get_untracked() {
            let root = self.root.get_value();
            leptos::task::spawn_local(async move {
                if let Ok(counted) = service::replies_counted(&root).await {
                    _ = self.counted.try_set(counted);
                }
            });
            return;
        }
        if heard.missed {
            self.asked_anew();
            return;
        }
        self.replies
            .update(|replies| replies.retain(|reply| !gone.contains(&reply.id)));
        self.recounted();
        if changed.is_empty() {
            return;
        }
        leptos::task::spawn_local(async move {
            if let Ok(found) = service::replies(&changed).await {
                _ = self.replies.try_update(|replies| {
                    replies.retain(|reply| {
                        !changed.contains(&reply.id) || found.iter().any(|one| one.id == reply.id)
                    });
                    for reply in found {
                        match replies.iter_mut().find(|one| one.id == reply.id) {
                            Some(held) => *held = reply,
                            None => replies.push(reply),
                        }
                    }
                });
                self.recounted();
            }
        });
    }
}

/// The replies on a piece given, under it: how many there are, opened to read them, and a reply written there.
/// A reply is sent at once; yours may be changed, and deleted once asked. What others reply is taken in as it is heard.
pub fn thread(root: FeedbackId, counted: u32) -> impl IntoView {
    let thread = expect_context::<Threads>().of(root, counted);
    Effect::new(move |_| {
        if thread.open.get() && !thread.asked.get_untracked() {
            thread.asked_anew();
        }
    });
    let followed = expect_context::<FollowedFeedback>();
    Effect::new(move |heard_before: Option<()>| {
        let heard = followed.heard();
        if heard_before.is_some() {
            thread.heard(&heard);
        }
    });
    let sending = Action::new_local(move |comment: &Vec<CommentBlock>| {
        let root = thread.root.get_value();
        let comment = comment.clone();

        async move {
            match service::reply(&root, &comment).await {
                Ok(sent) => {
                    thread.failed.set(None);
                    thread.replies.update(|replies| replies.push(sent));
                    thread.recounted();
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
                        thread.recounted();
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
