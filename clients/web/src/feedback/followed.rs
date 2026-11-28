use std::collections::{HashMap, HashSet};
use std::time::Duration;

use feedback_contract::GivenFeedbackChangeDTO;
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
    pub gone: HashSet<FeedbackId>,
    /// Whether changes may have been missed, as the following was taken up again after a break,
    /// so what is shown is asked for anew.
    pub missed: bool,
}

impl Heard {
    /// The pieces that changed on one subject.
    pub fn changed_on(&self, subject: &str) -> Vec<FeedbackId> {
        self.changed
            .iter()
            .filter(|(_, on)| on.as_str() == subject)
            .map(|(piece, _)| piece.clone())
            .collect()
    }

    fn told(&mut self, frame: FrameDTO) {
        let Some(piece) = frame.of.map(FeedbackId::from) else {
            return;
        };
        match serde_json::from_value::<GivenFeedbackChangeDTO>(frame.event) {
            Ok(GivenFeedbackChangeDTO::Changed { subject }) => {
                self.changed.insert(piece, subject);
            }
            Ok(GivenFeedbackChangeDTO::Gone { .. }) => {
                self.changed.remove(&piece);
                self.gone.insert(piece);
            }
            Err(_) => {}
        }
    }
}

/// The feedback given in the project open now, followed live.
/// What is heard is gathered while it settles, then handed on together to whatever shows feedback,
/// which takes in the pieces changed and lets go of those gone.
#[derive(Clone, Copy)]
pub struct FollowedFeedback {
    project: StoredValue<Option<String>>,
    gathered: StoredValue<Heard>,
    heard: RwSignal<Heard>,
    settling: StoredValue<Option<TimeoutHandle>, LocalStorage>,
    following: StoredValue<Option<Subscription>, LocalStorage>,
}

impl FollowedFeedback {
    pub fn none() -> Self {
        Self {
            project: StoredValue::new(None),
            gathered: StoredValue::new(Heard::default()),
            heard: RwSignal::new(Heard::default()),
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

    fn gathering(self, told: impl FnOnce(&mut Heard)) {
        self.gathered.update_value(told);
        if self.settling.with_value(Option::is_some) {
            return;
        }
        let settled = move || {
            _ = self.settling.try_set_value(None);
            if let Some(heard) = self.gathered.try_update_value(std::mem::take) {
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
