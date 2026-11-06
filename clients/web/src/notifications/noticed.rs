use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::task;
use leptos::{IntoView, ev, html};
use time::format_description::well_known::Rfc3339;

use crate::http::ApiError;
use crate::live::Subscription;
use crate::moment;
use crate::notifications::notice::{Notice, RoleChange};
use crate::notifications::service;
use crate::speech;
use crate::speech::Speech;
use crate::working_copy;
use crate::working_copy::{Versioned, WorkingCopies};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Intent {
    Dismiss,
}

#[derive(Clone, Copy)]
pub struct Noticed {
    open: WorkingCopies<String, RoleChange, Intent>,
    refused: RwSignal<Option<ApiError>>,
}

impl Noticed {
    pub fn following() -> Self {
        let refused = RwSignal::new(None);
        let open = WorkingCopies::new(
            move |id: String, on: RoleChange, intent: Intent| async move {
                match intent {
                    Intent::Dismiss => match service::dismiss(&id, on.version).await {
                        Ok(()) | Err(ApiError::NotFound(_)) => {}
                        Err(failed) => return Err(failed),
                    },
                }

                Ok(on)
            },
            move |id: String| async move {
                service::open()
                    .await?
                    .into_iter()
                    .find(|change| change.id == id)
                    .map(|change| held(change).1)
                    .ok_or(ApiError::NotFound("notice"))
            },
            move |failure| _ = refused.try_set(Some(failure)),
        );
        let listing = move || {
            let reading = open.read_all(
                || async { Ok(service::open().await?.into_iter().map(held).collect()) },
                None,
            );
            task::spawn_local(async move {
                _ = reading.await;
            });
        };
        listing();
        StoredValue::new_local(Subscription::start(
            "notifications",
            &[("mine", "true")],
            move |_| listing(),
            listing,
        ));

        Self { open, refused }
    }

    pub fn notices(self) -> Vec<Notice> {
        self.open
            .shown()
            .into_iter()
            .map(Notice::RoleChanged)
            .collect()
    }

    pub fn refused(self) -> Option<ApiError> {
        self.refused.get()
    }

    fn dismiss(self, notice: String) {
        self.refused.set(None);
        let ending = self.open.ending(&notice, Intent::Dismiss);
        task::spawn_local(async move {
            ending.await;
        });
    }

    fn dismissing(self, notice: &String) -> Option<&'static str> {
        self.open.unanswered_on(notice).then_some("true")
    }
}

impl Versioned for RoleChange {
    fn version(&self) -> u64 {
        self.version
    }
}

impl working_copy::Intent<RoleChange> for Intent {
    fn lay_over(&self, _: &mut RoleChange) {}

    fn merged_after(&self, earlier: &Self) -> Option<Self> {
        (self == earlier).then(|| self.clone())
    }
}

fn held(change: RoleChange) -> (String, RoleChange) {
    (change.id.clone(), change)
}

pub fn row(noticed: Noticed, change: RoleChange) -> impl IntoView {
    let speech = expect_context::<Speech>();
    let told = {
        let change = change.clone();
        move || {
            let mut told = FluentArgs::new();
            told.set("role", change.role.named());
            told.set(
                "project",
                change
                    .project_name
                    .clone()
                    .unwrap_or_else(|| speech.say("notice-role-unnamed-project")),
            );
            match &change.by_name {
                Some(by) => {
                    told.set("by", by.clone());
                    speech.say_with("notice-role-changed-by", &told)
                }
                None => speech.say_with("notice-role-changed", &told),
            }
        }
    };
    let noted_at = change.noted_at;
    let (dismissed, busy) = (change.id.clone(), change.id);

    html::li().class("role-changed").child((
        html::div().class("told").child((
            html::p().child(told),
            html::time()
                .class("when")
                .attr("datetime", noted_at.format(&Rfc3339).unwrap_or_default())
                .child(moment::human_time(noted_at)),
        )),
        html::button()
            .r#type("button")
            .class("dismiss")
            .attr("aria-label", speech::said("problem-dismiss"))
            .attr("title", speech::said("problem-dismiss"))
            .attr("aria-busy", move || noticed.dismissing(&busy))
            .on(ev::click, move |_| noticed.dismiss(dismissed.clone()))
            .child(speech::said("problem-dismiss")),
    ))
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::*;
    use crate::members::listed::Role;
    use crate::working_copy::Intent as _;

    fn a_role_changed() -> RoleChange {
        held(RoleChange {
            id: "notice_1".to_owned(),
            version: 1,
            noted_at: datetime!(2026-10-30 09:00 UTC),
            role: Role::Author,
            project_name: Some("Shared".to_owned()),
            by_name: Some("Ada Lovelace".to_owned()),
        })
        .1
    }

    #[test]
    fn a_dismissal_on_its_way_changes_nothing_shown() {
        // given: a notice not yet dismissed
        let noted = a_role_changed();

        // when: its dismissal is laid over it
        let mut shown = noted.clone();
        Intent::Dismiss.lay_over(&mut shown);

        // then: it shows as it was
        assert_eq!(shown, noted);
    }
}
