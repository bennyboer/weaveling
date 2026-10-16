use leptos::prelude::*;
use leptos::{IntoView, html};

use crate::projects::open_project::OpenProject;
use crate::writing::write_view_state::WriteViewState;

const FOCUSED: &str = "focused";

#[derive(Clone, Copy)]
pub struct Focus {
    on: RwSignal<bool>,
}

impl Focus {
    pub fn off() -> Self {
        let focus = Self {
            on: RwSignal::new(false),
        };

        Effect::new(move || {
            let on = focus.on.get();
            if let Some(body) = document().body() {
                _ = body.class_list().toggle_with_force(FOCUSED, on);
            }
        });
        on_cleanup(|| {
            if let Some(body) = document().body() {
                _ = body.class_list().remove_1(FOCUSED);
            }
        });

        focus
    }

    pub fn is_on(self) -> bool {
        self.on.get()
    }

    pub fn toggle(self) {
        self.on.update(|on| *on = !*on);
    }
}

pub fn focus_line(state: WriteViewState) -> impl IntoView {
    let open_project = expect_context::<OpenProject>();

    move || {
        state.focus.is_on().then(|| {
            let section = move || {
                state
                    .writing_in
                    .section()
                    .and_then(|section| state.structure.title_of(&section))
                    .filter(|title| !title.trim().is_empty())
            };

            html::nav()
                .class("focus-line")
                .attr("aria-label", "Where you are")
                .child((
                    html::span()
                        .class("project")
                        .child(move || open_project.name().unwrap_or_default()),
                    move || {
                        section().map(|title| {
                            (
                                html::span().class("between").child("\u{203a}"),
                                html::span().class("section").child(title),
                            )
                        })
                    },
                ))
        })
    }
}
