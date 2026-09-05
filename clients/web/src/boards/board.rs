use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev};
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

use crate::boards::card::card;
use crate::boards::carrying::Carrying;
use crate::boards::chrome::{actions, rename, zooming};
use crate::boards::handles::{Handles, Renaming};
use crate::boards::model::Spot;
use crate::boards::open_board::OpenBoard;
use crate::boards::viewport::{NEARER, Viewport};
use crate::pieces::model::{Piece, PieceId};
use crate::route;

const DOTS: i64 = 20;

#[component]
pub fn TheBoard(project: String) -> impl IntoView {
    let open = OpenBoard::open(&route::project_id(&project));
    let handles = Handles {
        viewport: RwSignal::new(Viewport::RESTING),
        carrying: RwSignal::new(None::<Carrying>),
        selected: RwSignal::new(None::<PieceId>),
        renaming: RwSignal::new(None::<Renaming>),
        open,
    };

    html::section().class("board").child((
        html::h2().child("Board"),
        move || {
            open.problem().map(|failure| {
                html::p().class("problem").role("alert").child((
                    failure.to_string(),
                    html::button()
                        .r#type("button")
                        .class("dismiss")
                        .attr("aria-label", "Dismiss")
                        .on(ev::click, move |_| open.dismiss())
                        .child("\u{00d7}"),
                ))
            })
        },
        corkboard(project, handles),
        html::section().class("unpinned").child((
            html::h3().child("Not on the board"),
            move || {
                (open.ready() && open.unpinned().is_empty()).then(|| {
                    html::p()
                        .class("empty")
                        .child("Every piece is on the board.")
                })
            },
            html::ul()
                .class("waiting")
                .attr("aria-label", "Pieces not on the board")
                .child(move || {
                    open.unpinned()
                        .into_iter()
                        .map(|piece| pinnable(piece, handles))
                        .collect::<Vec<_>>()
                }),
        )),
    ))
}

fn corkboard(project: String, handles: Handles) -> impl IntoView {
    let Handles {
        viewport,
        carrying,
        selected,
        renaming,
        open,
    } = handles;
    let board_ref = NodeRef::<html::Section>::new();
    let panning = RwSignal::new(false);

    let chosen = move || {
        if carrying.with(|held| held.as_ref().is_some_and(Carrying::dragging)) {
            return None;
        }
        let held = selected.get()?;

        open.pinned()
            .into_iter()
            .find(|(piece, _)| piece.id == held)
    };

    html::section()
        .class("corkboard")
        .node_ref(board_ref)
        .attr("aria-label", "Board")
        .attr("style", move || viewport.get().grid(DOTS))
        .on(ev::pointerdown, move |event| {
            selected.set(None);
            panning.set(true);
            grabbed(&event);
        })
        .on(ev::pointermove, move |event| {
            if panning.get_untracked() {
                viewport.update(|it| *it = it.panned(dragged(&event)));
            }
        })
        .on(ev::pointerup, move |_| panning.set(false))
        .on(ev::pointercancel, move |_| panning.set(false))
        .on(ev::wheel, move |event| {
            let Some((towards, by)) = zoom_asked(&event) else {
                return;
            };

            viewport.update(|it| *it = it.zoomed(towards, by));
        })
        .on(ev::keydown, move |event| {
            if event.key() == "Escape" {
                selected.set(None);
            }
        })
        .child((
            html::div()
                .class("surface")
                .attr("style", move || viewport.get().surface())
                .child((
                    {
                        let project = project.clone();

                        move || {
                            let project = project.clone();

                            open.pinned()
                                .into_iter()
                                .map(|(piece, at)| {
                                    card(
                                        route::piece(&project, &piece.id, &piece.title),
                                        piece,
                                        at,
                                        handles,
                                    )
                                })
                                .collect::<Vec<_>>()
                        }
                    },
                    move || renaming.get().map(|held| rename(held, handles)),
                )),
            move || {
                let (piece, at) = chosen()?;

                Some(actions(
                    route::piece(&project, &piece.id, &piece.title),
                    piece,
                    at,
                    handles,
                ))
            },
            zooming(viewport, board_ref),
        ))
}

fn grabbed(event: &ev::PointerEvent) {
    if let Some(board) = event
        .current_target()
        .and_then(|it| it.dyn_into::<HtmlElement>().ok())
    {
        let _ = board.set_pointer_capture(event.pointer_id());
    }
}

fn dragged(event: &ev::PointerEvent) -> Spot {
    Spot {
        x: event.movement_x() as i64,
        y: event.movement_y() as i64,
    }
}

fn zoom_asked(event: &ev::WheelEvent) -> Option<(Spot, f64)> {
    if !event.ctrl_key() && !event.meta_key() {
        return None;
    }
    event.prevent_default();

    let within = event
        .current_target()
        .and_then(|it| it.dyn_into::<HtmlElement>().ok())?;
    let edge = within.get_bounding_client_rect();

    Some((
        Spot {
            x: event.client_x() as i64 - edge.left() as i64,
            y: event.client_y() as i64 - edge.top() as i64,
        },
        match event.delta_y() < 0.0 {
            true => NEARER,
            false => 1.0 / NEARER,
        },
    ))
}

fn pinnable(piece: Piece, handles: Handles) -> impl IntoView {
    let shown = piece.shown_as().to_owned();
    let id = piece.id;

    html::li().child(
        html::button()
            .r#type("button")
            .attr("aria-label", format!("Pin {shown}"))
            .on(ev::click, move |_| {
                handles
                    .open
                    .pin(id.clone(), handles.viewport.get_untracked());
            })
            .child(shown),
    )
}
