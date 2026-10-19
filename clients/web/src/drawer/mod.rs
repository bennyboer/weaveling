mod handle;
pub mod width;

use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::drawer::handle::{Grows, drawer_handle};
use crate::drawer::width::DrawerWidth;
use crate::icons::{Icon, mark};
use crate::kept::{keep, recall};

const OPEN: &str = "open";
const SHUT: &str = "shut";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Left,
    Right,
}

#[derive(Clone, Copy)]
pub struct Drawer {
    kept: StoredValue<String>,
    open: RwSignal<bool>,
    width: DrawerWidth,
}

impl Drawer {
    pub fn as_last_left(kept: &str, open_at_first: bool) -> Self {
        let open = recall(kept).map_or(open_at_first, |kept| kept == OPEN);

        Self {
            kept: StoredValue::new(kept.to_owned()),
            open: RwSignal::new(open),
            width: DrawerWidth::as_last_left(format!("{kept}.width")),
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

    fn keep_open(self, open: bool) {
        self.open.set(open);

        self.kept
            .with_value(|kept| keep(kept, if open { OPEN } else { SHUT }));
    }
}

pub struct Inside<Head, Body> {
    pub id: &'static str,
    pub named: TextProp,
    pub called: &'static str,
    pub head: Head,
    pub body: Body,
}

pub fn drawer<Head, Body>(
    drawer: Drawer,
    edge: Edge,
    tab: impl Fn() -> String + Send + Sync + 'static,
    inside: Inside<Head, Body>,
) -> impl IntoView
where
    Head: IntoView + 'static,
    Body: IntoView + 'static,
{
    let Inside {
        id,
        named,
        called,
        head,
        body,
    } = inside;
    let width = drawer.width();
    let along = match edge {
        Edge::Left => "left",
        Edge::Right => "right",
    };
    let edge_class = match edge {
        Edge::Left => "from-left",
        Edge::Right => "from-right",
    };
    let escaping = move |event: ev::KeyboardEvent| {
        if event.key() == "Escape" && drawer.open.get_untracked() {
            event.stop_propagation();
            drawer.close();
        }
    };

    (
        html::button()
            .r#type("button")
            .class(format!("drawer-tab {edge_class}"))
            .attr("aria-expanded", move || drawer.is_open().to_string())
            .attr("aria-controls", id)
            .style((along, move || match drawer.is_open() {
                true => format!("{}px", width.shown()),
                false => "0px".to_owned(),
            }))
            .on(ev::click, move |_| drawer.toggle())
            .on(ev::keydown, escaping)
            .child((html::span().child(tab), move || {
                match (edge, drawer.is_open()) {
                    (Edge::Right, false) | (Edge::Left, true) => mark(Icon::PullOut),
                    (Edge::Right, true) | (Edge::Left, false) => mark(Icon::PushIn),
                }
            })),
        html::aside()
            .id(id)
            .class(format!("drawer {edge_class} {id}"))
            .attr("aria-label", move || named.get())
            .attr("hidden", move || (!drawer.is_open()).then_some(""))
            .style(("width", move || format!("{}px", width.shown())))
            .on(ev::keydown, escaping)
            .child((
                drawer_handle(
                    width,
                    match edge {
                        Edge::Left => Grows::Rightwards,
                        Edge::Right => Grows::Leftwards,
                    },
                    format!("Resize {called}"),
                ),
                html::div().class("inside").child((
                    html::div().class("head").child((
                        head,
                        html::button()
                            .r#type("button")
                            .class("shut")
                            .attr("aria-label", format!("Close {called}"))
                            .on(ev::click, move |_| drawer.close())
                            .child("\u{00d7}"),
                    )),
                    body,
                )),
            )),
    )
}
