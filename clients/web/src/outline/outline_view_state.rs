use leptos::prelude::*;
use leptos_router::hooks;

use crate::act::{Act, Here};
use crate::http::ApiError;
use crate::ideas::followed;
use crate::ideas::model::Idea;
use crate::live::Subscription;
use crate::outline::model::{Attachment, Outline, Section, SectionId};
use crate::outline::service;
use crate::projects::model::ProjectId;
use crate::route;
use crate::scenes::followed as scenes_followed;
use crate::scenes::model;
use crate::scenes::model::SceneSummary;
use crate::scenes::service as scene_service;
use crate::working_copy;
use crate::working_copy::{Versioned, WorkingCopy};
use live_contract::FrameDTO;
use outline_contract::OutlineEventDTO;

#[derive(Clone, Copy)]
pub struct OutlineViewState {
    problem: RwSignal<Option<ApiError>>,
    outline: Memo<Option<Outline>>,
    scenes: RwSignal<Option<Vec<SceneSummary>>>,
    ideas: RwSignal<Option<Vec<Idea>>>,
    added: RwSignal<Option<SectionId>>,
    working: WorkingCopy<Outline, Intent>,
    writing: Act<SectionId>,
}

#[derive(Clone, Debug, PartialEq)]
enum Intent {
    Add {
        under: Option<SectionId>,
        after: Option<SectionId>,
        title: String,
    },
    Retitle {
        section: SectionId,
        title: String,
    },
    Urge {
        section: SectionId,
        urge: Urge,
    },
    Place {
        section: SectionId,
        under: Option<SectionId>,
        after: Option<SectionId>,
    },
    Remove {
        section: SectionId,
    },
    Attach {
        attachment: Attachment,
        to: SectionId,
    },
    Detach {
        attachment: Attachment,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Urge {
    Earlier,
    Later,
    Promote,
    Demote,
}

impl OutlineViewState {
    pub fn open(project: &ProjectId) -> Self {
        let problem = RwSignal::new(None::<ApiError>);
        let added = RwSignal::new(None::<SectionId>);
        let working = {
            let (sending_to, reading) = (project.clone(), project.clone());

            WorkingCopy::new(
                move |on: Outline, intent: Intent| {
                    let project = sending_to.clone();

                    async move { intent.made_on(&on, &project, added).await }
                },
                move || {
                    let project = reading.clone();

                    async move { service::open(&project).await }
                },
                move |failure| _ = problem.try_set(Some(failure)),
            )
        };
        let outline = Memo::new(move |_| working.shown());
        let scenes = RwSignal::new(None::<Vec<SceneSummary>>);
        let ideas = RwSignal::new(None::<Vec<Idea>>);
        working.read_again();

        let (scenes_followed, listing_scenes) = scenes_followed::follow(
            project,
            move |listed| _ = scenes.try_set(Some(listed.to_vec())),
            move |summary| {
                scenes.update(|held| {
                    if let Some(held) = held {
                        model::hold(held, summary.clone());
                    }
                })
            },
            move |failure| _ = problem.try_set(Some(failure)),
        );
        StoredValue::new_local(scenes_followed);

        let listing_ideas = followed::asking(project, ideas, move |failure| {
            _ = problem.try_set(Some(failure));
        });
        StoredValue::new_local(followed::following(project, ideas, listing_ideas));

        let writing = {
            let id = project.clone();
            let opening = hooks::use_navigate();

            Act::new(move |to: SectionId, here: Here| {
                let id = id.clone();
                let opening = opening.clone();

                async move {
                    let started = match scene_service::create(&id).await {
                        Ok(started) => started,
                        Err(failure) => return problem.set(Some(failure)),
                    };

                    scenes.update(|held| {
                        model::hold(held.get_or_insert_default(), started.summarized())
                    });
                    let placed = working
                        .intended(Intent::Attach {
                            attachment: Attachment::Scene(started.id.clone()),
                            to,
                        })
                        .await;

                    if placed && here.still() {
                        opening(
                            &route::writing_at(&id.to_string(), &started.id),
                            Default::default(),
                        );
                    }
                }
            })
        };

        let following = StoredValue::new_local(None::<Subscription>);
        Effect::new(move || {
            let Some(id) = working.confirmed().map(|held| held.id) else {
                return;
            };
            if following.with_value(Option::is_some) {
                return;
            }
            following.set_value(Some(Subscription::start(
                "outlines",
                &[("outline", id.to_string().as_str())],
                move |told: FrameDTO| {
                    let Some(held) = working.confirmed_untracked() else {
                        return;
                    };
                    if told.version <= held.version {
                        return;
                    }
                    let next = (told.version == held.version + 1)
                        .then(|| serde_json::from_value::<OutlineEventDTO>(told.event).ok())
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

        Effect::new(move || {
            let Some(attached) = working.confirmed().map(|held| {
                held.sections
                    .iter()
                    .flat_map(|section| section.attachments.clone())
                    .collect::<Vec<_>>()
            }) else {
                return;
            };
            let unknown_scene = untrack(|| {
                scenes.with(|known| {
                    known.as_ref().is_some_and(|known| {
                        attached.iter().any(|attached| match attached {
                            Attachment::Scene(scene) => known.iter().all(|held| &held.id != scene),
                            Attachment::Idea(_) => false,
                        })
                    })
                })
            });
            let unknown_idea = untrack(|| {
                ideas.with(|known| {
                    known.as_ref().is_some_and(|known| {
                        attached.iter().any(|attached| match attached {
                            Attachment::Idea(idea) => known.iter().all(|held| &held.id != idea),
                            Attachment::Scene(_) => false,
                        })
                    })
                })
            });
            if unknown_scene {
                listing_scenes();
            }
            if unknown_idea {
                listing_ideas.dispatch(None);
            }
        });

        Self {
            problem,
            outline,
            scenes,
            ideas,
            added,
            working,
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

    pub fn waiting_scenes(&self) -> Vec<SceneSummary> {
        let Some(open) = self.outline.get() else {
            return Vec::new();
        };

        self.scenes
            .get()
            .unwrap_or_default()
            .into_iter()
            .filter(|scene| !open.holds(&Attachment::Scene(scene.id.clone())))
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
            Attachment::Scene(scene) => self
                .scenes
                .get()
                .unwrap_or_default()
                .into_iter()
                .find(|held| &held.id == scene)
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
        self.working.intend(Intent::Add {
            under,
            after,
            title,
        });
    }

    pub fn retitle(&self, section: SectionId, title: String) {
        self.working.intend(Intent::Retitle { section, title });
    }

    pub fn place(&self, section: SectionId, under: Option<SectionId>, after: Option<SectionId>) {
        self.working.intend(Intent::Place {
            section,
            under,
            after,
        });
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
        self.working.intend(Intent::Urge { section, urge });
    }

    pub fn remove(&self, section: SectionId) {
        self.working.intend(Intent::Remove { section });
    }

    pub fn attach(&self, attachment: Attachment, to: SectionId) {
        self.working.intend(Intent::Attach { attachment, to });
    }

    pub fn detach(&self, attachment: Attachment) {
        self.working.intend(Intent::Detach { attachment });
    }

    pub fn write_in(&self, section: SectionId) {
        self.writing.start(section);
    }

    pub fn writing_in(&self, section: &SectionId) -> Option<&'static str> {
        self.writing.busy_on(section)
    }
}

impl Versioned for Outline {
    fn version(&self) -> u64 {
        self.version
    }
}

impl working_copy::Intent<Outline> for Intent {
    fn lay_over(&self, open: &mut Outline) {
        match self {
            Self::Retitle { section, title } => {
                if let Some(held) = open.sections.iter_mut().find(|held| &held.id == section) {
                    held.title = title.clone();
                }
            }
            Self::Detach { attachment } => {
                for section in &mut open.sections {
                    section.attachments.retain(|held| held != attachment);
                }
            }
            Self::Add { .. }
            | Self::Urge { .. }
            | Self::Place { .. }
            | Self::Remove { .. }
            | Self::Attach { .. } => {}
        }
    }

    fn merged_after(&self, earlier: &Self) -> Option<Self> {
        match (self, earlier) {
            (
                Self::Retitle { section, .. },
                Self::Retitle {
                    section: before, ..
                },
            ) if section == before => Some(self.clone()),
            _ => None,
        }
    }
}

impl Intent {
    async fn made_on(
        &self,
        on: &Outline,
        project: &ProjectId,
        added: RwSignal<Option<SectionId>>,
    ) -> Result<Outline, ApiError> {
        let version = Some(on.version);
        let told = match self {
            Self::Add {
                under,
                after,
                title,
            } => {
                let (section, told) =
                    service::add(&on.id, under.clone(), after.clone(), title, version).await?;
                _ = added.try_set(Some(section));
                told
            }
            Self::Retitle { section, title } => {
                service::retitle(&on.id, section, title, version).await?
            }
            Self::Urge { section, urge } => match urge {
                Urge::Promote => service::promote(&on.id, section, version).await?,
                Urge::Demote => service::demote(&on.id, section, version).await?,
                shifted => {
                    service::place(
                        &on.id,
                        section,
                        on.parent_of(section),
                        on.landing_for(section, *shifted == Urge::Later),
                        version,
                    )
                    .await?
                }
            },
            Self::Place {
                section,
                under,
                after,
            } => service::place(&on.id, section, under.clone(), after.clone(), version).await?,
            Self::Remove { section } => service::remove(&on.id, section, version).await?,
            Self::Attach { attachment, to } => {
                let behind = on
                    .sections
                    .iter()
                    .find(|section| &section.id == to)
                    .and_then(|section| section.attachments.last().cloned());

                service::attach(&on.id, attachment, to, behind, version).await?
            }
            Self::Detach { attachment } => {
                service::detach(&on.id, attachment, version).await?;

                return service::open(project).await;
            }
        };

        match on.fits(&told) {
            true => Ok(on.changed_by(told)),
            false => service::open(project).await,
        }
    }
}
