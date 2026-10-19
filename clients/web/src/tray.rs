use leptos::html;
use leptos::prelude::*;

use crate::drawer::{Drawer, Edge, Inside, drawer};

const WIDE: f64 = 960.0;

pub fn laid_out(
    work: AnyView,
    kept: AnyView,
    tally: impl Fn() -> String + Send + Sync + 'static,
) -> impl IntoView {
    let wide = window()
        .inner_width()
        .ok()
        .and_then(|width| width.as_f64())
        .is_some_and(|width| width > WIDE);
    let tray = Drawer::as_last_left("weaveling.tray", wide);

    html::div().class("laid-out").child((
        work,
        drawer(
            tray,
            Edge::Right,
            tally,
            Inside {
                id: "tray",
                named: "Tray".into(),
                called: "the tray",
                head: (),
                body: kept,
            },
        ),
    ))
}
