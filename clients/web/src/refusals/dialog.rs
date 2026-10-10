use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::moment::human_time;
use crate::refusals::alarm::AlarmState;
use crate::refusals::model::Refusal;

const WORDLESS: &str = "A change did not reach everywhere it should have.";

pub fn refusals_dialog(state: AlarmState) -> impl IntoView {
    window_event_listener(ev::keydown, move |event| {
        if event.key() == "Escape" {
            state.stop_looking();
        }
    });

    html::div()
        .class("overlay")
        .on(ev::click, move |_| state.stop_looking())
        .child(
            html::div()
                .class("dialog refusals")
                .role("dialog")
                .attr("aria-modal", "true")
                .attr("aria-label", "Changes that could not be applied")
                .on(ev::click, |event| event.stop_propagation())
                .child((
                    html::h2().child("Changes that could not be applied"),
                    html::p().class("tagline").child(
                        "These were saved, but did not reach everywhere they should have. \
                         Try again once whatever stopped them is fixed, or acknowledge them \
                         to quiet the alarm.",
                    ),
                    move || {
                        let refusals = state.refusals();

                        if refusals.is_empty() {
                            return html::p()
                                .class("empty")
                                .child("Nothing is left to look at.")
                                .into_any();
                        }

                        html::ul()
                            .class("refused")
                            .attr("aria-label", "Refused changes")
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
                            .child("Close"),
                    ),
                )),
        )
}

fn refused(refusal: Refusal, state: AlarmState) -> impl IntoView {
    let id = refusal.id;

    html::li()
        .class(("acknowledged", refusal.acknowledged))
        .child((
            html::p()
                .class("plainly")
                .child(refusal.plainly.unwrap_or_else(|| WORDLESS.to_owned())),
            html::p()
                .class("when")
                .child(format!("Given up on {}", human_time(refusal.given_up_at))),
            html::details().child((
                html::summary().child("Details"),
                html::dl().child((
                    html::dt().child("Listener"),
                    html::dd().child(refusal.listener),
                    html::dt().child("Message"),
                    html::dd().child(refusal.routing),
                    html::dt().child("Attempts"),
                    html::dd().child(refusal.attempts),
                    html::dt().child("Reason"),
                    html::dd().child(html::code().child(refusal.why)),
                )),
            )),
            html::div().class("dialog-actions").child((
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| state.retry(id))
                    .child("Try again"),
                (!refusal.acknowledged).then(|| {
                    html::button()
                        .r#type("button")
                        .on(ev::click, move |_| state.acknowledge(id))
                        .child("Acknowledge")
                }),
            )),
        ))
}
