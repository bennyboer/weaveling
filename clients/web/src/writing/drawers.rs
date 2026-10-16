use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::icons::{Icon, mark};

use crate::writing::book_rail::BookRail;
use crate::writing::contents::BOOK_RAIL;
use crate::writing::drawer_width::{BOOK_LEFT_IN_SIGHT, DrawerWidth, NARROWEST};
use crate::writing::scene_tray::SceneTray;
use crate::writing::side::Side;
use crate::writing::this_scene::SCENE_TRAY;
use crate::writing::write_view_state::WriteViewState;

const LYING_OVER_THE_BOOK: f64 = 960.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Drawer {
    Rail,
    Tray,
}

pub fn keep_apart(rail: BookRail, tray: SceneTray) {
    Effect::new(move |before: Option<(bool, bool, Drawer)>| {
        let (rail_open, tray_open) = (rail.is_open(), tray.is_open());
        let latest = match before {
            Some((false, _, _)) if rail_open => Drawer::Rail,
            Some((_, false, _)) if tray_open => Drawer::Tray,
            Some((_, _, latest)) => latest,
            None => Drawer::Tray,
        };
        let (rail_width, tray_width) = (rail.width(), tray.width());

        if !(rail_open && tray_open) {
            rail_width.share(None);
            tray_width.share(None);
            return (rail_open, tray_open, latest);
        }

        let window = rail_width.window();
        let room = window - BOOK_LEFT_IN_SIGHT;
        if window <= LYING_OVER_THE_BOOK || room < 2.0 * NARROWEST {
            match latest {
                Drawer::Rail => tray.close(),
                Drawer::Tray => rail.close(),
            }
            return (rail_open, tray_open, latest);
        }

        let (first, giving_way): (DrawerWidth, DrawerWidth) = match latest {
            Drawer::Rail => (tray_width, rail_width),
            Drawer::Tray => (rail_width, tray_width),
        };
        let kept = Some(room - NARROWEST);
        first.share(kept);
        giving_way.share(Some(room - first.shown_in(kept)));

        (rail_open, tray_open, latest)
    });
}

pub fn drawer_toggles(state: WriteViewState) -> impl IntoView {
    let (rail, tray) = (state.book_rail, state.scene_tray);

    html::div().class("drawer-toggles").child((
        (state.side == Side::Book).then(|| {
            html::button()
                .r#type("button")
                .attr("aria-label", "Contents")
                .attr("title", "Contents")
                .attr("aria-controls", BOOK_RAIL)
                .attr("aria-expanded", move || rail.is_open().to_string())
                .on(ev::click, move |_| rail.toggle())
                .child(mark(Icon::Section))
        }),
        html::button()
            .r#type("button")
            .attr("aria-label", "This scene")
            .attr("title", "This scene")
            .attr("aria-controls", SCENE_TRAY)
            .attr("aria-expanded", move || tray.is_open().to_string())
            .on(ev::click, move |_| tray.toggle())
            .child(mark(Icon::Scene)),
    ))
}
