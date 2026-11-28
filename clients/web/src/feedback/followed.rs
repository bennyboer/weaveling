use std::collections::HashMap;
use std::time::Duration;

use feedback_contract::{GivenFeedbackChangeDTO, SubjectDTO};
use leptos::prelude::*;
use live_contract::FrameDTO;

use crate::feedback::model::{Feedback, FeedbackId};
use crate::live::Subscription;

/// How long after a change is heard what was heard is handed on:
/// changes heard close together, as when many drafts are submitted at once, are taken in together,
/// and a piece you marked done yourself has finished moving to where it now belongs before it is shown afresh.
const SETTLING: Duration = Duration::from_millis(1200);

/// What was heard of the feedback given in the project followed, gathered while it settled.
#[derive(Clone, Debug, Default)]
pub struct Heard {
    /// The pieces given that changed, each with what it is given on, named as `subjects=` names it.
    pub changed: HashMap<FeedbackId, String>,
    /// The pieces given that went, each with what it was given on.
    pub gone: HashMap<FeedbackId, String>,
    /// Whether changes may have been missed, as the following was taken up again after a break,
    /// so what is shown is asked for anew.
    pub missed: bool,
}

impl Heard {
    /// The pieces that changed on one subject.
    pub fn changed_on(&self, subject: &str) -> Vec<FeedbackId> {
        on(&self.changed, |on| on == subject)
    }

    /// The pieces that went from one subject.
    pub fn gone_on(&self, subject: &str) -> Vec<FeedbackId> {
        on(&self.gone, |on| on == subject)
    }

    /// The pieces given on the work that changed, its comments, leaving out the replies to them.
    pub fn changed_on_the_work(&self) -> Vec<FeedbackId> {
        on(&self.changed, on_the_work)
    }

    /// Whether a piece went.
    pub fn is_gone(&self, piece: &FeedbackId) -> bool {
        self.gone.contains_key(piece)
    }

    /// Whether anything given on the work changed or went, or may have been missed: replies alone do not.
    pub fn touches_the_work(&self) -> bool {
        self.missed
            || self.changed.values().any(|on| on_the_work(on))
            || self.gone.values().any(|on| on_the_work(on))
    }

    fn told(&mut self, frame: FrameDTO) {
        let Some(piece) = frame.of.map(FeedbackId::from) else {
            return;
        };
        match serde_json::from_value::<GivenFeedbackChangeDTO>(frame.event) {
            Ok(GivenFeedbackChangeDTO::Changed { subject }) => {
                self.changed.insert(piece, subject);
            }
            Ok(GivenFeedbackChangeDTO::Gone { subject }) => {
                self.changed.remove(&piece);
                self.gone.insert(piece, subject);
            }
            Err(_) => {}
        }
    }
}

/// The pieces held whose subject is as asked.
fn on(held: &HashMap<FeedbackId, String>, asked: impl Fn(&str) -> bool) -> Vec<FeedbackId> {
    held.iter()
        .filter(|(_, on)| asked(on))
        .map(|(piece, _)| piece.clone())
        .collect()
}

/// Whether a subject is something of the work, not a piece of feedback whose replies are given on it.
fn on_the_work(subject: &str) -> bool {
    !matches!(
        SubjectDTO::listed(subject).as_deref(),
        Ok([SubjectDTO::Feedback(_)])
    )
}

/// The feedback given in the project open now, followed live.
/// What is heard is gathered while it settles, then handed on together to whatever shows feedback,
/// which takes in the pieces changed and lets go of those gone.
#[derive(Clone, Copy)]
pub struct FollowedFeedback {
    project: StoredValue<Option<String>>,
    gathered: StoredValue<Heard>,
    heard: RwSignal<Heard>,
    /// Moves on with what is heard where it touches the work, so what counts it asks again only then.
    work_changed: RwSignal<u64>,
    settling: StoredValue<Option<TimeoutHandle>, LocalStorage>,
    following: StoredValue<Option<Subscription>, LocalStorage>,
}

impl FollowedFeedback {
    pub fn none() -> Self {
        Self {
            project: StoredValue::new(None),
            gathered: StoredValue::new(Heard::default()),
            heard: RwSignal::new(Heard::default()),
            work_changed: RwSignal::new(0),
            settling: StoredValue::new_local(None),
            following: StoredValue::new_local(None),
        }
    }

    /// Follows the feedback given in a project, unless it is followed already; the project followed before is let go.
    pub fn follow(self, project: &str) {
        if self
            .project
            .with_value(|followed| followed.as_deref() == Some(project))
        {
            return;
        }
        self.project.set_value(Some(project.to_owned()));
        self.following.set_value(None);
        self.gathered.set_value(Heard::default());
        let opened_before = StoredValue::new(false);
        self.following.set_value(Some(Subscription::start(
            "feedback",
            &[("project", project)],
            move |frame| self.gathering(|heard| heard.told(frame)),
            move || {
                if opened_before.get_value() {
                    self.gathering(|heard| heard.missed = true);
                }
                opened_before.set_value(true);
            },
        )));
    }

    /// What was heard last, tracked, for whatever takes it in.
    pub fn heard(self) -> Heard {
        self.heard.get()
    }

    /// Tracks what is heard where it touches the work, for what counts the work's feedback.
    pub fn track_the_work(self) {
        self.work_changed.track();
    }

    fn gathering(self, told: impl FnOnce(&mut Heard)) {
        self.gathered.update_value(told);
        if self.settling.with_value(Option::is_some) {
            return;
        }
        let settled = move || {
            _ = self.settling.try_set_value(None);
            if let Some(heard) = self.gathered.try_update_value(std::mem::take) {
                if heard.touches_the_work() {
                    _ = self.work_changed.try_update(|changed| *changed += 1);
                }
                _ = self.heard.try_set(heard);
            }
        };
        self.settling.set_value(set_timeout(settled, SETTLING).ok());
    }
}

/// Takes the pieces asked for again into what is held: each found replaces the one held,
/// or is added where `adding` lets it; one asked for and not found any more is let go of.
pub fn taken_in(
    held: &mut Vec<Feedback>,
    asked: &[FeedbackId],
    found: Vec<Feedback>,
    adding: bool,
) {
    held.retain(|piece| !asked.contains(&piece.id) || found.iter().any(|one| one.id == piece.id));
    for piece in found {
        match held.iter_mut().find(|one| one.id == piece.id) {
            Some(one) => *one = piece,
            None if adding => held.push(piece),
            None => {}
        }
    }
}
