use leptos::prelude::*;

use crate::http::ApiError;
use crate::outline::model::{Outline, Section, SectionId};
use crate::outline::service;
use crate::pieces::model::{Piece, PieceId};
use crate::pieces::service as pieces;
use crate::projects::model::ProjectId;

#[derive(Clone, Copy)]
pub struct OpenOutline {
    problem: RwSignal<Option<ApiError>>,
    outline: RwSignal<Option<Outline>>,
    pool: RwSignal<Option<Vec<Piece>>>,
    added: RwSignal<Option<SectionId>>,
    adding: Action<(Option<SectionId>, Option<SectionId>, String), ()>,
    retitling: Action<(SectionId, String), ()>,
    urging: Action<(SectionId, Urge), ()>,
    placing: Action<(SectionId, Option<SectionId>, Option<SectionId>), ()>,
    removing: Action<SectionId, ()>,
    attaching: Action<(PieceId, SectionId), ()>,
    detaching: Action<PieceId, ()>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Urge {
    Earlier,
    Later,
    Promote,
    Demote,
}

impl OpenOutline {
    pub fn open(project: &ProjectId) -> Self {
        let problem = RwSignal::new(None::<ApiError>);
        let outline = RwSignal::new(None::<Outline>);
        let pool = RwSignal::new(None::<Vec<Piece>>);
        let added = RwSignal::new(None::<SectionId>);

        let arrived = move |told: Outline| {
            let known = outline.with_untracked(|held| held.as_ref().map(|held| held.version));

            if known.is_none_or(|known| told.version >= known) {
                outline.set(Some(told));
            }
        };

        let settled = move |answer: Result<Outline, ApiError>| match answer {
            Ok(told) => arrived(told),
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

        let adding = Action::new_local(
            move |(under, after, title): &(Option<SectionId>, Option<SectionId>, String)| {
                let under = under.clone();
                let after = after.clone();
                let title = title.clone();

                async move {
                    let Some(open) = outline.get_untracked() else {
                        return;
                    };

                    match service::add(&open.id, under, after, &title).await {
                        Ok((section, told)) => {
                            arrived(told);
                            added.set(Some(section));
                        }
                        Err(failure) => problem.set(Some(failure)),
                    }
                }
            },
        );

        let retitling = Action::new_local(move |(section, title): &(SectionId, String)| {
            let section = section.clone();
            let title = title.clone();

            async move {
                let Some(open) = outline.get_untracked() else {
                    return;
                };

                settled(service::retitle(&open.id, &section, &title).await);
            }
        });

        let urging = Action::new_local(move |(section, urge): &(SectionId, Urge)| {
            let section = section.clone();
            let urge = *urge;

            async move {
                let Some(open) = outline.get_untracked() else {
                    return;
                };

                settled(match urge {
                    Urge::Promote => service::promote(&open.id, &section).await,
                    Urge::Demote => service::demote(&open.id, &section).await,
                    shifted => {
                        let later = shifted == Urge::Later;

                        service::place(
                            &open.id,
                            &section,
                            open.parent_of(&section),
                            open.landing_for(&section, later),
                        )
                        .await
                    }
                });
            }
        });

        let placing = Action::new_local(
            move |(section, under, after): &(SectionId, Option<SectionId>, Option<SectionId>)| {
                let section = section.clone();
                let under = under.clone();
                let after = after.clone();

                async move {
                    let Some(open) = outline.get_untracked() else {
                        return;
                    };

                    settled(service::place(&open.id, &section, under, after).await);
                }
            },
        );

        let removing = Action::new_local(move |section: &SectionId| {
            let section = section.clone();

            async move {
                let Some(open) = outline.get_untracked() else {
                    return;
                };

                settled(service::remove(&open.id, &section).await);
            }
        });

        let attaching = Action::new_local(move |(piece, to): &(PieceId, SectionId)| {
            let piece = piece.clone();
            let to = to.clone();

            async move {
                let Some(open) = outline.get_untracked() else {
                    return;
                };
                let behind = open
                    .sections
                    .iter()
                    .find(|section| section.id == to)
                    .and_then(|section| section.pieces.last().cloned());

                settled(service::attach(&open.id, &piece, &to, behind).await);
            }
        });

        let detaching = Action::new_local(move |piece: &PieceId| {
            let piece = piece.clone();

            async move {
                let Some(open) = outline.get_untracked() else {
                    return;
                };

                match service::detach(&open.id, &piece).await {
                    Ok(()) => outline.update(|held| {
                        if let Some(held) = held {
                            for section in &mut held.sections {
                                section.pieces.retain(|held| held != &piece);
                            }
                        }
                    }),
                    Err(failure) => problem.set(Some(failure)),
                }
            }
        });

        Self {
            problem,
            outline,
            pool,
            added,
            adding,
            retitling,
            urging,
            placing,
            removing,
            attaching,
            detaching,
        }
    }

    pub fn ready(&self) -> bool {
        self.outline.get().is_some()
    }

    pub fn problem(&self) -> Option<ApiError> {
        self.problem.get()
    }

    pub fn dismiss(&self) {
        self.problem.set(None);
    }

    pub fn sections(&self) -> Vec<Section> {
        self.outline
            .get()
            .map(|open| open.sections)
            .unwrap_or_default()
    }

    pub fn pieces_in(&self, section: &SectionId) -> Vec<PieceId> {
        self.outline
            .with(|held| {
                held.as_ref().and_then(|held| {
                    held.sections
                        .iter()
                        .find(|known| &known.id == section)
                        .map(|known| known.pieces.clone())
                })
            })
            .unwrap_or_default()
    }

    pub fn title_of(&self, section: &SectionId) -> String {
        self.outline
            .with(|held| {
                held.as_ref().and_then(|held| {
                    held.sections
                        .iter()
                        .find(|known| &known.id == section)
                        .map(|known| known.title.clone())
                })
            })
            .unwrap_or_default()
    }

    pub fn can(&self, section: &SectionId, urge: Urge) -> bool {
        self.outline
            .with(|held| {
                held.as_ref().map(|held| match urge {
                    Urge::Earlier => held.can_move_earlier(section),
                    Urge::Later => held.can_move_later(section),
                    Urge::Promote => held.can_promote(section),
                    Urge::Demote => held.can_demote(section),
                })
            })
            .unwrap_or_default()
    }

    pub fn unplaced(&self) -> Vec<Piece> {
        let Some(open) = self.outline.get() else {
            return Vec::new();
        };

        self.pool
            .get()
            .unwrap_or_default()
            .into_iter()
            .filter(|piece| !open.holds(&piece.id))
            .collect()
    }

    pub fn named(&self, piece: &PieceId) -> String {
        self.pool
            .get()
            .unwrap_or_default()
            .into_iter()
            .find(|held| &held.id == piece)
            .map(|held| held.shown_as().to_owned())
            .unwrap_or_default()
    }

    pub fn just_added(&self) -> Option<SectionId> {
        self.added.get()
    }

    pub fn settled_in(&self) {
        self.added.set(None);
    }

    pub fn add(&self, under: Option<SectionId>, after: Option<SectionId>, title: String) {
        self.adding.dispatch((under, after, title));
    }

    pub fn retitle(&self, section: SectionId, title: String) {
        self.retitling.dispatch((section, title));
    }

    pub fn place(&self, section: SectionId, under: Option<SectionId>, after: Option<SectionId>) {
        self.placing.dispatch((section, under, after));
    }

    pub fn landing_into(&self, section: &SectionId) -> Option<SectionId> {
        self.outline
            .with(|held| held.as_ref().and_then(|held| held.last_child_of(section)))
    }

    pub fn landing_before(&self, section: &SectionId) -> Option<SectionId> {
        self.outline
            .with(|held| held.as_ref().and_then(|held| held.before(section)))
    }

    pub fn parent_of(&self, section: &SectionId) -> Option<SectionId> {
        self.outline
            .with(|held| held.as_ref().and_then(|held| held.parent_of(section)))
    }

    pub fn would_swallow(&self, section: &SectionId, target: &SectionId) -> bool {
        self.outline
            .with(|held| {
                held.as_ref()
                    .map(|held| held.descends_from(target, section))
            })
            .unwrap_or_default()
    }

    pub fn urge(&self, section: SectionId, urge: Urge) {
        self.urging.dispatch((section, urge));
    }

    pub fn remove(&self, section: SectionId) {
        self.removing.dispatch(section);
    }

    pub fn attach(&self, piece: PieceId, to: SectionId) {
        self.attaching.dispatch((piece, to));
    }

    pub fn detach(&self, piece: PieceId) {
        self.detaching.dispatch(piece);
    }
}
