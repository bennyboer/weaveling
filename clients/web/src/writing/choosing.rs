use std::collections::HashSet;

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

use crate::writing::loose_scenes::LooseScenes;
use crate::writing::side::Side;
use crate::writing::structure::Structure;
use crate::writing::walk;
use crate::writing::walk::{Stop, Towards};

#[derive(Clone, Copy)]
pub struct Choosing {
    structure: Structure,
    loose: LooseScenes,
    side: Side,
    ends: RwSignal<Option<(Stop, Stop)>>,
    span: Memo<Option<(Stop, Stop)>>,
    chosen: Memo<HashSet<Stop>>,
}

impl Choosing {
    pub fn nothing(structure: Structure, loose: LooseScenes, side: Side) -> Self {
        let ends = RwSignal::new(None::<(Stop, Stop)>);
        let span = Memo::new(move |_| {
            let (anchor, head) = ends.get()?;
            let order = choosable(structure, loose, side);
            let from = order.iter().position(|stop| stop == &anchor)?;
            let to = order.iter().position(|stop| stop == &head)?;

            Some((order[from.min(to)].clone(), order[from.max(to)].clone()))
        });
        let chosen = Memo::new(move |_| {
            let Some((first, last)) = span.get() else {
                return HashSet::new();
            };
            let order = choosable(structure, loose, side);
            let (Some(from), Some(to)) = (
                order.iter().position(|stop| stop == &first),
                order.iter().position(|stop| stop == &last),
            ) else {
                return HashSet::new();
            };

            order[from..=to].iter().cloned().collect()
        });

        Self {
            structure,
            loose,
            side,
            ends,
            span,
            chosen,
        }
    }

    pub fn span(self) -> Option<(Stop, Stop)> {
        self.span.get()
    }

    pub fn in_reading_order(self) -> Vec<Stop> {
        let Some((first, last)) = self.span.get_untracked() else {
            return Vec::new();
        };
        let order = untrack(|| choosable(self.structure, self.loose, self.side));
        let (Some(from), Some(to)) = (
            order.iter().position(|stop| stop == &first),
            order.iter().position(|stop| stop == &last),
        ) else {
            return Vec::new();
        };

        order[from..=to].to_vec()
    }

    pub fn holds(self, stop: &Stop) -> bool {
        self.chosen.with(|chosen| chosen.contains(stop))
    }

    pub fn is_active(self) -> bool {
        self.ends.with_untracked(Option::is_some)
    }

    pub fn head(self) -> Option<Stop> {
        self.ends
            .with_untracked(|ends| ends.as_ref().map(|(_, head)| head.clone()))
    }

    pub fn begin_at(self, stop: Stop) {
        self.ends.set(Some((stop.clone(), stop)));
    }

    pub fn take_in_everything(self) {
        let order = untrack(|| choosable(self.structure, self.loose, self.side));

        if let (Some(first), Some(last)) = (order.first(), order.last()) {
            self.ends.set(Some((first.clone(), last.clone())));
        }
    }

    pub fn reach(self, towards: Towards) -> Option<Stop> {
        let (anchor, head) = self.ends.get_untracked()?;
        let order = untrack(|| choosable(self.structure, self.loose, self.side));
        let at = order.iter().position(|stop| stop == &head)?;
        let next = match towards {
            Towards::Before => at.checked_sub(1),
            Towards::After => Some(at + 1).filter(|next| *next < order.len()),
        }?;
        let reached = order[next].clone();

        self.ends.set(Some((anchor, reached.clone())));

        Some(reached)
    }

    pub fn reach_to(self, stop: Stop) {
        let Some((anchor, head)) = self.ends.get_untracked() else {
            return;
        };
        if head != stop {
            self.ends.set(Some((anchor, stop)));
        }
    }

    pub fn let_go(self) {
        if self.is_active() {
            self.ends.set(None);
        }
    }
}

pub fn hand_the_keys_to_the_book() {
    if let Some(book) = document()
        .query_selector("article.writing")
        .ok()
        .flatten()
        .and_then(|book| book.dyn_into::<HtmlElement>().ok())
    {
        _ = book.focus();
    }
}

fn choosable(structure: Structure, loose: LooseScenes, side: Side) -> Vec<Stop> {
    match side {
        Side::Book => structure
            .read(walk::stops_in_reading_order)
            .unwrap_or_default()
            .into_iter()
            .filter(|stop| matches!(stop, Stop::Heading(_) | Stop::Scene(_)))
            .collect(),
        Side::Loose => loose.in_order().into_iter().map(Stop::Scene).collect(),
    }
}
