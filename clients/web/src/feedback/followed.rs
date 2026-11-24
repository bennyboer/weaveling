use std::time::Duration;

use leptos::prelude::*;

use crate::live::Subscription;

/// How long after a change is heard whatever shows feedback asks again:
/// changes heard close together, as when many drafts are submitted at once, are asked for once,
/// and a piece you marked done yourself has finished moving to where it now belongs before it is shown afresh.
const SETTLING: Duration = Duration::from_millis(1200);

/// The feedback given in the project open now, followed live.
/// It moves on whenever a piece given there changed or went, by anyone,
/// and whenever the following is taken up again after a break, in which changes may have been missed;
/// whatever shows feedback tracks it and asks again.
#[derive(Clone, Copy)]
pub struct FollowedFeedback {
    project: StoredValue<Option<String>>,
    changed: RwSignal<u64>,
    settling: StoredValue<Option<TimeoutHandle>, LocalStorage>,
    following: StoredValue<Option<Subscription>, LocalStorage>,
}

impl FollowedFeedback {
    pub fn none() -> Self {
        Self {
            project: StoredValue::new(None),
            changed: RwSignal::new(0),
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
        let opened_before = StoredValue::new(false);
        self.following.set_value(Some(Subscription::start(
            "feedback",
            &[("project", project)],
            move |_| self.heard(),
            move || {
                if opened_before.get_value() {
                    self.heard();
                }
                opened_before.set_value(true);
            },
        )));
    }

    /// Tracks the feedback given changing, for whatever asks again when it does.
    pub fn track(self) {
        self.changed.track();
    }

    fn heard(self) {
        if self.settling.with_value(Option::is_some) {
            return;
        }
        let settled = move || {
            _ = self.settling.try_set_value(None);
            _ = self.changed.try_update(|changed| *changed += 1);
        };
        self.settling.set_value(set_timeout(settled, SETTLING).ok());
    }
}
