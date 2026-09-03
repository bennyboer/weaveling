use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, view};
use leptos_router::components::A;

use crate::boards::model::{Board, PositionedPiece, Spot};
use crate::boards::service;
use crate::http::ApiError;
use crate::pieces::model::Piece;
use crate::pieces::service as pieces;
use crate::route;

const STEP: i64 = 40;

#[component]
pub fn TheBoard(project: String) -> impl IntoView {
    let problem = RwSignal::new(None::<ApiError>);
    let board = RwSignal::new(None::<Board>);
    let id = route::project_id(&project);

    let pool = {
        let id = id.clone();

        LocalResource::new(move || {
            let id = id.clone();

            async move { pieces::list(&id).await }
        })
    };

    let opening = {
        let id = id.clone();

        Action::new_local(move |()| {
            let id = id.clone();

            async move {
                match service::open(&id).await {
                    Ok(opened) => {
                        problem.set(None);
                        board.set(Some(opened));
                    }
                    Err(failure) => problem.set(Some(failure)),
                }
            }
        })
    };
    opening.dispatch(());

    let pinning = Action::new_local(move |piece: &Piece| {
        let piece = piece.id.clone();
        let at = next_spot(board.get_untracked().as_ref());

        async move {
            let Some(open) = board.get_untracked() else {
                return;
            };

            match service::pin(&open.id, &piece, at).await {
                Ok(pinned) => {
                    problem.set(None);
                    board.set(Some(pinned));
                }
                Err(failure) => problem.set(Some(failure)),
            }
        }
    });

    let in_pool = move || match pool.get() {
        Some(found) => found.as_ref().cloned().unwrap_or_default(),
        None => Vec::new(),
    };

    let unpinned = move || {
        let held = board.get();

        in_pool()
            .into_iter()
            .filter(|piece| held.as_ref().is_none_or(|open| !open.holds(&piece.id)))
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
            .child(move || {
                let project = project.clone();

                pinned()
                    .into_iter()
                    .map(|(piece, at)| {
                        card(route::piece(&project, &piece.id, &piece.title), piece, at)
                    })
                    .collect::<Vec<_>>()
            }),
        html::section().class("unpinned").child((
            html::h3().child("Not on the board"),
            move || {
                let waiting = unpinned();

                (board.get().is_some() && waiting.is_empty()).then(|| {
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
                        .collect_view()
                }),
        )),
    ))
}

fn drawn(held: &PositionedPiece, pool: &[Piece]) -> Option<(Piece, Spot)> {
    pool.iter()
        .find(|piece| piece.id == held.piece)
        .map(|piece| (piece.clone(), held.spot))
}

fn card(href: String, piece: Piece, at: Spot) -> impl IntoView {
    let shown = piece.shown_as().to_owned();
    let placed = format!("left: {}px; top: {}px;", at.x, at.y);

    html::article()
        .class("pinned")
        .attr("style", placed)
        .child(view! {
            <A href=href attr:class="name">
                {shown}
            </A>
        })
}

fn pinnable(piece: Piece, pinning: Action<Piece, ()>) -> impl IntoView {
    let shown = piece.shown_as().to_owned();

    html::li().child(
        html::button()
            .r#type("button")
            .attr("aria-label", format!("Pin {shown}"))
            .on(leptos::ev::click, move |_| {
                pinning.dispatch(piece.clone());
            })
            .child(shown),
    )
}

fn next_spot(board: Option<&Board>) -> Spot {
    let taken = board.map(|open| open.pieces.len() as i64).unwrap_or(0);

    Spot {
        x: STEP + (taken % 5) * (STEP * 4),
        y: STEP + (taken / 5) * (STEP * 3),
    }
}
