use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::dialog::modal;
use crate::moment::human_time;
use crate::refusals::alarm::AlarmState;
use crate::refusals::model::Refusal;
use crate::speech::{said, say, say_about};

pub fn refusals_dialog(state: AlarmState) -> impl IntoView {
    modal(
        Signal::derive(said("refusals")),
        "refusals",
        move || state.stop_looking(),
        (
            html::p().class("tagline").child(said("refusals-told")),
            move || {
                let refusals = state.refusals();

                if refusals.is_empty() {
                    return html::p()
                        .class("empty")
                        .child(said("refusals-none"))
                        .into_any();
                }

                html::ul()
                    .class("refused")
                    .attr("aria-label", said("refusals-list"))
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
                    .child(said("close")),
            ),
        ),
    )
}

fn refused(refusal: Refusal, state: AlarmState) -> impl IntoView {
    let id = refusal.id;

    html::li()
        .class(("acknowledged", refusal.acknowledged))
        .child((
            html::p()
                .class("plainly")
                .child(refusal.plainly.unwrap_or_else(|| say("refusal-wordless"))),
            html::p().class("when").child(say_about(
                "refusal-given-up",
                "when",
                &human_time(refusal.given_up_at),
            )),
            html::details().child((
                html::summary().child(said("refusal-details")),
                html::dl().child((
                    html::dt().child(said("refusal-listener")),
                    html::dd().child(refusal.listener),
                    html::dt().child(said("refusal-message")),
                    html::dd().child(refusal.routing),
                    html::dt().child(said("refusal-attempts")),
                    html::dd().child(refusal.attempts),
                    html::dt().child(said("refusal-reason")),
                    html::dd().child(html::code().child(refusal.why)),
                )),
            )),
            html::div().class("dialog-actions").child((
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| state.retry(id))
                    .child(said("refusal-retry")),
                (!refusal.acknowledged).then(|| {
                    html::button()
                        .r#type("button")
                        .on(ev::click, move |_| state.acknowledge(id))
                        .child(said("refusal-acknowledge"))
                }),
            )),
        ))
}
