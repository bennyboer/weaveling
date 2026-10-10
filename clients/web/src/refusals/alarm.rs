use std::collections::BTreeSet;
use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::{IntoView, view};

use crate::icons::{Icon, mark};
use crate::refusals::model::Refusal;
use crate::refusals::service;

const ASK_EVERY: Duration = Duration::from_secs(5);

#[derive(Clone, Copy)]
pub struct AlarmState {
    refusals: RwSignal<Vec<Refusal>>,
    heard: StoredValue<BTreeSet<i64>>,
    pulsing: RwSignal<bool>,
}

impl AlarmState {
    pub fn new() -> Self {
        let state = Self {
            refusals: RwSignal::new(Vec::new()),
            heard: StoredValue::new(BTreeSet::new()),
            pulsing: RwSignal::new(false),
        };
        let ask = move || {
            spawn_local(async move {
                if let Ok(found) = service::all().await {
                    state.arrived(found);
                }
            })
        };

        ask();
        if let Err(failure) = set_interval(ask, ASK_EVERY) {
            leptos::logging::error!("the alarm cannot ask again: {failure:?}");
        }

        state
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

    fn raised(self) -> bool {
        self.refusals
            .with(|refusals| refusals.iter().any(|refusal| !refusal.acknowledged))
    }
}

pub fn alarm(state: AlarmState) -> impl IntoView {
    move || {
        state.raised().then(|| {
            view! {
                <span
                    class="alarm"
                    class:pulsing=move || state.pulsing.get()
                    role="img"
                    aria-label="Some changes could not be applied"
                    on:animationend=move |_| state.pulsing.set(false)
                >
                    {mark(Icon::Flash)}
                </span>
            }
        })
    }
}
