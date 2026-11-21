use leptos::IntoView;
use leptos::html;
use leptos::prelude::*;
use leptos_router::components::A;

use crate::route;
use crate::shell;
use crate::speech;

#[component]
pub fn MissingPage() -> impl IntoView {
    not_found(|| speech::say("missing"))
}

/// A page for what is not there, or not any more: what it was, and the way back to one's projects,
/// with none of a project's views in the masthead, as there is nothing behind them.
pub fn not_found(told: impl Fn() -> String + Send + Sync + 'static) -> impl IntoView {
    (
        shell::masthead(None),
        html::main().child(
            html::div().class("column not-found").child((
                shell::mark(),
                html::h2().child(told),
                html::p()
                    .class("not-found-hint")
                    .child(speech::said("not-found-hint")),
                view! {
                    <A href=route::WORKSPACE attr:class="button primary">
                        {speech::said("not-found-to-projects")}
                    </A>
                },
            )),
        ),
    )
}
