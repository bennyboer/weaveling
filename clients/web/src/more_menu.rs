use leptos::prelude::*;
use leptos::{IntoView, ev, html, view};
use leptos_router::components::A;

use crate::accounts::{CurrentSession, SignInState};
use crate::icons;
use crate::icons::Icon;
use crate::route;
use crate::speech;
use crate::speech::picker;
use crate::theme;

pub fn more_menu(project: Option<String>, sharing: RwSignal<bool>) -> impl IntoView {
    let open = RwSignal::new(false);
    let session = expect_context::<CurrentSession>();
    let signed_in = Memo::new(move |_| matches!(session.state(), SignInState::SignedIn(_)));

    let clicking_away = window_event_listener(ev::click, move |_| open.set(false));
    let escaping = window_event_listener(ev::keydown, move |event| {
        if event.key() == "Escape" {
            open.set(false);
        }
    });
    on_cleanup(move || {
        clicking_away.remove();
        escaping.remove();
    });

    html::div()
        .class("more")
        .on(ev::click, |event| event.stop_propagation())
        .child((
            html::button()
                .r#type("button")
                .class("more-button")
                .attr("aria-label", speech::said("masthead-more"))
                .attr("title", speech::said("masthead-more"))
                .attr("aria-haspopup", "dialog")
                .attr("aria-expanded", move || open.get().to_string())
                .on(ev::click, move |_| open.update(|open| *open = !*open))
                .child(icons::mark(Icon::More)),
            move || {
                open.get().then(|| {
                    html::div()
                        .class("menu more-menu")
                        .role("dialog")
                        .attr("aria-label", speech::said("masthead-more"))
                        .child((
                            signed_in
                                .get()
                                .then(|| project.clone())
                                .flatten()
                                .map(|project| this_project(project, open, sharing)),
                            html::section()
                                .child((html::h2().child(speech::said("theme")), theme::picker())),
                            html::section().child((
                                html::h2().child(speech::said("language")),
                                picker::picker(),
                            )),
                        ))
                })
            },
        ))
}

fn this_project(project: String, open: RwSignal<bool>, sharing: RwSignal<bool>) -> impl IntoView {
    html::section().class("this-project").child((
        html::h2().child(speech::said("masthead-this-project")),
        html::div().class("actions").child((
            view! {
                <A href=route::members(&project) on:click=move |_| open.set(false)>
                    {icons::mark(Icon::People)}
                    <span>{speech::said("members-link")}</span>
                </A>
            },
            html::button()
                .r#type("button")
                .on(ev::click, move |_| {
                    open.set(false);
                    sharing.set(true);
                })
                .child((
                    icons::mark(Icon::Share),
                    html::span().child(speech::said("share")),
                )),
        )),
    ))
}
