use leptos::prelude::*;
use leptos::{IntoView, html};

/// The frame of a page made of cards.
/// It puts the heading on top and the content below it.
/// The style class names the page, so its own styles can find it.
pub fn page(
    style_class: &'static str,
    heading: impl IntoView + 'static,
    content: impl IntoView + 'static,
) -> impl IntoView {
    html::main()
        .class(format!("operating page {style_class}"))
        .child((
            html::header()
                .class("page-head")
                .child(html::h1().child(heading)),
            content,
        ))
}

/// Cards laid out next to each other.
/// There are two columns on a wide window and one on a phone.
pub fn cards(content: impl IntoView + 'static) -> impl IntoView {
    html::div().class("cards").child(content)
}

/// One card: a framed box on the page.
/// The style class names what the card is about.
pub fn card(style_class: &'static str, content: impl IntoView + 'static) -> impl IntoView {
    html::section()
        .class(format!("card {style_class}"))
        .child(content)
}

/// The card for something that cannot be undone, like deleting.
/// It shows a heading and a hint on one side and the button on the other.
/// On a phone the button goes below the hint.
pub fn irreversible(
    style_class: &'static str,
    heading: impl IntoView + 'static,
    hint: impl IntoView + 'static,
    action: impl IntoView + 'static,
) -> impl IntoView {
    html::section()
        .class(format!("card irreversible {style_class}"))
        .child((
            html::div().child((
                html::h2().child(heading),
                html::p().class("hint").child(hint),
            )),
            action,
        ))
}

/// A sentence that says how much is used, with a bar below it.
/// The bar is only shown when there is a most.
pub fn usage(
    told: impl IntoView + 'static,
    used: u64,
    most: Option<u64>,
    label: impl Fn() -> String + Send + Sync + 'static,
) -> impl IntoView {
    html::div().class("usage").child((
        html::p().child(told),
        most.map(|most| {
            html::meter()
                .attr("min", 0)
                .attr("max", most as f64)
                .attr("value", used as f64)
                .attr("aria-label", label)
        }),
    ))
}
