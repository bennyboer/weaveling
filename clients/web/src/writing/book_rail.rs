use std::collections::HashMap;

use leptos::prelude::*;
use web_sys::Storage;

use crate::outline::model::SectionId;
use crate::writing::drawer_width::DrawerWidth;

const KEPT_OPEN: &str = "weaveling.book-rail";
const OPEN: &str = "open";

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
    open: RwSignal<bool>,
    unfolded: RwSignal<Unfolded>,
    turned: RwSignal<HashMap<SectionId, Fold>>,
    width: DrawerWidth,
}

impl BookRail {
    pub fn as_last_left() -> Self {
        let open = shelf()
            .and_then(|shelf| shelf.get_item(KEPT_OPEN).ok().flatten())
            .is_some_and(|kept| kept == OPEN);

        Self {
            open: RwSignal::new(open),
            unfolded: RwSignal::new(Unfolded::WhereNested),
            turned: RwSignal::new(HashMap::new()),
            width: DrawerWidth::as_last_left("weaveling.book-rail.width", "--book-rail"),
        }
    }

    pub fn width(self) -> DrawerWidth {
        self.width
    }

    pub fn is_open(self) -> bool {
        self.open.get()
    }

    pub fn toggle(self) {
        self.keep_open(!self.open.get_untracked());
    }

    pub fn close(self) {
        self.keep_open(false);
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

    fn keep_open(self, open: bool) {
        self.open.set(open);

        if let Some(shelf) = shelf() {
            _ = match open {
                true => shelf.set_item(KEPT_OPEN, OPEN),
                false => shelf.remove_item(KEPT_OPEN),
            };
        }
    }
}

fn shelf() -> Option<Storage> {
    window().local_storage().ok().flatten()
}
