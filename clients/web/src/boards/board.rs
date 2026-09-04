use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev};
use leptos_router::hooks::use_navigate;
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

use crate::boards::model::{Board, PositionedPiece, Spot};
use crate::boards::service;
use crate::http::ApiError;
use crate::pieces::model::{Piece, PieceId};
use crate::pieces::service as pieces;
use crate::route;

const GRID: i64 = 5;
const LEAP: i64 = 40;
const DRAG_BEGINS: i64 = 4;
const STEP: i64 = 40;
const COLUMNS: i64 = 3;
const ROOM_ABOVE: i64 = 44;

#[derive(Clone, PartialEq, Eq)]
struct Renaming {
    piece: PieceId,
    at: Spot,
    was: String,
}

#[derive(Clone, PartialEq, Eq)]
struct Carrying {
    piece: PieceId,
    from: Spot,
    by: Spot,
}

#[derive(Clone, Copy)]
struct Handles {
    carrying: RwSignal<Option<Carrying>>,
    selected: RwSignal<Option<PieceId>>,
    renaming: RwSignal<Option<Renaming>>,
    moving: Action<(PieceId, Spot), ()>,
    unpinning: Action<PieceId, ()>,
    retitling: Action<(PieceId, String), ()>,
}

#[component]
pub fn TheBoard(project: String) -> impl IntoView {
    let problem = RwSignal::new(None::<ApiError>);
    let board = RwSignal::new(None::<Board>);
    let carrying = RwSignal::new(None::<Carrying>);
    let selected = RwSignal::new(None::<PieceId>);
    let renaming = RwSignal::new(None::<Renaming>);
    let pool = RwSignal::new(None::<Vec<Piece>>);
    let id = route::project_id(&project);

    let listing = {
        let id = id.clone();

        Action::new_local(move |()| {
            let id = id.clone();

            async move {
                match pieces::list(&id).await {
                    Ok(found) => pool.set(Some(found)),
                    Err(failure) => problem.set(Some(failure)),
                }
            }
        })
    };
    listing.dispatch(());

    let arrived = move |open: Board| {
        let known = board.with_untracked(|held| held.as_ref().map(|held| held.version));

        if known.is_none_or(|known| open.version >= known) {
            board.set(Some(open));
        }
    };

    let settled = move |answer: Result<Board, ApiError>| match answer {
        Ok(open) => {
            problem.set(None);
            arrived(open);
        }
        Err(failure) => problem.set(Some(failure)),
    };

    let opening = Action::new_local(move |()| {
        let id = id.clone();

        async move { settled(service::open(&id).await) }
    });
    opening.dispatch(());

    let pinning = Action::new_local(move |piece: &PieceId| {
        let piece = piece.clone();
        let at = next_spot(board.get_untracked().as_ref());
        held(board, &piece, at);

        async move {
            let Some(open) = board.get_untracked() else {
                return;
            };

            match service::pin(&open.id, &piece, at).await {
                Ok(pinned) => {
                    problem.set(None);
                    arrived(pinned);
                }
                Err(failure) => {
                    problem.set(Some(failure));
                    board.update(|open| {
                        if let Some(open) = open {
                            open.pieces.retain(|holds| holds.piece != piece);
                        }
                    });
                }
            }
        }
    });

    let moving = Action::new_local(move |(piece, to): &(PieceId, Spot)| {
        let piece = piece.clone();
        let to = *to;
        let was = shifted(board, &piece, to);

        async move {
            let Some(open) = board.get_untracked() else {
                return;
            };

            match service::move_piece(&open.id, &piece, to).await {
                Ok(moved) => {
                    problem.set(None);
                    arrived(moved);
                }
                Err(failure) => {
                    problem.set(Some(failure));

                    if let Some(back) = was {
                        shifted(board, &piece, back);
                    }
                }
            }
        }
    });

    let unpinning = Action::new_local(move |piece: &PieceId| {
        let piece = piece.clone();

        async move {
            let Some(open) = board.get_untracked() else {
                return;
            };

            match service::unpin(&open.id, &piece).await {
                Ok(()) => {
                    problem.set(None);
                    selected.update(|held| {
                        if held.as_ref() == Some(&piece) {
                            *held = None;
                        }
                    });
                    board.update(|open| {
                        if let Some(open) = open {
                            open.pieces.retain(|held| held.piece != piece);
                        }
                    });
                }
                Err(failure) => problem.set(Some(failure)),
            }
        }
    });

    let in_pool = move || pool.get().unwrap_or_default();

    let unpinned = move || {
        let Some(held) = board.get() else {
            return Vec::new();
        };

        in_pool()
            .into_iter()
            .filter(|piece| !held.holds(&piece.id))
            .collect::<Vec<_>>()
    };

    let pinned = move || {
        let pool = in_pool();

        board
            .get()
            .map(|open| {
                open.pieces
                    .into_iter()
                    .filter_map(|held| drawn(&held, &pool))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };

    let retitling = Action::new_local(move |(piece, title): &(PieceId, String)| {
        let piece = piece.clone();
        let title = title.clone();

        async move {
            match pieces::retitle(&piece, &title).await {
                Ok(renamed) => {
                    problem.set(None);
                    pool.update(|held| {
                        if let Some(held) = held
                            && let Some(known) =
                                held.iter_mut().find(|known| known.id == renamed.id)
                        {
                            *known = renamed;
                        }
                    });
                }
                Err(failure) => problem.set(Some(failure)),
            }
        }
    });

    let handles = Handles {
        carrying,
        selected,
        renaming,
        moving,
        unpinning,
        retitling,
    };

    let chosen = move || {
        if carrying.get().is_some() {
            return None;
        }
        let held = selected.get()?;

        pinned().into_iter().find(|(piece, _)| piece.id == held)
    };

    html::section().class("board").child((
        html::h2().child("Board"),
        move || {
            problem.get().map(|failure| {
                html::p()
                    .class("problem")
                    .role("alert")
                    .child(failure.to_string())
            })
        },
        html::section()
            .class("corkboard")
            .attr("aria-label", "Board")
            .on(ev::pointerdown, move |_| selected.set(None))
            .on(ev::keydown, move |event| {
                if event.key() == "Escape" {
                    selected.set(None);
                }
            })
            .child((
                {
                    let project = project.clone();

                    move || {
                        let project = project.clone();

                        pinned()
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
                move || {
                    let (piece, at) = chosen()?;

                    Some(actions(
                        route::piece(&project, &piece.id, &piece.title),
                        piece,
                        at,
                        handles,
                    ))
                },
                move || renaming.get().map(|held| rename(held, handles)),
            )),
        html::section().class("unpinned").child((
            html::h3().child("Not on the board"),
            move || {
                (board.get().is_some() && unpinned().is_empty()).then(|| {
                    html::p()
                        .class("empty")
                        .child("Every piece is on the board.")
                })
            },
            html::ul()
                .class("waiting")
                .attr("aria-label", "Pieces not on the board")
                .child(move || {
                    unpinned()
                        .into_iter()
                        .map(|piece| pinnable(piece, pinning))
                        .collect::<Vec<_>>()
                }),
        )),
    ))
}

fn drawn(held: &PositionedPiece, pool: &[Piece]) -> Option<(Piece, Spot)> {
    pool.iter()
        .find(|piece| piece.id == held.piece)
        .map(|piece| (piece.clone(), held.spot))
}

fn card(href: String, piece: Piece, at: Spot, handles: Handles) -> impl IntoView {
    let opening = use_navigate();
    let reopening = opening.clone();
    let shown = piece.shown_as().to_owned();
    let named = shown.clone();
    let opened = href.clone();
    let reopened = href.clone();
    let id = piece.id;
    let mine = id.clone();
    let borne = id.clone();
    let pressed = id.clone();
    let nudged = id.clone();
    let placed = id;

    html::article()
        .class("pinned")
        .class(("selected", move || {
            handles.selected.with(|held| held.as_ref() == Some(&mine))
        }))
        .class(("carried", move || {
            handles
                .carrying
                .with(|held| held.as_ref().is_some_and(|held| held.piece == borne))
        }))
        .attr("tabindex", "0")
        .attr("aria-label", shown.clone())
        .attr("title", shown.clone())
        .attr("style", move || {
            let at = handles.carrying.with(|held| match held {
                Some(held) if held.piece == placed => held.landing(),
                _ => at,
            });

            format!("left: {}px; top: {}px;", at.x, at.y)
        })
        .on(ev::pointerdown, move |event| {
            event.stop_propagation();

            if let Some(card) = event
                .current_target()
                .and_then(|it| it.dyn_into::<HtmlElement>().ok())
            {
                let _ = card.focus();
            }

            handles.selected.set(Some(pressed.clone()));
            handles.carrying.set(Some(Carrying {
                piece: pressed.clone(),
                from: at,
                by: Spot { x: 0, y: 0 },
            }));
        })
        .on(ev::pointermove, move |event| {
            let Some(mut carried) = handles.carrying.get_untracked() else {
                return;
            };
            let already = carried.dragging();

            carried.by = Spot {
                x: carried.by.x + event.movement_x() as i64,
                y: carried.by.y + event.movement_y() as i64,
            };
            let now = carried.dragging();
            handles.carrying.set(Some(carried));

            if already || !now {
                return;
            }

            if let Some(card) = event
                .current_target()
                .and_then(|it| it.dyn_into::<HtmlElement>().ok())
            {
                let _ = card.set_pointer_capture(event.pointer_id());
            }
        })
        .on(ev::pointerup, move |_| {
            let Some(carried) = handles.carrying.get_untracked() else {
                return;
            };
            handles.carrying.set(None);
            let landed = carried.landing();

            if landed != carried.from {
                handles.moving.dispatch((carried.piece, landed));
            }
        })
        .on(ev::pointercancel, move |_| {
            handles.carrying.set(None);
        })
        .on(ev::dblclick, move |_| {
            opening(&opened, Default::default());
        })
        .on(ev::keydown, move |event| {
            if event.key() == "Enter" {
                event.prevent_default();
                reopening(&reopened, Default::default());

                return;
            }

            let Some(to) = nudge(&event.key(), event.shift_key(), at) else {
                return;
            };
            event.prevent_default();

            handles.moving.dispatch((nudged.clone(), to));
        })
        .child(name(href, named))
}

fn rename(renaming: Renaming, handles: Handles) -> impl IntoView {
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
        .attr("style", format!("left: {}px; top: {}px;", at.x, at.y))
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
        handles.retitling.dispatch((piece.clone(), written));
    }
}

fn actions(href: String, piece: Piece, at: Spot, handles: Handles) -> impl IntoView {
    let opening = use_navigate();
    let shown = piece.shown_as().to_owned();
    let called = shown.clone();
    let id = piece.id;
    let renamed = id.clone();
    let unpinned = id;
    let cramped = at.y < ROOM_ABOVE;

    html::div()
        .class("pinned-actions")
        .class(("below", move || cramped))
        .attr("role", "toolbar")
        .attr("aria-label", format!("Actions for {shown}"))
        .attr("style", format!("left: {}px; top: {}px;", at.x, at.y))
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
                handles.unpinning.dispatch(unpinned.clone());
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

fn pinnable(piece: Piece, pinning: Action<PieceId, ()>) -> impl IntoView {
    let shown = piece.shown_as().to_owned();
    let id = piece.id;

    html::li().child(
        html::button()
            .r#type("button")
            .attr("aria-label", format!("Pin {shown}"))
            .on(ev::click, move |_| {
                pinning.dispatch(id.clone());
            })
            .child(shown),
    )
}

fn nudge(key: &str, leaping: bool, from: Spot) -> Option<Spot> {
    let by = if leaping { LEAP } else { GRID };
    let (x, y) = match key {
        "ArrowLeft" => (-by, 0),
        "ArrowRight" => (by, 0),
        "ArrowUp" => (0, -by),
        "ArrowDown" => (0, by),
        _ => return None,
    };

    Some(snapped(Spot {
        x: from.x + x,
        y: from.y + y,
    }))
}

fn held(board: RwSignal<Option<Board>>, piece: &PieceId, at: Spot) {
    board.update(|open| {
        if let Some(open) = open {
            open.pieces.push(PositionedPiece {
                piece: piece.clone(),
                spot: at,
            });
        }
    });
}

fn shifted(board: RwSignal<Option<Board>>, piece: &PieceId, to: Spot) -> Option<Spot> {
    let mut was = None;

    board.update(|open| {
        if let Some(open) = open
            && let Some(held) = open.pieces.iter_mut().find(|held| &held.piece == piece)
        {
            was = Some(held.spot);
            held.spot = to;
        }
    });

    was
}

fn snapped(loose: Spot) -> Spot {
    Spot {
        x: onto_grid(loose.x),
        y: onto_grid(loose.y),
    }
}

fn onto_grid(loose: i64) -> i64 {
    ((loose + GRID / 2).div_euclid(GRID) * GRID).max(0)
}

fn next_spot(board: Option<&Board>) -> Spot {
    let taken = board
        .map(|open| open.pieces.iter().map(|held| held.spot).collect::<Vec<_>>())
        .unwrap_or_default();
    let mut nth = 0;

    while taken.contains(&slot(nth)) {
        nth += 1;
    }

    slot(nth)
}

fn slot(nth: i64) -> Spot {
    snapped(Spot {
        x: STEP + (nth % COLUMNS) * (STEP * 5),
        y: STEP + (nth / COLUMNS) * (STEP * 3),
    })
}

impl Carrying {
    fn dragging(&self) -> bool {
        self.by.x.abs().max(self.by.y.abs()) >= DRAG_BEGINS
    }

    fn landing(&self) -> Spot {
        if !self.dragging() {
            return self.from;
        }

        snapped(Spot {
            x: self.from.x + self.by.x,
            y: self.from.y + self.by.y,
        })
    }
}
