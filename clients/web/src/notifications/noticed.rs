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

#[derive(Clone, Copy)]
pub struct Noticed {
    open: RwSignal<Vec<RoleChange>>,
    refused: RwSignal<Option<ApiError>>,
}

impl Noticed {
    pub fn following() -> Self {
        let noticed = Self {
            open: RwSignal::new(Vec::new()),
            refused: RwSignal::new(None),
        };
        let latest = StoredValue::new(0_u64);
        let listing = Action::new_local(move |_: &()| {
            let asked = latest.get_value() + 1;
            latest.set_value(asked);

            async move {
                let answer = service::open().await;
                if latest.try_get_value() != Some(asked) {
                    return;
                }
                if let Ok(listed) = answer {
                    _ = noticed.open.try_set(listed);
                }
            }
        });
        listing.dispatch(());
        StoredValue::new_local(Subscription::start(
            "notifications",
            &[("mine", "true")],
            move |_| _ = listing.dispatch(()),
            move || _ = listing.dispatch(()),
        ));

        noticed
    }

    pub fn notices(self) -> Vec<Notice> {
        self.open
            .get()
            .into_iter()
            .map(Notice::RoleChanged)
            .collect()
    }

    pub fn refused(self) -> Option<ApiError> {
        self.refused.get()
    }

    fn dismiss(self, notice: String) {
        self.refused.set(None);
        task::spawn_local(async move {
            match service::dismiss(&notice).await {
                Ok(()) | Err(ApiError::NotFound(_)) => {
                    _ = self
                        .open
                        .try_update(|open| open.retain(|kept| kept.id != notice));
                }
                Err(failed) => _ = self.refused.try_set(Some(failed)),
            }
        });
    }
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
    let dismissed = change.id;

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
            .on(ev::click, move |_| noticed.dismiss(dismissed.clone()))
            .child(speech::said("problem-dismiss")),
    ))
}
