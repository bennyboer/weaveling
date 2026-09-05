use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev};
use leptos_router::hooks::use_navigate;

use crate::boards::card::boxed;
use crate::boards::handles::{Handles, Renaming};
use crate::boards::model::{Placement, Spot};
use crate::boards::viewport::{NEARER, Viewport};
use crate::pieces::model::{Piece, PieceId};

const ROOM_ABOVE: i64 = 42;
const BAR_GAP: i64 = 8;

pub fn actions(href: String, piece: Piece, at: Placement, handles: Handles) -> impl IntoView {
    let opening = use_navigate();
    let shown = piece.shown_as().to_owned();
    let called = shown.clone();
    let id = piece.id;
    let renamed = id.clone();
    let unpinned = id;
    let alongside = move || {
        let seen = handles.viewport.get();
        let corner = seen.on_screen(at.spot);

        (corner, corner.y < ROOM_ABOVE, seen.tall(at.size))
    };

    html::div()
        .class("pinned-actions")
        .class(("below", move || alongside().1))
        .attr("role", "toolbar")
        .attr("aria-label", format!("Actions for {shown}"))
        .attr("style", move || {
            let (corner, cramped, tall) = alongside();
            let top = match cramped {
                true => corner.y + tall + BAR_GAP,
                false => corner.y - ROOM_ABOVE,
            };

            format!("left: {}px; top: {}px;", corner.x, top)
        })
        .on(ev::pointerdown, |event| event.stop_propagation())
        .child((
            deed(format!("Rename {shown}"), "\u{270e}", move || {
                handles.renaming.set(Some(Renaming {
                    piece: renamed.clone(),
                    at,
                    was: called.clone(),
                }));
            }),
            deed(format!("Open {shown}"), "\u{2197}", move || {
                opening(&href, Default::default());
            }),
            deed(format!("Unpin {shown}"), "\u{00d7}", move || {
                handles.open.unpin(unpinned.clone());
            }),
        ))
}

fn deed(what: String, glyph: &'static str, done: impl Fn() + 'static) -> impl IntoView {
    html::button()
        .r#type("button")
        .attr("aria-label", what)
        .on(ev::click, move |event| {
            event.stop_propagation();
            done();
        })
        .child(glyph)
}

pub fn rename(renaming: Renaming, handles: Handles) -> impl IntoView {
    let field = NodeRef::<html::Textarea>::new();
    let Renaming { piece, at, was } = renaming;
    let asked = was.clone();
    let shown = was.clone();
    let leaving = piece.clone();

    Effect::new(move |_| {
        if let Some(field) = field.get() {
            let _ = field.focus();
            field.select();
        }
    });

    html::textarea()
        .class("pinned-rename")
        .attr("aria-label", format!("Rename {asked}"))
        .attr("style", boxed(at))
        .node_ref(field)
        .prop("value", was.clone())
        .on(ev::pointerdown, |event| event.stop_propagation())
        .on(ev::dblclick, |event| event.stop_propagation())
        .on(ev::keydown, move |event| {
            event.stop_propagation();

            match event.key().as_str() {
                "Enter" if !event.shift_key() => {
                    event.prevent_default();
                    settle(field, &piece, &was, handles);
                }
                "Escape" => {
                    event.prevent_default();
                    handles.renaming.set(None);
                }
                _ => {}
            }
        })
        .on(ev::focusout, move |_| {
            settle(field, &leaving, &shown, handles)
        })
}

fn settle(field: NodeRef<html::Textarea>, piece: &PieceId, was: &str, handles: Handles) {
    if handles
        .renaming
        .with_untracked(|held| held.as_ref().map(|held| &held.piece) != Some(piece))
    {
        return;
    }
    handles.renaming.set(None);

    let Some(written) = field.get_untracked().map(|field| field.value()) else {
        return;
    };

    if written != was {
        handles.open.retitle(piece.clone(), written);
    }
}

pub fn zooming(viewport: RwSignal<Viewport>, board: NodeRef<html::Section>) -> impl IntoView {
    html::div()
        .class("zooming")
        .attr("role", "toolbar")
        .attr("aria-label", "Zoom")
        .on(ev::pointerdown, |event| event.stop_propagation())
        .on(ev::wheel, |event| event.stop_propagation())
        .child((
            deed("Zoom out".to_owned(), "\u{2212}", move || {
                let middle = middle_of(board);
                viewport.update(|it| *it = it.zoomed(middle, 1.0 / NEARER));
            }),
            html::button()
                .r#type("button")
                .class("reading")
                .attr("aria-label", "Reset the zoom")
                .on(ev::click, move |event| {
                    event.stop_propagation();
                    let middle = middle_of(board);
                    viewport.update(|it| *it = it.unzoomed(middle));
                })
                .child(move || viewport.get().as_percent()),
            deed("Zoom in".to_owned(), "+", move || {
                let middle = middle_of(board);
                viewport.update(|it| *it = it.zoomed(middle, NEARER));
            }),
        ))
}

fn middle_of(board: NodeRef<html::Section>) -> Spot {
    board
        .get_untracked()
        .map(|it| {
            let edge = it.get_bounding_client_rect();

            Spot {
                x: (edge.width() / 2.0) as i64,
                y: (edge.height() / 2.0) as i64,
            }
        })
        .unwrap_or(Spot { x: 0, y: 0 })
}
