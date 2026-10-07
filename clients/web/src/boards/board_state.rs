use leptos::prelude::*;

use crate::boards::carrying::{CARD, snapped};
use crate::boards::model::{Board, Placement, PositionedIdea, Size, Spot};
use crate::boards::service;
use crate::boards::viewport::Viewport;
use crate::http::ApiError;
use crate::ideas::model::{Idea, IdeaId};
use crate::ideas::service as ideas;
use crate::projects::model::ProjectId;

const STEP: i64 = 40;
const COLUMNS: i64 = 3;

type Ticket = u64;

#[derive(Clone, Debug, PartialEq)]
enum Intent {
    Pin {
        idea: IdeaId,
        at: Placement,
    },
    Reshape {
        idea: IdeaId,
        to: Option<Spot>,
        size: Option<Size>,
    },
    Unpin {
        idea: IdeaId,
    },
}

#[derive(Clone, Copy)]
struct Intentions {
    pending: RwSignal<Vec<(Ticket, Intent)>>,
    issued: StoredValue<Ticket>,
}

#[derive(Clone, Copy)]
pub struct BoardState {
    problem: RwSignal<Option<ApiError>>,
    board: Memo<Option<Board>>,
    pool: RwSignal<Option<Vec<Idea>>>,
    capturing: Action<(String, Placement), ()>,
    pinning: Action<(IdeaId, Viewport), ()>,
    reshaping: Action<(IdeaId, Option<Spot>, Option<Size>), ()>,
    unpinning: Action<IdeaId, ()>,
    retitling: Action<(IdeaId, String), ()>,
}

impl BoardState {
    pub fn open(project: &ProjectId) -> Self {
        let problem = RwSignal::new(None::<ApiError>);
        let confirmed = RwSignal::new(None::<Board>);
        let intentions = Intentions {
            pending: RwSignal::new(Vec::new()),
            issued: StoredValue::new(0),
        };
        let board = Memo::new(move |_| {
            confirmed.get().map(|mut open| {
                intentions.pending.with(|pending| {
                    for (_, intent) in pending {
                        intent.apply(&mut open);
                    }
                });

                open
            })
        });
        let pool = RwSignal::new(None::<Vec<Idea>>);

        let arrived = move |open: Board| {
            let known = confirmed.with_untracked(|held| held.as_ref().map(|held| held.version));

            if known.is_none_or(|known| open.version >= known) {
                intentions.pending.update(|pending| {
                    pending.retain(|(_, intent)| match intent {
                        Intent::Unpin { idea } => open.holds(idea),
                        Intent::Pin { .. } | Intent::Reshape { .. } => true,
                    });
                });
                confirmed.set(Some(open));
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
                    match ideas::list(&id).await {
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

        let capturing = {
            let id = project.clone();

            Action::new_local(move |(title, at): &(String, Placement)| {
                let project = id.clone();
                let title = title.clone();
                let at = *at;

                async move {
                    let Some(open) = board.get_untracked() else {
                        return;
                    };

                    let caught = match ideas::capture(&project, &title).await {
                        Ok(caught) => caught,
                        Err(failure) => return problem.set(Some(failure)),
                    };
                    let idea = caught.id.clone();
                    pool.update(|held| {
                        if let Some(held) = held {
                            held.push(caught);
                        }
                    });

                    match service::pin(&open.id, &idea, at).await {
                        Ok(pinned) => arrived(pinned),
                        Err(failure) => problem.set(Some(failure)),
                    }
                }
            })
        };

        let pinning = Action::new_local(move |(idea, seen): &(IdeaId, Viewport)| {
            let idea = idea.clone();
            let at = Placement {
                spot: next_spot(board.get_untracked().as_ref(), *seen),
                size: CARD,
            };
            let ticket = intentions.intend(Intent::Pin {
                idea: idea.clone(),
                at,
            });

            async move {
                let Some(open) = board.get_untracked() else {
                    return;
                };

                match service::pin(&open.id, &idea, at).await {
                    Ok(pinned) => arrived(pinned),
                    Err(failure) => problem.set(Some(failure)),
                }
                intentions.settle(ticket);
            }
        });

        let reshaping = Action::new_local(
            move |(idea, to, size): &(IdeaId, Option<Spot>, Option<Size>)| {
                let idea = idea.clone();
                let to = *to;
                let size = *size;
                let ticket = intentions.intend(Intent::Reshape {
                    idea: idea.clone(),
                    to,
                    size,
                });

                async move {
                    let Some(open) = board.get_untracked() else {
                        return;
                    };

                    match service::reshape(&open.id, &idea, to, size).await {
                        Ok(moved) => arrived(moved),
                        Err(failure) => problem.set(Some(failure)),
                    }
                    intentions.settle(ticket);
                }
            },
        );

        let unpinning = Action::new_local(move |idea: &IdeaId| {
            let idea = idea.clone();

            async move {
                let Some(open) = board.get_untracked() else {
                    return;
                };

                match service::unpin(&open.id, &idea).await {
                    Ok(()) => {
                        intentions.intend(Intent::Unpin { idea });
                    }
                    Err(failure) => problem.set(Some(failure)),
                }
            }
        });

        let retitling = Action::new_local(move |(idea, title): &(IdeaId, String)| {
            let idea = idea.clone();
            let title = title.clone();

            async move {
                match ideas::retitle(&idea, &title).await {
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
            capturing,
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

    pub fn pinned(&self) -> Vec<(Idea, Placement)> {
        let pool = self.in_pool();

        self.board
            .get()
            .map(|open| {
                open.ideas
                    .into_iter()
                    .filter_map(|held| drawn(&held, &pool))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn unpinned(&self) -> Vec<Idea> {
        let Some(held) = self.board.get() else {
            return Vec::new();
        };

        self.in_pool()
            .into_iter()
            .filter(|idea| !held.holds(&idea.id))
            .collect()
    }

    pub fn capture(&self, title: String, at: Placement) {
        self.capturing.dispatch((title, at));
    }

    pub fn pin(&self, idea: IdeaId, seen: Viewport) {
        self.pinning.dispatch((idea, seen));
    }

    pub fn reshape(&self, idea: IdeaId, to: Option<Spot>, size: Option<Size>) {
        self.reshaping.dispatch((idea, to, size));
    }

    pub fn unpin(&self, idea: IdeaId) {
        self.unpinning.dispatch(idea);
    }

    pub fn retitle(&self, idea: IdeaId, title: String) {
        self.retitling.dispatch((idea, title));
    }

    pub fn idea(&self, id: &IdeaId) -> Option<Idea> {
        self.pool
            .with(|held| held.as_ref()?.iter().find(|known| &known.id == id).cloned())
    }

    pub fn adopt(&self, newer: Idea) {
        let older = self.pool.with_untracked(|held| {
            held.as_ref().is_some_and(|held| {
                held.iter()
                    .any(|known| known.id == newer.id && known.version < newer.version)
            })
        });

        if older {
            self.pool.update(|held| {
                if let Some(held) = held
                    && let Some(known) = held.iter_mut().find(|known| known.id == newer.id)
                {
                    *known = newer;
                }
            });
        }
    }

    fn in_pool(&self) -> Vec<Idea> {
        self.pool.get().unwrap_or_default()
    }
}

fn drawn(held: &PositionedIdea, pool: &[Idea]) -> Option<(Idea, Placement)> {
    pool.iter().find(|idea| idea.id == held.idea).map(|idea| {
        (
            idea.clone(),
            Placement {
                spot: held.spot,
                size: held.size,
            },
        )
    })
}

impl Intentions {
    fn intend(self, intent: Intent) -> Ticket {
        let ticket = self.issued.get_value() + 1;
        self.issued.set_value(ticket);

        self.pending.update(|pending| {
            if let Intent::Pin { idea, .. } = &intent {
                pending.retain(|(_, earlier)| {
                    !matches!(earlier, Intent::Unpin { idea: unpinned } if unpinned == idea)
                });
            }
            pending.push((ticket, intent));
        });

        ticket
    }

    fn settle(self, ticket: Ticket) {
        self.pending
            .update(|pending| pending.retain(|(issued, _)| *issued != ticket));
    }
}

impl Intent {
    fn apply(&self, open: &mut Board) {
        match self {
            Self::Pin { idea, at } => {
                if !open.holds(idea) {
                    open.ideas.push(PositionedIdea {
                        idea: idea.clone(),
                        spot: at.spot,
                        size: at.size,
                    });
                }
            }
            Self::Reshape { idea, to, size } => {
                let Some(nth) = open.ideas.iter().position(|held| &held.idea == idea) else {
                    return;
                };
                let held = &mut open.ideas[nth];

                if let Some(to) = to {
                    held.spot = *to;
                }

                if let Some(size) = size {
                    held.size = *size;
                }

                if to.is_some() {
                    let raised = open.ideas.remove(nth);
                    open.ideas.push(raised);
                }
            }
            Self::Unpin { idea } => open.ideas.retain(|held| &held.idea != idea),
        }
    }
}

fn next_spot(board: Option<&Board>, seen: Viewport) -> Spot {
    let taken = board
        .map(|open| open.ideas.iter().map(|held| held.spot).collect::<Vec<_>>())
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
