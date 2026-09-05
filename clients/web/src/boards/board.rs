use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev};
use leptos_router::hooks::use_navigate;
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

use crate::boards::model::{Board, Placement, PositionedPiece, Size, Spot};
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
const ROOM_ABOVE: i64 = 42;
const BAR_GAP: i64 = 8;
const CARD: Size = Size {
    width: 168,
    height: 84,
};
const SMALLEST: Size = Size {
    width: 80,
    height: 40,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Held {
    Whole,
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Clone, PartialEq, Eq)]
struct Renaming {
    piece: PieceId,
    at: Placement,
    was: String,
}

#[derive(Clone, PartialEq, Eq)]
struct Carrying {
    piece: PieceId,
    held: Held,
    from: Placement,
    by: Spot,
}

#[derive(Clone, Copy)]
struct Handles {
    carrying: RwSignal<Option<Carrying>>,
    selected: RwSignal<Option<PieceId>>,
    renaming: RwSignal<Option<Renaming>>,
    reshaping: Action<(PieceId, Option<Spot>, Option<Size>), ()>,
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
        let at = Placement {
            spot: next_spot(board.get_untracked().as_ref()),
            size: CARD,
        };
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

    let reshaping = Action::new_local(
        move |(piece, to, size): &(PieceId, Option<Spot>, Option<Size>)| {
            let piece = piece.clone();
            let to = *to;
            let size = *size;
            let was = reshaped(board, &piece, to, size);

            async move {
                let Some(open) = board.get_untracked() else {
                    return;
                };

                match service::reshape(&open.id, &piece, to, size).await {
                    Ok(moved) => {
                        problem.set(None);
                        arrived(moved);
                    }
                    Err(failure) => {
                        problem.set(Some(failure));

                        if let Some(back) = was {
                            reshaped(board, &piece, Some(back.spot), Some(back.size));
                        }
                    }
                }
            }
        },
    );

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
        reshaping,
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

fn drawn(held: &PositionedPiece, pool: &[Piece]) -> Option<(Piece, Placement)> {
    pool.iter()
        .find(|piece| piece.id == held.piece)
        .map(|piece| {
            (
                piece.clone(),
                Placement {
                    spot: held.spot,
                    size: held.size,
                },
            )
        })
}

fn card(href: String, piece: Piece, at: Placement, handles: Handles) -> impl IntoView {
    let opening = use_navigate();
    let reopening = opening.clone();
    let shown = piece.shown_as().to_owned();
    let named = shown.clone();
    let opened = href.clone();
    let reopened = href.clone();
    let id = piece.id;
    let mine = id.clone();
    let borne = id.clone();
    let nudged = id.clone();
    let placed = id.clone();
    let gripped = id.clone();

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
        .attr("style", move || boxed(drawn_at(handles, &placed, at)))
        .on(ev::pointerdown, move |event| {
            grab(&event, id.clone(), Held::Whole, at, handles);
        })
        .on(ev::pointermove, move |event| carry(&event, handles))
        .on(ev::pointerup, move |_| drop_it(handles))
        .on(ev::pointercancel, move |_| handles.carrying.set(None))
        .on(ev::dblclick, move |_| {
            opening(&opened, Default::default());
        })
        .on(ev::keydown, move |event| {
            if event.key() == "Enter" {
                event.prevent_default();
                reopening(&reopened, Default::default());

                return;
            }

            let Some(to) = nudge(&event.key(), event.shift_key(), at.spot) else {
                return;
            };
            event.prevent_default();

            handles.reshaping.dispatch((nudged.clone(), Some(to), None));
        })
        .child((
            name(href, named),
            EVERY_HANDLE
                .iter()
                .map(|held| grip(gripped.clone(), *held, at, shown.clone(), handles))
                .collect::<Vec<_>>(),
        ))
}

const EVERY_HANDLE: [Held; 8] = [
    Held::Top,
    Held::Bottom,
    Held::Left,
    Held::Right,
    Held::TopLeft,
    Held::TopRight,
    Held::BottomLeft,
    Held::BottomRight,
];

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

    carried.by = Spot {
        x: carried.by.x + event.movement_x() as i64,
        y: carried.by.y + event.movement_y() as i64,
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
    handles.carrying.set(None);
    let landed = carried.landing();

    if landed == carried.from {
        return;
    }

    handles.reshaping.dispatch((
        carried.piece,
        (landed.spot != carried.from.spot).then_some(landed.spot),
        (landed.size != carried.from.size).then_some(landed.size),
    ));
}

fn drawn_at(handles: Handles, piece: &PieceId, at: Placement) -> Placement {
    handles.carrying.with(|held| match held {
        Some(held) if &held.piece == piece => held.landing(),
        _ => at,
    })
}

fn boxed(at: Placement) -> String {
    format!(
        "left: {}px; top: {}px; width: {}px; height: {}px;",
        at.spot.x, at.spot.y, at.size.width, at.size.height
    )
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
        handles.retitling.dispatch((piece.clone(), written));
    }
}

fn actions(href: String, piece: Piece, at: Placement, handles: Handles) -> impl IntoView {
    let opening = use_navigate();
    let shown = piece.shown_as().to_owned();
    let called = shown.clone();
    let id = piece.id;
    let renamed = id.clone();
    let unpinned = id;
    let cramped = at.spot.y < ROOM_ABOVE;
    let top = match cramped {
        true => at.spot.y + at.size.height + BAR_GAP,
        false => at.spot.y - ROOM_ABOVE,
    };

    html::div()
        .class("pinned-actions")
        .class(("below", move || cramped))
        .attr("role", "toolbar")
        .attr("aria-label", format!("Actions for {shown}"))
        .attr("style", format!("left: {}px; top: {}px;", at.spot.x, top))
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

fn held(board: RwSignal<Option<Board>>, piece: &PieceId, at: Placement) {
    board.update(|open| {
        if let Some(open) = open {
            open.pieces.push(PositionedPiece {
                piece: piece.clone(),
                spot: at.spot,
                size: at.size,
            });
        }
    });
}

fn reshaped(
    board: RwSignal<Option<Board>>,
    piece: &PieceId,
    to: Option<Spot>,
    size: Option<Size>,
) -> Option<Placement> {
    let mut was = None;

    board.update(|open| {
        if let Some(open) = open
            && let Some(held) = open.pieces.iter_mut().find(|held| &held.piece == piece)
        {
            was = Some(Placement {
                spot: held.spot,
                size: held.size,
            });

            if let Some(to) = to {
                held.spot = to;
            }

            if let Some(size) = size {
                held.size = size;
            }
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

impl Held {
    fn side(&self) -> &'static str {
        match self {
            Self::Whole => "whole",
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::Left => "left",
            Self::Right => "right",
            Self::TopLeft => "top-left",
            Self::TopRight => "top-right",
            Self::BottomLeft => "bottom-left",
            Self::BottomRight => "bottom-right",
        }
    }

    fn pulls_left(&self) -> bool {
        matches!(self, Self::Left | Self::TopLeft | Self::BottomLeft)
    }

    fn pulls_right(&self) -> bool {
        matches!(self, Self::Right | Self::TopRight | Self::BottomRight)
    }

    fn pulls_top(&self) -> bool {
        matches!(self, Self::Top | Self::TopLeft | Self::TopRight)
    }

    fn pulls_bottom(&self) -> bool {
        matches!(self, Self::Bottom | Self::BottomLeft | Self::BottomRight)
    }
}

impl Carrying {
    fn dragging(&self) -> bool {
        self.by.x.abs().max(self.by.y.abs()) >= DRAG_BEGINS
    }

    fn landing(&self) -> Placement {
        if !self.dragging() {
            return self.from;
        }

        if self.held == Held::Whole {
            return Placement {
                spot: snapped(Spot {
                    x: self.from.spot.x + self.by.x,
                    y: self.from.spot.y + self.by.y,
                }),
                size: self.from.size,
            };
        }

        let far = Spot {
            x: self.from.spot.x + self.from.size.width,
            y: self.from.spot.y + self.from.size.height,
        };
        let left = match self.held.pulls_left() {
            true => onto_grid(self.from.spot.x + self.by.x).min(far.x - SMALLEST.width),
            false => self.from.spot.x,
        };
        let top = match self.held.pulls_top() {
            true => onto_grid(self.from.spot.y + self.by.y).min(far.y - SMALLEST.height),
            false => self.from.spot.y,
        };
        let right = match self.held.pulls_right() {
            true => onto_grid(far.x + self.by.x).max(left + SMALLEST.width),
            false => far.x,
        };
        let bottom = match self.held.pulls_bottom() {
            true => onto_grid(far.y + self.by.y).max(top + SMALLEST.height),
            false => far.y,
        };

        Placement {
            spot: Spot { x: left, y: top },
            size: Size {
                width: right - left,
                height: bottom - top,
            },
        }
    }
}
