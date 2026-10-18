use std::fmt::Display;

use leptos::prelude::*;
use leptos::{IntoView, ev, html};

pub fn problem(failure: impl Display) -> impl IntoView {
    html::p()
        .class("problem")
        .role("alert")
        .child(failure.to_string())
}

pub fn problem_to_dismiss(
    failure: impl Display,
    dismiss: impl Fn() + Send + Sync + 'static,
) -> impl IntoView {
    html::p().class("problem").role("alert").child((
        failure.to_string(),
        html::button()
            .r#type("button")
            .class("dismiss")
            .attr("aria-label", "Dismiss")
            .on(ev::click, move |_| dismiss())
            .child("\u{00d7}"),
    ))
}
