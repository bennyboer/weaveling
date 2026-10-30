use std::collections::HashMap;

use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev};
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

use crate::boards::board_view_state::BoardViewState;
use crate::boards::card;
use crate::boards::carrying;
use crate::boards::carrying::CARD;
use crate::boards::carrying::Carrying;
use crate::boards::chrome;
use crate::boards::handles::{Handles, Naming};
use crate::boards::model::{Placement, Size, Spot};
use crate::boards::viewport::{NEARER, Viewport};
use crate::ideas::inspector;
use crate::ideas::inspector_state::InspectorState;
use crate::ideas::model::{Idea, IdeaId};
use crate::members::roles::Roles;
use crate::problem;
use crate::route;
use crate::speech;
use crate::tray;

const DOTS: i64 = 20;

#[component]
pub fn BoardView(project: String) -> impl IntoView {
    let state = BoardViewState::open(&route::project_id(&project));
    let selected = RwSignal::new(None::<IdeaId>);
    let roles = expect_context::<Roles>();
    let may_write = {
        let project = project.clone();

        Memo::new(move |_| roles.may_write(&project))
    };
    let handles = Handles {
        viewport: RwSignal::new(Viewport::RESTING),
        carrying: RwSignal::new(None::<Carrying>),
        selected,
        naming: RwSignal::new(None::<Naming>),
        state,
        may_write,
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

    html::section()
        .class("board")
        .class(("reading", move || !may_write.get()))
        .child((
            move || {
                state
                    .problem()
                    .map(|failure| problem::problem_to_dismiss(failure, move || state.dismiss()))
            },
            tray::laid_out(
                corkboard(project, handles).into_any(),
                kept(handles, inspector).into_any(),
                move || {
                    speech::say_about(
                        "board-not-on-it",
                        "count",
                        &state.unpinned().len().to_string(),
                    )
                },
            ),
        ))
}

fn kept(handles: Handles, inspector: InspectorState) -> impl IntoView {
    let state = handles.state;

    (
        inspector::docked(inspector),
        html::p().class("tally").child(move || {
            speech::say_about(
                "board-not-on-it",
                "count",
                &state.unpinned().len().to_string(),
            )
        }),
        html::ul()
            .class("waiting")
            .attr("aria-label", speech::said("board-waiting"))
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
                    .child(speech::said("board-every-idea-pinned"))
            })
        },
        html::p()
            .class("how")
            .child(speech::said("board-how-to-pin")),
    )
}

fn corkboard(project: String, handles: Handles) -> impl IntoView {
    let Handles {
        viewport,
        carrying,
        selected,
        naming: held,
        state,
        may_write,
    } = handles;
    let board_ref = NodeRef::<html::Section>::new();
    let panning = RwSignal::new(false);
    let touching = StoredValue::new(HashMap::<i32, Spot>::new());

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
        .attr("aria-label", speech::said("board"))
        .attr("style", move || viewport.get().grid(DOTS))
        .on(ev::capture(ev::pointerdown), move |event| {
            let Some(at) = pointed_at(&event) else {
                return;
            };
            touching.update_value(|fingers| _ = fingers.insert(event.pointer_id(), at));
            if touching.with_value(|fingers| fingers.len() >= 2) {
                event.stop_propagation();
                carrying.set(None);
            }
        })
        .on(ev::capture(ev::pointermove), move |event| {
            let Some(now) = pointed_at(&event) else {
                return;
            };
            let finger = event.pointer_id();
            let (before, other, down) = touching.with_value(|fingers| {
                let other = fingers
                    .iter()
                    .find(|(held, _)| **held != finger)
                    .map(|(_, at)| *at);

                (fingers.get(&finger).copied(), other, fingers.len())
            });
            let Some(before) = before else {
                return;
            };
            touching.update_value(|fingers| _ = fingers.insert(finger, now));

            match other {
                Some(other) if down == 2 => {
                    viewport.update(|it| *it = it.pinched((before, other), (now, other)))
                }
                None if panning.get_untracked() => viewport.update(|it| {
                    *it = it.panned(Spot {
                        x: now.x - before.x,
                        y: now.y - before.y,
                    })
                }),
                _ => {}
            }
        })
        .on(ev::capture(ev::pointerup), move |event| {
            let_go(&event, touching)
        })
        .on(ev::capture(ev::pointercancel), move |event| {
            let_go(&event, touching)
        })
        .on(ev::pointerdown, move |event| {
            selected.set(None);
            panning.set(true);
            grabbed(&event);
        })
        .on(ev::pointerup, move |_| still_panning(touching, panning))
        .on(ev::pointercancel, move |_| still_panning(touching, panning))
        .on(ev::wheel, move |event| {
            let Some((towards, by)) = zoom_asked(&event) else {
                return;
            };

            viewport.update(|it| *it = it.zoomed(towards, by));
        })
        .on(ev::dblclick, move |event| {
            if event.target() != event.current_target() || !may_write.get_untracked() {
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
                                    card::card(
                                        route::idea(&project, &idea.id, &idea.title),
                                        idea,
                                        at,
                                        handles,
                                    )
                                })
                                .collect::<Vec<_>>()
                        }
                    },
                    move || held.get().map(|open| chrome::naming(open, handles)),
                )),
            move || {
                let (idea, at) = chosen()?;

                Some(chrome::actions(
                    route::idea(&project, &idea.id, &idea.title),
                    idea,
                    at,
                    handles,
                ))
            },
            chrome::zooming(viewport, board_ref),
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
        spot: carrying::snapped(Spot {
            x: middle.x - size.width / 2,
            y: middle.y - size.height / 2,
        }),
        size,
    }
}

fn still_panning(touching: StoredValue<HashMap<i32, Spot>>, panning: RwSignal<bool>) {
    if touching.with_value(HashMap::is_empty) {
        panning.set(false);
    }
}

fn let_go(event: &ev::PointerEvent, touching: StoredValue<HashMap<i32, Spot>>) {
    touching.update_value(|fingers| _ = fingers.remove(&event.pointer_id()));
}

fn grabbed(event: &ev::PointerEvent) {
    if let Some(board) = event
        .current_target()
        .and_then(|it| it.dyn_into::<HtmlElement>().ok())
    {
        let _ = board.set_pointer_capture(event.pointer_id());
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
            .attr("aria-label", {
                let shown = shown.clone();

                move || speech::say_about("board-pin", "idea", &shown)
            })
            .attr("disabled", move || {
                (!handles.may_write.get()).then_some("disabled")
            })
            .on(ev::click, move |_| {
                handles
                    .state
                    .pin(id.clone(), handles.viewport.get_untracked());
            })
            .child(shown),
    )
}
