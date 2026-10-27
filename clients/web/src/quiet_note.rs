use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::icons;
use crate::icons::Icon;
use crate::speech;

#[derive(Clone, Copy)]
pub struct QuietNote {
    told: RwSignal<Option<String>>,
}

impl QuietNote {
    pub fn none() -> Self {
        Self {
            told: RwSignal::new(None),
        }
    }

    pub fn tell(self, saying: String) {
        self.told.set(Some(saying));
    }

    pub fn dismiss(self) {
        self.told.set(None);
    }

    pub fn told(self) -> Option<String> {
        self.told.get()
    }
}

pub fn quiet_note() -> impl IntoView {
    let note = expect_context::<QuietNote>();

    move || {
        note.told().map(|saying| {
            html::div().class("quiet-note").role("status").child((
                html::span().child(saying),
                html::button()
                    .r#type("button")
                    .attr("aria-label", speech::said("problem-dismiss"))
                    .attr("title", speech::said("problem-dismiss"))
                    .on(ev::click, move |_| note.dismiss())
                    .child(icons::mark(Icon::Remove)),
            ))
        })
    }
}
