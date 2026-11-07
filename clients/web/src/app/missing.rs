use leptos::IntoView;
use leptos::html;
use leptos::prelude::*;

use crate::shell;
use crate::speech;

#[component]
pub fn MissingPage() -> impl IntoView {
    (
        shell::masthead(None),
        html::main().child(
            html::div()
                .class("column")
                .child((html::p().class("empty").child(speech::said("missing")),)),
        ),
    )
}
