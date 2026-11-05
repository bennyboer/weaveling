use leptos::prelude::*;

use crate::boards::carrying;
use crate::boards::carrying::CARD;
use crate::boards::model::{Board, Placement, PositionedIdea, Size, Spot};
use crate::boards::service;
use crate::boards::viewport::Viewport;
use crate::http::ApiError;
use crate::ideas::followed;
use crate::ideas::model;
use crate::ideas::model::{Idea, IdeaId};
use crate::ideas::service as ideas;
use crate::live::Subscription;
use crate::projects::model::ProjectId;
use crate::working_copy;
use crate::working_copy::{Versioned, WorkingCopy};
use boards_contract::BoardEventDTO;
use live_contract::FrameDTO;

const STEP: i64 = 40;
const COLUMNS: i64 = 3;

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
pub struct BoardViewState {
    problem: RwSignal<Option<ApiError>>,
    board: Memo<Option<Board>>,
    pool: RwSignal<Option<Vec<Idea>>>,
    working: WorkingCopy<Board, Intent>,
    capturing: Action<(String, Placement), ()>,
    retitling: Action<(IdeaId, String), ()>,
}

impl BoardViewState {
    pub fn open(project: &ProjectId) -> Self {
        let problem = RwSignal::new(None::<ApiError>);
        let working = {
            let (sending_to, reading) = (project.clone(), project.clone());

            WorkingCopy::new(
                move |on: Board, intent: Intent| {
                    let project = sending_to.clone();

                    async move { intent.made_on(&on, &project).await }
                },
                move || {
                    let project = reading.clone();

                    async move { service::open(&project).await }
                },
                move |failure| _ = problem.try_set(Some(failure)),
            )
        };
        let board = Memo::new(move |_| working.shown());
        let pool = RwSignal::new(None::<Vec<Idea>>);

        let listing = followed::asking(project, pool, move |failure| {
            _ = problem.try_set(Some(failure));
        });
        StoredValue::new_local(followed::following(project, pool, listing));

        working.read_again();

        let following = StoredValue::new_local(None::<Subscription>);
        Effect::new(move || {
            let Some(id) = working.confirmed().map(|held| held.id) else {
                return;
            };
            if following.with_value(Option::is_some) {
                return;
            }
            following.set_value(Some(Subscription::start(
                "boards",
                &[("board", id.to_string().as_str())],
                move |told: FrameDTO| {
                    let Some(held) = working.confirmed_untracked() else {
                        return;
                    };
                    if told.version <= held.version {
                        return;
                    }
                    let next = (told.version == held.version + 1)
                        .then(|| serde_json::from_value::<BoardEventDTO>(told.event).ok())
                        .flatten()
                        .and_then(|event| held.taken_in(event));
                    match next {
                        Some(next) => working.take_in(next),
                        None => working.read_again(),
                    }
                },
                || {},
            )));
        });

        let listed_again_at = StoredValue::new(None::<u64>);
        Effect::new(move || {
            let Some(version) = working.confirmed().map(|held| held.version) else {
                return;
            };
            let unknown = pool.with(|held| {
                held.as_ref().is_some_and(|known| {
                    working.confirmed_untracked().is_some_and(|open| {
                        open.ideas
                            .iter()
                            .any(|pinned| known.iter().all(|idea| idea.id != pinned.idea))
                    })
                })
            });
            if unknown && listed_again_at.get_value() != Some(version) {
                listed_again_at.set_value(Some(version));
                listing.dispatch(None);
            }
        });

        let capturing = {
            let id = project.clone();

            Action::new_local(move |(title, at): &(String, Placement)| {
                let project = id.clone();
                let title = title.clone();
                let at = *at;

                async move {
                    if board.get_untracked().is_none() {
                        return;
                    }

                    let caught = match ideas::capture(&project, &title).await {
                        Ok(caught) => caught,
                        Err(failure) => return problem.set(Some(failure)),
                    };
                    let idea = caught.id.clone();
                    pool.update(|held| {
                        if let Some(held) = held {
                            model::hold(held, caught);
                        }
                    });

                    working.intend(Intent::Pin { idea, at });
                }
            })
        };

        let retitling = Action::new_local(move |(idea, title): &(IdeaId, String)| {
            let idea = idea.clone();
            let title = title.clone();

            async move {
                match ideas::retitle(&idea, &title).await {
                    Ok(renamed) => pool.update(|held| {
                        if let Some(held) = held {
                            model::hold(held, renamed);
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
            working,
            capturing,
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
        let at = Placement {
            spot: next_spot(self.board.get_untracked().as_ref(), seen),
            size: CARD,
        };

        self.working.intend(Intent::Pin { idea, at });
    }

    pub fn reshape(&self, idea: IdeaId, to: Option<Spot>, size: Option<Size>) {
        self.working.intend(Intent::Reshape { idea, to, size });
    }

    pub fn unpin(&self, idea: IdeaId) {
        self.working.intend(Intent::Unpin { idea });
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

impl Versioned for Board {
    fn version(&self) -> u64 {
        self.version
    }
}

impl working_copy::Intent<Board> for Intent {
    fn lay_over(&self, open: &mut Board) {
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

    fn merged_after(&self, earlier: &Self) -> Option<Self> {
        match (self, earlier) {
            (
                Self::Reshape { idea, to, size },
                Self::Reshape {
                    idea: before,
                    to: earlier_to,
                    size: earlier_size,
                },
            ) if idea == before => Some(Self::Reshape {
                idea: idea.clone(),
                to: to.or(*earlier_to),
                size: size.or(*earlier_size),
            }),
            _ => None,
        }
    }
}

impl Intent {
    async fn made_on(&self, on: &Board, project: &ProjectId) -> Result<Board, ApiError> {
        match self {
            Self::Pin { idea, at } => service::pin(&on.id, idea, *at, on.version).await,
            Self::Reshape { idea, to, size } => {
                service::reshape(&on.id, idea, *to, *size, on.version).await
            }
            Self::Unpin { idea } => {
                service::unpin(&on.id, idea, on.version).await?;
                service::open(project).await
            }
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
    carrying::snapped(Spot {
        x: from.x + STEP + (nth % COLUMNS) * (STEP * 5),
        y: from.y + STEP + (nth / COLUMNS) * (STEP * 3),
    })
}
