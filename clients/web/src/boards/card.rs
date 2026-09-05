use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev};
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

use crate::boards::carrying::{Carrying, EVERY_HANDLE, Held, nudge};
use crate::boards::handles::{Handles, Naming};
use crate::boards::model::{Placement, Spot};
use crate::pieces::model::{Piece, PieceId};

pub fn card(href: String, piece: Piece, at: Placement, handles: Handles) -> impl IntoView {
    let shown = piece.shown_as().to_owned();
    let named = shown.clone();
    let id = piece.id;
    let mine = id.clone();
    let chosen = id.clone();
    let borne = id.clone();
    let nudged = id.clone();
    let placed = id.clone();
    let gripped = id.clone();
    let renamed = id.clone();
    let retyped = id.clone();
    let called = shown.clone();
    let recalled = shown.clone();

    html::article()
        .class("pinned")
        .class(("selected", move || {
            handles.selected.with(|held| held.as_ref() == Some(&chosen))
        }))
        .class(("carried", move || {
            handles
                .carrying
                .with(|held| held.as_ref().is_some_and(|held| held.piece == borne))
        }))
        .attr("tabindex", "0")
        .attr("aria-label", shown.clone())
        .attr("title", shown.clone())
        .attr("style", move || boxed(drawn_at(handles, &placed, at)))
        .on(ev::pointerdown, move |event| {
            grab(&event, id.clone(), Held::Whole, at, handles);
        })
        .on(ev::pointermove, move |event| carry(&event, handles))
        .on(ev::pointerup, move |_| drop_it(handles))
        .on(ev::pointercancel, move |_| handles.carrying.set(None))
        .on(ev::focusin, move |_| {
            handles.selected.set(Some(mine.clone()))
        })
        .on(ev::dblclick, move |event| {
            event.stop_propagation();
            handles.naming.set(Some(Naming::Renaming {
                piece: renamed.clone(),
                at,
                was: called.clone(),
            }));
        })
        .on(ev::keydown, move |event| {
            if event.key() == "Enter" {
                event.prevent_default();
                handles.naming.set(Some(Naming::Renaming {
                    piece: retyped.clone(),
                    at,
                    was: recalled.clone(),
                }));

                return;
            }

            let Some(to) = nudge(&event.key(), event.shift_key(), at.spot) else {
                return;
            };
            event.prevent_default();

            handles.open.reshape(nudged.clone(), Some(to), None);
        })
        .child((
            name(href, named),
            EVERY_HANDLE
                .iter()
                .map(|held| grip(gripped.clone(), *held, at, shown.clone(), handles))
                .collect::<Vec<_>>(),
        ))
}

fn grip(
    piece: PieceId,
    held: Held,
    at: Placement,
    shown: String,
    handles: Handles,
) -> impl IntoView {
    html::div()
        .class(format!("grip {}", held.side()))
        .attr("role", "separator")
        .attr(
            "aria-label",
            format!("Resize {shown} from the {}", held.side()),
        )
        .on(ev::pointerdown, move |event| {
            grab(&event, piece.clone(), held, at, handles);
        })
        .on(ev::pointermove, move |event| {
            event.stop_propagation();
            carry(&event, handles);
        })
        .on(ev::pointerup, move |event| {
            event.stop_propagation();
            drop_it(handles);
        })
        .on(ev::pointercancel, move |event| {
            event.stop_propagation();
            handles.carrying.set(None);
        })
}

fn grab(event: &ev::PointerEvent, piece: PieceId, held: Held, at: Placement, handles: Handles) {
    event.stop_propagation();

    if let Some(under) = event
        .current_target()
        .and_then(|it| it.dyn_into::<HtmlElement>().ok())
    {
        let _ = under.focus();

        if held != Held::Whole {
            let _ = under.set_pointer_capture(event.pointer_id());
        }
    }

    handles.selected.set(Some(piece.clone()));
    handles.carrying.set(Some(Carrying {
        piece,
        held,
        from: at,
        by: Spot { x: 0, y: 0 },
    }));
}

fn carry(event: &ev::PointerEvent, handles: Handles) {
    let Some(mut carried) = handles.carrying.get_untracked() else {
        return;
    };
    let already = carried.dragging();

    let across = handles.viewport.with_untracked(|it| {
        it.across(Spot {
            x: event.movement_x() as i64,
            y: event.movement_y() as i64,
        })
    });
    carried.by = Spot {
        x: carried.by.x + across.x,
        y: carried.by.y + across.y,
    };
    let now = carried.dragging();
    handles.carrying.set(Some(carried));

    if already || !now {
        return;
    }

    if let Some(under) = event
        .current_target()
        .and_then(|it| it.dyn_into::<HtmlElement>().ok())
    {
        let _ = under.set_pointer_capture(event.pointer_id());
    }
}

fn drop_it(handles: Handles) {
    let Some(carried) = handles.carrying.get_untracked() else {
        return;
    };
    let landed = carried.landing();

    if landed != carried.from {
        handles.open.reshape(
            carried.piece,
            (landed.spot != carried.from.spot).then_some(landed.spot),
            (landed.size != carried.from.size).then_some(landed.size),
        );
    }

    handles.carrying.set(None);
}

fn drawn_at(handles: Handles, piece: &PieceId, at: Placement) -> Placement {
    handles.carrying.with(|held| match held {
        Some(held) if &held.piece == piece => held.landing(),
        _ => at,
    })
}

pub fn boxed(at: Placement) -> String {
    format!(
        "left: {}px; top: {}px; width: {}px; height: {}px;",
        at.spot.x, at.spot.y, at.size.width, at.size.height
    )
}

fn name(href: String, shown: String) -> impl IntoView {
    html::a()
        .href(href)
        .class("name")
        .attr("tabindex", "-1")
        .attr("draggable", "false")
        .on(ev::click, |event| {
            if !opening_elsewhere(&event) {
                event.prevent_default();
            }
        })
        .child(shown)
}

fn opening_elsewhere(event: &ev::MouseEvent) -> bool {
    event.ctrl_key() || event.meta_key() || event.shift_key()
}
