use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::icons;
use crate::icons::Icon;
use crate::speech;
use crate::writing::contents::BOOK_RAIL;
use crate::writing::inspecting::INSPECTOR;
use crate::writing::side::Side;
use crate::writing::write_view_state::WriteViewState;

pub fn drawer_toggles(state: WriteViewState) -> impl IntoView {
    let (rail, tray) = (state.book_rail.drawer(), state.inspector.drawer());

    html::div().class("drawer-toggles").child((
        (state.side == Side::Book).then(|| {
            html::button()
                .r#type("button")
                .attr("aria-label", speech::said("contents"))
                .attr("title", speech::said("contents"))
                .attr("aria-controls", BOOK_RAIL)
                .attr("aria-expanded", move || rail.is_open().to_string())
                .on(ev::click, move |_| rail.toggle())
                .child(icons::mark(Icon::Section))
        }),
        html::button()
            .r#type("button")
            .attr("aria-label", speech::said("this-scene"))
            .attr("title", speech::said("this-scene"))
            .attr("aria-controls", INSPECTOR)
            .attr("aria-expanded", move || tray.is_open().to_string())
            .on(ev::click, move |_| tray.toggle())
            .child(icons::mark(Icon::Scene)),
    ))
}
