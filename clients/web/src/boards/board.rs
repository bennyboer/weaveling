use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev};
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

use crate::boards::board_state::BoardState;
use crate::boards::card::card;
use crate::boards::carrying::Carrying;
use crate::boards::carrying::{CARD, snapped};
use crate::boards::chrome::{actions, naming, zooming};
use crate::boards::handles::{Handles, Naming};
use crate::boards::model::{Placement, Size, Spot};
use crate::boards::viewport::{NEARER, Viewport};
use crate::ideas::inspector::docked;
use crate::ideas::inspector_state::InspectorState;
use crate::ideas::model::{Idea, IdeaId};
use crate::route;
use crate::tray::laid_out;

const DOTS: i64 = 20;

#[component]
pub fn TheBoard(project: String) -> impl IntoView {
    let state = BoardState::open(&route::project_id(&project));
    let selected = RwSignal::new(None::<IdeaId>);
    let handles = Handles {
        viewport: RwSignal::new(Viewport::RESTING),
        carrying: RwSignal::new(None::<Carrying>),
        selected,
        naming: RwSignal::new(None::<Naming>),
        state,
    };
    let inspector = {
        let project = project.clone();

        InspectorState::inspecting(Memo::new(move |_| project.clone()), selected.into())
    };

    Effect::new(move || {
        if let Some(newer) = inspector.opened() {
            state.adopt(newer);
        }
    });
    Effect::new(move || {
        if let Some(newer) = selected.get().and_then(|id| state.idea(&id)) {
            inspector.adopt(newer);
        }
    });

    html::section().class("board").child((
        move || {
            state.problem().map(|failure| {
                html::p().class("problem").role("alert").child((
                    failure.to_string(),
                    html::button()
                        .r#type("button")
                        .class("dismiss")
                        .attr("aria-label", "Dismiss")
                        .on(ev::click, move |_| state.dismiss())
                        .child("\u{00d7}"),
                ))
            })
        },
        laid_out(
            corkboard(project, handles).into_any(),
            kept(handles, inspector).into_any(),
            move || format!("Not on the board \u{00b7} {}", state.unpinned().len()),
        ),
    ))
}

fn kept(handles: Handles, inspector: InspectorState) -> impl IntoView {
    let state = handles.state;

    (
        docked(inspector),
        html::p()
            .class("tally")
            .child(move || format!("Not on the board \u{00b7} {}", state.unpinned().len())),
        html::ul()
            .class("waiting")
            .attr("aria-label", "Ideas not on the board")
            .child(move || {
                state
                    .unpinned()
                    .into_iter()
                    .map(|idea| pinnable(idea, handles))
                    .collect::<Vec<_>>()
            }),
        move || {
            (state.ready() && state.unpinned().is_empty()).then(|| {
                html::p()
                    .class("empty")
                    .child("Every idea is on the board.")
            })
        },
        html::p()
            .class("how")
            .child("Click an idea to pin it where there is room."),
    )
}

fn corkboard(project: String, handles: Handles) -> impl IntoView {
    let Handles {
        viewport,
        carrying,
        selected,
        naming: held,
        state,
    } = handles;
    let board_ref = NodeRef::<html::Section>::new();
    let panning = RwSignal::new(false);

    let chosen = move || {
        if carrying.with(|held| held.as_ref().is_some_and(Carrying::dragging)) {
            return None;
        }
        let held = selected.get()?;

        state.pinned().into_iter().find(|(idea, _)| idea.id == held)
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
        .on(ev::dblclick, move |event| {
            if event.target() != event.current_target() {
                return;
            }

            let Some(pointed) = pointed_at(&event) else {
                return;
            };
            let at = viewport.get_untracked().on_board(pointed);

            held.set(Some(Naming::Capturing {
                at: around(at, CARD),
            }));
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

                            state
                                .pinned()
                                .into_iter()
                                .map(|(idea, at)| {
                                    card(
                                        route::idea(&project, &idea.id, &idea.title),
                                        idea,
                                        at,
                                        handles,
                                    )
                                })
                                .collect::<Vec<_>>()
                        }
                    },
                    move || held.get().map(|open| naming(open, handles)),
                )),
            move || {
                let (idea, at) = chosen()?;

                Some(actions(
                    route::idea(&project, &idea.id, &idea.title),
                    idea,
                    at,
                    handles,
                ))
            },
            zooming(viewport, board_ref),
        ))
}

fn pointed_at(event: &ev::MouseEvent) -> Option<Spot> {
    let within = event
        .current_target()
        .and_then(|it| it.dyn_into::<HtmlElement>().ok())?;
    let edge = within.get_bounding_client_rect();

    Some(Spot {
        x: event.client_x() as i64 - edge.left() as i64,
        y: event.client_y() as i64 - edge.top() as i64,
    })
}

fn around(middle: Spot, size: Size) -> Placement {
    Placement {
        spot: snapped(Spot {
            x: middle.x - size.width / 2,
            y: middle.y - size.height / 2,
        }),
        size,
    }
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

fn pinnable(idea: Idea, handles: Handles) -> impl IntoView {
    let shown = idea.shown_as().to_owned();
    let id = idea.id;

    html::li().child(
        html::button()
            .r#type("button")
            .attr("aria-label", format!("Pin {shown}"))
            .on(ev::click, move |_| {
                handles
                    .state
                    .pin(id.clone(), handles.viewport.get_untracked());
            })
            .child(shown),
    )
}
