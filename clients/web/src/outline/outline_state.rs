use leptos::prelude::*;
use leptos_router::hooks::use_navigate;

use crate::http::ApiError;
use crate::ideas::model::Idea;
use crate::ideas::service as idea_service;
use crate::outline::model::{Attachment, Outline, Section, SectionId};
use crate::outline::service;
use crate::passages::model::Passage;
use crate::passages::service as passage_service;
use crate::projects::model::ProjectId;
use crate::route;

#[derive(Clone, Copy)]
pub struct OutlineState {
    problem: RwSignal<Option<ApiError>>,
    outline: Memo<Option<Outline>>,
    passages: RwSignal<Option<Vec<Passage>>>,
    ideas: RwSignal<Option<Vec<Idea>>>,
    added: RwSignal<Option<SectionId>>,
    adding: Action<(Option<SectionId>, Option<SectionId>, String), ()>,
    retitling: Action<(SectionId, String), ()>,
    urging: Action<(SectionId, Urge), ()>,
    placing: Action<(SectionId, Option<SectionId>, Option<SectionId>), ()>,
    removing: Action<SectionId, ()>,
    attaching: Action<(Attachment, SectionId), ()>,
    detaching: Action<Attachment, ()>,
    writing: Action<SectionId, ()>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Urge {
    Earlier,
    Later,
    Promote,
    Demote,
}

impl OutlineState {
    pub fn open(project: &ProjectId) -> Self {
        let problem = RwSignal::new(None::<ApiError>);
        let confirmed = RwSignal::new(None::<Outline>);
        let retitles = RwSignal::new(Vec::<(u64, SectionId, String)>::new());
        let issued = StoredValue::new(0_u64);
        let outline = Memo::new(move |_| {
            confirmed.get().map(|mut open| {
                retitles.with(|pending| {
                    for (_, section, title) in pending {
                        if let Some(held) =
                            open.sections.iter_mut().find(|held| &held.id == section)
                        {
                            held.title = title.clone();
                        }
                    }
                });

                open
            })
        });
        let passages = RwSignal::new(None::<Vec<Passage>>);
        let ideas = RwSignal::new(None::<Vec<Idea>>);
        let added = RwSignal::new(None::<SectionId>);

        let arrived = move |told: Outline| {
            let known = confirmed.with_untracked(|held| held.as_ref().map(|held| held.version));

            if known.is_none_or(|known| told.version >= known) {
                confirmed.set(Some(told));
            }
        };

        let settled = move |answer: Result<Outline, ApiError>| match answer {
            Ok(told) => arrived(told),
            Err(failure) => problem.set(Some(failure)),
        };

        let listing_passages = {
            let id = project.clone();

            Action::new_local(move |()| {
                let id = id.clone();

                async move {
                    match passage_service::in_project(&id).await {
                        Ok(found) => passages.set(Some(found)),
                        Err(failure) => problem.set(Some(failure)),
                    }
                }
            })
        };
        listing_passages.dispatch(());

        let listing_ideas = {
            let id = project.clone();

            Action::new_local(move |()| {
                let id = id.clone();

                async move {
                    match idea_service::list(&id).await {
                        Ok(found) => ideas.set(Some(found)),
                        Err(failure) => problem.set(Some(failure)),
                    }
                }
            })
        };
        listing_ideas.dispatch(());

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
            let ticket = issued.get_value() + 1;
            issued.set_value(ticket);
            retitles.update(|pending| pending.push((ticket, section.clone(), title.clone())));

            async move {
                let Some(open) = outline.get_untracked() else {
                    return;
                };

                settled(service::retitle(&open.id, &section, &title).await);
                retitles.update(|pending| pending.retain(|(issued, _, _)| *issued != ticket));
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

        let attaching = Action::new_local(move |(attachment, to): &(Attachment, SectionId)| {
            let attachment = attachment.clone();
            let to = to.clone();

            async move {
                let Some(open) = outline.get_untracked() else {
                    return;
                };
                let behind = open
                    .sections
                    .iter()
                    .find(|section| section.id == to)
                    .and_then(|section| section.attachments.last().cloned());

                settled(service::attach(&open.id, &attachment, &to, behind).await);
            }
        });

        let detaching = Action::new_local(move |attachment: &Attachment| {
            let attachment = attachment.clone();

            async move {
                let Some(open) = outline.get_untracked() else {
                    return;
                };

                match service::detach(&open.id, &attachment).await {
                    Ok(()) => confirmed.update(|held| {
                        if let Some(held) = held {
                            for section in &mut held.sections {
                                section.attachments.retain(|held| held != &attachment);
                            }
                        }
                    }),
                    Err(failure) => problem.set(Some(failure)),
                }
            }
        });

        let writing = {
            let id = project.clone();

            Action::new_local(move |to: &SectionId| {
                let id = id.clone();
                let to = to.clone();
                let opening = use_navigate();

                async move {
                    let started = match passage_service::create(&id).await {
                        Ok(started) => started,
                        Err(failure) => return problem.set(Some(failure)),
                    };

                    passages.update(|held| held.get_or_insert_default().push(started.clone()));
                    attaching.dispatch((Attachment::Passage(started.id.clone()), to));
                    opening(
                        &route::passage(&id.to_string(), &started.id),
                        Default::default(),
                    );
                }
            })
        };

        Self {
            problem,
            outline,
            passages,
            ideas,
            added,
            adding,
            retitling,
            urging,
            placing,
            removing,
            attaching,
            detaching,
            writing,
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

    pub fn attachments_in(&self, section: &SectionId) -> Vec<Attachment> {
        self.outline
            .with(|held| {
                held.as_ref().and_then(|held| {
                    held.sections
                        .iter()
                        .find(|known| &known.id == section)
                        .map(|known| known.attachments.clone())
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

    pub fn waiting_passages(&self) -> Vec<Passage> {
        let Some(open) = self.outline.get() else {
            return Vec::new();
        };

        self.passages
            .get()
            .unwrap_or_default()
            .into_iter()
            .filter(|passage| !open.holds(&Attachment::Passage(passage.id.clone())))
            .collect()
    }

    pub fn waiting_ideas(&self) -> Vec<Idea> {
        let Some(open) = self.outline.get() else {
            return Vec::new();
        };

        self.ideas
            .get()
            .unwrap_or_default()
            .into_iter()
            .filter(|idea| !open.holds(&Attachment::Idea(idea.id.clone())))
            .collect()
    }

    pub fn named(&self, attachment: &Attachment) -> String {
        match attachment {
            Attachment::Passage(passage) => self
                .passages
                .get()
                .unwrap_or_default()
                .into_iter()
                .find(|held| &held.id == passage)
                .map(|held| held.shown_as())
                .unwrap_or_default(),
            Attachment::Idea(idea) => self
                .ideas
                .get()
                .unwrap_or_default()
                .into_iter()
                .find(|held| &held.id == idea)
                .map(|held| held.shown_as().to_owned())
                .unwrap_or_default(),
        }
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

    pub fn attach(&self, attachment: Attachment, to: SectionId) {
        self.attaching.dispatch((attachment, to));
    }

    pub fn detach(&self, attachment: Attachment) {
        self.detaching.dispatch(attachment);
    }

    pub fn write_in(&self, section: SectionId) {
        self.writing.dispatch(section);
    }
}
