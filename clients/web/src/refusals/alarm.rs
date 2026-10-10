use std::collections::BTreeSet;
use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::{IntoView, ev, html};

use crate::icons::{Icon, mark};
use crate::refusals::dialog::refusals_dialog;
use crate::refusals::model::Refusal;
use crate::refusals::service;

const ASK_EVERY: Duration = Duration::from_secs(5);

#[derive(Clone, Copy)]
pub struct AlarmState {
    refusals: RwSignal<Vec<Refusal>>,
    heard: StoredValue<BTreeSet<i64>>,
    pulsing: RwSignal<bool>,
    looking: RwSignal<bool>,
}

impl AlarmState {
    pub fn new() -> Self {
        let state = Self {
            refusals: RwSignal::new(Vec::new()),
            heard: StoredValue::new(BTreeSet::new()),
            pulsing: RwSignal::new(false),
            looking: RwSignal::new(false),
        };

        state.ask();
        if let Err(failure) = set_interval(move || state.ask(), ASK_EVERY) {
            leptos::logging::error!("the alarm cannot ask again: {failure:?}");
        }

        state
    }

    fn ask(self) {
        spawn_local(async move {
            if let Ok(found) = service::all().await {
                self.arrived(found);
            }
        });
    }

    fn arrived(self, found: Vec<Refusal>) {
        let fresh = self.heard.with_value(|heard| {
            found
                .iter()
                .any(|refusal| !refusal.acknowledged && !heard.contains(&refusal.id))
        });

        self.heard
            .update_value(|heard| heard.extend(found.iter().map(|refusal| refusal.id)));
        if fresh {
            self.pulsing.set(true);
        }
        self.refusals.set(found);
    }

    pub fn refusals(self) -> Vec<Refusal> {
        self.refusals.get()
    }

    fn any(self) -> bool {
        self.refusals.with(|refusals| !refusals.is_empty())
    }

    fn raised(self) -> bool {
        self.refusals
            .with(|refusals| refusals.iter().any(|refusal| !refusal.acknowledged))
    }

    fn is_pulsing(self) -> bool {
        self.pulsing.get()
    }

    fn stop_pulsing(self) {
        self.pulsing.set(false);
    }

    fn is_looking(self) -> bool {
        self.looking.get()
    }

    fn start_looking(self) {
        self.looking.set(true);
    }

    pub fn stop_looking(self) {
        self.looking.set(false);
    }

    pub fn retry(self, refusal: i64) {
        spawn_local(async move {
            if service::retry(refusal).await.is_ok() {
                self.ask();
            }
        });
    }

    pub fn acknowledge(self, refusal: i64) {
        spawn_local(async move {
            if service::acknowledge(refusal).await.is_ok() {
                self.ask();
            }
        });
    }
}

pub fn alarm(state: AlarmState) -> impl IntoView {
    (
        move || {
            state.any().then(|| {
                html::button()
                    .r#type("button")
                    .class("alarm")
                    .class(("raised", move || state.raised()))
                    .class(("pulsing", move || state.is_pulsing()))
                    .attr("aria-label", "Changes that could not be applied")
                    .on(ev::click, move |_| state.start_looking())
                    .on(ev::animationend, move |_| state.stop_pulsing())
                    .child(mark(Icon::Flash))
            })
        },
        move || state.is_looking().then(|| refusals_dialog(state)),
    )
}
