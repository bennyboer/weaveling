use leptos::prelude::*;
use leptos::{IntoView, component, ev, html};

use crate::accounts::CurrentSession;
use crate::shell;
use crate::speech;

#[component]
pub fn UnreachablePage() -> impl IntoView {
    let session = expect_context::<CurrentSession>();

    (
        shell::masthead(None),
        html::main().class("signing-in").child((
            html::h1().child(speech::said("unreachable-heading")),
            html::p()
                .class("problem")
                .role("alert")
                .child(speech::said("problem-offline")),
            html::button()
                .r#type("button")
                .class("primary")
                .on(ev::click, move |_| session.ask_again())
                .child(speech::said("try-again")),
        )),
    )
}
