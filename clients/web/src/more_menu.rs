use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::icons;
use crate::icons::Icon;
use crate::speech;
use crate::speech::picker;
use crate::theme;

pub fn more_menu() -> impl IntoView {
    let open = RwSignal::new(false);

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
