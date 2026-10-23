use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::dialog;
use crate::moment;
use crate::refusals::alarm::AlarmState;
use crate::refusals::model::Refusal;
use crate::speech;

pub fn refusals_dialog(state: AlarmState) -> impl IntoView {
    dialog::modal(
        Signal::derive(speech::said("refusals")),
        "refusals",
        move || state.stop_looking(),
        (
            html::p()
                .class("tagline")
                .child(speech::said("refusals-told")),
            move || {
                let refusals = state.refusals();

                if refusals.is_empty() {
                    return html::p()
                        .class("empty")
                        .child(speech::said("refusals-none"))
                        .into_any();
                }

                html::ul()
                    .class("refused")
                    .attr("aria-label", speech::said("refusals-list"))
                    .child(
                        refusals
                            .into_iter()
                            .map(|refusal| refused(refusal, state))
                            .collect::<Vec<_>>(),
                    )
                    .into_any()
            },
            html::div().class("dialog-actions").child(
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| state.stop_looking())
                    .child(speech::said("close")),
            ),
        ),
    )
}

fn refused(refusal: Refusal, state: AlarmState) -> impl IntoView {
    let id = refusal.id;

    html::li()
        .class(("acknowledged", refusal.acknowledged))
        .child((
            html::p().class("plainly").child(
                refusal
                    .plainly
                    .unwrap_or_else(|| speech::say("refusal-wordless")),
            ),
            html::p().class("when").child(speech::say_about(
                "refusal-given-up",
                "when",
                &moment::human_time(refusal.given_up_at),
            )),
            html::details().child((
                html::summary().child(speech::said("refusal-details")),
                html::dl().child((
                    html::dt().child(speech::said("refusal-listener")),
                    html::dd().child(refusal.listener),
                    html::dt().child(speech::said("refusal-message")),
                    html::dd().child(refusal.routing),
                    html::dt().child(speech::said("refusal-attempts")),
                    html::dd().child(refusal.attempts),
                    html::dt().child(speech::said("refusal-reason")),
                    html::dd().child(html::code().child(refusal.why)),
                )),
            )),
            html::div().class("dialog-actions").child((
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| state.retry(id))
                    .child(speech::said("refusal-retry")),
                (!refusal.acknowledged).then(|| {
                    html::button()
                        .r#type("button")
                        .on(ev::click, move |_| state.acknowledge(id))
                        .child(speech::said("refusal-acknowledge"))
                }),
            )),
        ))
}
