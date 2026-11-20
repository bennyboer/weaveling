use leptos::prelude::*;

use crate::drawer::Drawer;
use crate::drawer::width::{DrawerWidth, NARROWEST, PAGE_LEFT_IN_SIGHT};

const LYING_OVER_THE_BOOK: f64 = 960.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Opened {
    Rail,
    Tray,
}

/// Keeps a drawer on the left and one on the right from lying over each other.
/// Where both are open they share the room beside what is left of the page in sight, the one opened last giving way;
/// where there is no room for both, the one opened first closes.
pub fn keep_apart(rail: Drawer, tray: Drawer) {
    Effect::new(move |before: Option<(bool, bool, Opened)>| {
        let (rail_open, tray_open) = (rail.is_open(), tray.is_open());
        let latest = match before {
            Some((false, _, _)) if rail_open => Opened::Rail,
            Some((_, false, _)) if tray_open => Opened::Tray,
            Some((_, _, latest)) => latest,
            None => Opened::Tray,
        };
        let (rail_width, tray_width) = (rail.width(), tray.width());

        if !(rail_open && tray_open) {
            rail_width.share(None);
            tray_width.share(None);
            return (rail_open, tray_open, latest);
        }

        let window = rail_width.window();
        let room = window - PAGE_LEFT_IN_SIGHT;
        if window <= LYING_OVER_THE_BOOK || room < 2.0 * NARROWEST {
            match latest {
                Opened::Rail => tray.close(),
                Opened::Tray => rail.close(),
            }
            return (rail_open, tray_open, latest);
        }

        let (first, giving_way): (DrawerWidth, DrawerWidth) = match latest {
            Opened::Rail => (tray_width, rail_width),
            Opened::Tray => (rail_width, tray_width),
        };
        let kept = Some(room - NARROWEST);
        first.share(kept);
        giving_way.share(Some(room - first.shown_in(kept)));

        (rail_open, tray_open, latest)
    });
}
