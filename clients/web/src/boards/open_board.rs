use leptos::prelude::*;

use crate::boards::carrying::snapped;
use crate::boards::model::{Board, Placement, PositionedPiece, Size, Spot};
use crate::boards::service;
use crate::boards::viewport::Viewport;
use crate::http::ApiError;
use crate::pieces::model::{Piece, PieceId};
use crate::pieces::service as pieces;
use crate::projects::model::ProjectId;

const STEP: i64 = 40;
const COLUMNS: i64 = 3;
const CARD: Size = Size {
    width: 168,
    height: 84,
};

#[derive(Clone, Copy)]
pub struct OpenBoard {
    problem: RwSignal<Option<ApiError>>,
    board: RwSignal<Option<Board>>,
    pool: RwSignal<Option<Vec<Piece>>>,
    pinning: Action<(PieceId, Viewport), ()>,
    reshaping: Action<(PieceId, Option<Spot>, Option<Size>), ()>,
    unpinning: Action<PieceId, ()>,
    retitling: Action<(PieceId, String), ()>,
}

impl OpenBoard {
    pub fn open(project: &ProjectId) -> Self {
        let problem = RwSignal::new(None::<ApiError>);
        let board = RwSignal::new(None::<Board>);
        let pool = RwSignal::new(None::<Vec<Piece>>);

        let arrived = move |open: Board| {
            let known = board.with_untracked(|held| held.as_ref().map(|held| held.version));

            if known.is_none_or(|known| open.version >= known) {
                board.set(Some(open));
            }
        };

        let settled = move |answer: Result<Board, ApiError>| match answer {
            Ok(open) => arrived(open),
            Err(failure) => problem.set(Some(failure)),
        };

        let listing = {
            let id = project.clone();

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

        let opening = {
            let id = project.clone();

            Action::new_local(move |()| {
                let id = id.clone();

                async move { settled(service::open(&id).await) }
            })
        };
        opening.dispatch(());

        let pinning = Action::new_local(move |(piece, seen): &(PieceId, Viewport)| {
            let piece = piece.clone();
            let at = Placement {
                spot: next_spot(board.get_untracked().as_ref(), *seen),
                size: CARD,
            };
            held(board, &piece, at);

            async move {
                let Some(open) = board.get_untracked() else {
                    return;
                };

                match service::pin(&open.id, &piece, at).await {
                    Ok(pinned) => arrived(pinned),
                    Err(failure) => {
                        problem.set(Some(failure));
                        taken_off(board, &piece);
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
                        Ok(moved) => arrived(moved),
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
                    Ok(()) => taken_off(board, &piece),
                    Err(failure) => problem.set(Some(failure)),
                }
            }
        });

        let retitling = Action::new_local(move |(piece, title): &(PieceId, String)| {
            let piece = piece.clone();
            let title = title.clone();

            async move {
                match pieces::retitle(&piece, &title).await {
                    Ok(renamed) => pool.update(|held| {
                        if let Some(held) = held
                            && let Some(known) =
                                held.iter_mut().find(|known| known.id == renamed.id)
                        {
                            *known = renamed;
                        }
                    }),
                    Err(failure) => problem.set(Some(failure)),
                }
            }
        });

        Self {
            problem,
            board,
            pool,
            pinning,
            reshaping,
            unpinning,
            retitling,
        }
    }

    pub fn ready(&self) -> bool {
        self.board.get().is_some()
    }

    pub fn problem(&self) -> Option<ApiError> {
        self.problem.get()
    }

    pub fn dismiss(&self) {
        self.problem.set(None);
    }

    pub fn pinned(&self) -> Vec<(Piece, Placement)> {
        let pool = self.in_pool();

        self.board
            .get()
            .map(|open| {
                open.pieces
                    .into_iter()
                    .filter_map(|held| drawn(&held, &pool))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn unpinned(&self) -> Vec<Piece> {
        let Some(held) = self.board.get() else {
            return Vec::new();
        };

        self.in_pool()
            .into_iter()
            .filter(|piece| !held.holds(&piece.id))
            .collect()
    }

    pub fn pin(&self, piece: PieceId, seen: Viewport) {
        self.pinning.dispatch((piece, seen));
    }

    pub fn reshape(&self, piece: PieceId, to: Option<Spot>, size: Option<Size>) {
        self.reshaping.dispatch((piece, to, size));
    }

    pub fn unpin(&self, piece: PieceId) {
        self.unpinning.dispatch(piece);
    }

    pub fn retitle(&self, piece: PieceId, title: String) {
        self.retitling.dispatch((piece, title));
    }

    fn in_pool(&self) -> Vec<Piece> {
        self.pool.get().unwrap_or_default()
    }
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

fn taken_off(board: RwSignal<Option<Board>>, piece: &PieceId) {
    board.update(|open| {
        if let Some(open) = open {
            open.pieces.retain(|held| &held.piece != piece);
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
        let Some(open) = open else {
            return;
        };
        let Some(nth) = open.pieces.iter().position(|held| &held.piece == piece) else {
            return;
        };
        let held = &mut open.pieces[nth];

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

        if to.is_some() {
            let raised = open.pieces.remove(nth);
            open.pieces.push(raised);
        }
    });

    was
}

fn next_spot(board: Option<&Board>, seen: Viewport) -> Spot {
    let taken = board
        .map(|open| open.pieces.iter().map(|held| held.spot).collect::<Vec<_>>())
        .unwrap_or_default();
    let from = seen.on_board(Spot { x: 0, y: 0 });
    let mut nth = 0;

    while taken.contains(&slot(nth, from)) {
        nth += 1;
    }

    slot(nth, from)
}

fn slot(nth: i64, from: Spot) -> Spot {
    snapped(Spot {
        x: from.x + STEP + (nth % COLUMNS) * (STEP * 5),
        y: from.y + STEP + (nth / COLUMNS) * (STEP * 3),
    })
}
