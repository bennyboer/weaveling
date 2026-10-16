use std::collections::HashMap;

use leptos::prelude::*;

use crate::drawer::Drawer;
use crate::outline::model::SectionId;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Fold {
    Folded,
    Outlined,
    Open,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Unfolded {
    WhereNested,
    Everything,
    Nothing,
}

#[derive(Clone, Copy)]
pub struct BookRail {
    drawer: Drawer,
    unfolded: RwSignal<Unfolded>,
    turned: RwSignal<HashMap<SectionId, Fold>>,
}

impl BookRail {
    pub fn as_last_left() -> Self {
        Self {
            drawer: Drawer::as_last_left("weaveling.book-rail", false),
            unfolded: RwSignal::new(Unfolded::WhereNested),
            turned: RwSignal::new(HashMap::new()),
        }
    }

    pub fn drawer(self) -> Drawer {
        self.drawer
    }

    pub fn fold_of(self, section: &SectionId, nests: bool) -> Fold {
        let turned = self.turned.with(|turned| turned.get(section).copied());

        turned.unwrap_or_else(|| match self.unfolded.get() {
            Unfolded::WhereNested if nests => Fold::Outlined,
            Unfolded::WhereNested | Unfolded::Nothing => Fold::Folded,
            Unfolded::Everything => Fold::Open,
        })
    }

    pub fn turn(self, section: SectionId, from: Fold) {
        let to = match from {
            Fold::Folded => Fold::Open,
            Fold::Outlined | Fold::Open => Fold::Folded,
        };

        self.turned.update(|turned| _ = turned.insert(section, to));
    }

    pub fn unfold_all(self) {
        self.turned.update(HashMap::clear);
        self.unfolded.set(Unfolded::Everything);
    }

    pub fn fold_all(self) {
        self.turned.update(HashMap::clear);
        self.unfolded.set(Unfolded::Nothing);
    }
}
