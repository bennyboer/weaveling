use leptos::ev::MouseEvent;
use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::speech;

/// The frame of a page made of cards.
/// It puts the heading on top and the content below it.
/// A lead is a short sentence under the heading that says what the page is for.
/// Tabs lead to the other parts of the same page. They stand beside the heading, or below it on a phone.
/// The style class names the page, so its own styles can find it.
pub fn page(
    style_class: &'static str,
    heading: impl IntoView + 'static,
    lead: Option<&'static str>,
    tabs: impl IntoView + 'static,
    content: impl IntoView + 'static,
) -> impl IntoView {
    html::main().class(format!("page {style_class}")).child((
        html::header().class("page-head").child((
            html::div().class("titles").child((
                html::h1().child(heading),
                lead.map(|lead| html::p().class("lead").child(speech::said(lead))),
            )),
            tabs,
        )),
        content,
    ))
}

/// Links to the parts of a page, shown as pills.
/// The link to the part shown now is marked, as the router marks the current link.
/// The label names the links for screen readers.
pub fn tabs(label: &'static str, links: impl IntoView + 'static) -> impl IntoView {
    html::nav()
        .class("tabs")
        .attr("aria-label", move || speech::say(label))
        .child(links)
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

/// A card with a sentence that says how much is used, its bar, and a hint.
/// The hint stands beside the usage on a wide window and below it on a phone.
pub fn usage_card(usage: impl IntoView + 'static, hint: impl IntoView + 'static) -> impl IntoView {
    card("usage-card", (usage, html::p().class("hint").child(hint)))
}

/// A card that holds a list.
/// Its head shows the heading, with tools such as sorting or searching beside it.
/// On a phone the tools go below the heading.
/// The body holds the rows, or a hint when there are none.
/// The foot holds what comes after the rows, such as "Show more".
pub fn list_card(
    style_class: &'static str,
    heading: impl IntoView + 'static,
    tools: impl IntoView + 'static,
    body: impl IntoView + 'static,
    foot: impl IntoView + 'static,
) -> impl IntoView {
    html::section()
        .class(format!("card list-card {style_class}"))
        .child((
            html::header().class("list-head").child((
                html::h2().child(heading),
                html::div().class("tools").child(tools),
            )),
            body,
            foot,
        ))
}

/// The rows of a list card.
pub fn rows(items: impl IntoView + 'static) -> impl IntoView {
    html::ul().class("list-rows").child(items)
}

/// One row of a list card.
/// The leading part is a picture or an initial.
/// Next to it are the name and a detail line.
/// Marks are short words about the row, like "read only".
/// They stand beside the actions on a wide window and under the detail on a phone.
pub fn row(
    leading: impl IntoView + 'static,
    name: impl IntoView + 'static,
    detail: impl IntoView + 'static,
    marks: impl IntoView + 'static,
    actions: impl IntoView + 'static,
) -> impl IntoView {
    html::li().class("list-row").child((
        html::div().class("leading").child(leading),
        html::div().class("text").child((
            html::p().class("name").child(name),
            html::p().class("detail").child(detail),
        )),
        html::div().class("marks").child(marks),
        html::div().class("list-actions").child(actions),
    ))
}

/// A short word about a row, shown in a small rounded frame.
pub fn mark(said: impl IntoView + 'static) -> impl IntoView {
    html::span().class("mark").child(said)
}

/// The button at the foot of a list card that loads the next rows.
/// It cannot be pressed while rows are loading.
pub fn more(
    label: &'static str,
    loading: impl Fn() -> bool + Send + Sync + 'static,
    next: impl FnMut(MouseEvent) + 'static,
) -> impl IntoView {
    html::div().class("list-foot").child(
        html::button()
            .r#type("button")
            .class("more")
            .disabled(loading)
            .on(ev::click, next)
            .child(speech::said(label)),
    )
}
