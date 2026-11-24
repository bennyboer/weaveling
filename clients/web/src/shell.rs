use fluent_bundle::FluentArgs;
use leptos::either::Either;
use leptos::prelude::*;
use leptos::{IntoView, view};
use leptos::{ev, html, task};
use leptos_router::components::A;

use crate::accounts;
use crate::accounts::{CurrentSession, SignInState};
use crate::avatar;
use crate::feedback;
use crate::feedback::followed::FollowedFeedback;
use crate::icons;
use crate::icons::Icon;
use crate::members::listed::Member;
use crate::members::roles::Roles;
use crate::members::service as members;
use crate::members::sharing;
use crate::more_menu;
use crate::notifications::bell;
use crate::path;
use crate::projects::open_project::OpenProject;
use crate::projects::project_details;
use crate::refusals::alarm;
use crate::refusals::alarm::AlarmState;
use crate::route;
use crate::speech;
use crate::speech::Speech;
use crate::speech::picker;
use crate::theme;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Viewing {
    Board,
    Outline,
    Ideas,
    Feedback,
    Write,
    Read,
}

impl Viewing {
    fn message(&self) -> &'static str {
        match self {
            Self::Board => "view-board",
            Self::Outline => "view-outline",
            Self::Ideas => "view-ideas",
            Self::Feedback => "view-feedback",
            Self::Write => "view-write",
            Self::Read => "view-read",
        }
    }

    fn at(&self, project: &str) -> String {
        match self {
            Self::Board => route::board(project),
            Self::Outline => route::outline(project),
            Self::Ideas => route::pool(project),
            Self::Feedback => route::feedback(project),
            Self::Write => route::writing(project),
            Self::Read => route::reading(project),
        }
    }
}

pub struct Inside {
    project: String,
    named: String,
    here: Option<Viewing>,
}

impl Inside {
    pub fn of(project: &str, here: Option<Viewing>) -> Self {
        Self {
            project: project.to_owned(),
            named: route::named(project),
            here,
        }
    }
}

pub fn mark() -> impl IntoView {
    html::span().class("mark").attr("aria-hidden", "true")
}

pub fn masthead(inside: Option<Inside>) -> impl IntoView {
    let phone_sized = phone_sizing();
    let sharing = RwSignal::new(false);
    let this_project = inside.as_ref().map(|inside| inside.project.clone());
    let views = inside
        .as_ref()
        .map(|inside| Views::of(&inside.project, inside.here));

    let header = html::header().class("masthead").child((
        view! {
            <A href=route::WORKSPACE attr:class="wordmark" attr:aria-label=speech::said("masthead-all-projects")>
                {mark()}
                <span>"Weaveling"</span>
            </A>
        },
        inside
            .zip(views.clone())
            .map(|(inside, views)| whereabouts(inside, views, phone_sized, sharing)),
        move || {
            (!phone_sized.get()).then(|| (theme::picker(), picker::picker()))
        },
        bell::bell_in_masthead(),
        accounts::account_in_masthead(),
        move || {
            phone_sized
                .get()
                .then(|| more_menu::more_menu(this_project.clone(), sharing))
        },
    ));

    (header, path::path_bar(views))
}

const PHONE: &str = "(width <= 30rem)";

fn is_phone_sized() -> bool {
    window()
        .match_media(PHONE)
        .ok()
        .flatten()
        .is_some_and(|query| query.matches())
}

fn phone_sizing() -> RwSignal<bool> {
    let phone_sized = RwSignal::new(is_phone_sized());
    let resizing = window_event_listener(ev::resize, move |_| {
        let now = is_phone_sized();
        if phone_sized.get_untracked() != now {
            phone_sized.set(now);
        }
    });
    on_cleanup(move || resizing.remove());

    phone_sized
}

pub fn status_bar() -> impl IntoView {
    html::footer()
        .class("status-bar")
        .attr("aria-label", speech::said("status-bar"))
        .child(alarm::alarm(AlarmState::new()))
}

/// A project's views, the one open, and how much feedback is pending in it: for its tabs and for the path's switcher alike.
#[derive(Clone)]
pub struct Views {
    project: String,
    here: Option<Viewing>,
    pending: LocalResource<u32>,
}

impl Views {
    fn of(project: &str, here: Option<Viewing>) -> Self {
        let id = route::project_id(project);
        let followed = expect_context::<FollowedFeedback>();
        followed.follow(&id.to_string());
        let pending = LocalResource::new(move || {
            followed.track();
            let id = id.clone();

            async move {
                feedback::service::pending(&feedback::service::Asked::in_project(&id))
                    .await
                    .map(|pending| pending.total)
                    .unwrap_or(0)
            }
        });

        Self {
            project: project.to_owned(),
            here,
            pending,
        }
    }

    /// The views whoever is signed in may open, the writing view only for those who may write.
    fn shown(&self) -> Vec<Viewing> {
        let roles = expect_context::<Roles>();
        let writing = roles.may_write(&self.project);

        [
            Viewing::Write,
            Viewing::Read,
            Viewing::Board,
            Viewing::Outline,
            Viewing::Ideas,
            Viewing::Feedback,
        ]
        .into_iter()
        .filter(|view| writing || *view != Viewing::Write)
        .collect()
    }

    fn tabs(&self) -> Vec<impl IntoView + use<>> {
        self.shown()
            .into_iter()
            .map(|view| tab(view, self.project.clone(), self.here, self.pending))
            .collect()
    }
}

fn whereabouts(
    inside: Inside,
    views: Views,
    phone_sized: RwSignal<bool>,
    sharing: RwSignal<bool>,
) -> impl IntoView {
    let Inside { project, named, .. } = inside;
    let open_project = expect_context::<OpenProject>();
    let id = route::project_id(&project);
    open_project.open(&id);
    let shown = Signal::derive(move || open_project.name().unwrap_or_else(|| named.clone()));

    (
        html::div().class("whose-row").child((
            html::h1()
                .class("whose")
                .attr("title", shown)
                .child(html::span().child(shown)),
            project_details::project_details(id),
            {
                let project = project.clone();
                move || (!phone_sized.get()).then(|| members_link(project.clone()))
            },
            sharing::share_in_masthead(
                project.clone(),
                shown,
                sharing,
                Signal::derive(move || !phone_sized.get()),
            ),
        )),
        move || {
            (!phone_sized.get()).then(|| {
                html::nav()
                    .class("views")
                    .attr("aria-label", speech::said("masthead-views"))
                    .child(views.tabs())
            })
        },
    )
}

/// The view open, as a step of the path: a button naming it, opening a list of every view to switch to.
pub(crate) fn views_switch(views: Views, here: bool) -> impl IntoView {
    let open = RwSignal::new(false);
    let clicking_away = window_event_listener(ev::click, move |_| open.set(false));
    let escaping = window_event_listener(ev::keydown, move |event| {
        if event.key() == "Escape" {
            open.set(false);
        }
    });
    on_cleanup(move || {
        clicking_away.remove();
        escaping.remove();
    });
    let named = views.here.map_or("masthead-views", |view| view.message());
    let pending = views.pending;

    html::div()
        .class("views-switch")
        .on(ev::click, |event| event.stop_propagation())
        .child((
            html::button()
                .r#type("button")
                .class("views-button")
                .attr("aria-current", here.then_some("page"))
                .attr("aria-expanded", move || open.get().to_string())
                .on(ev::click, move |_| open.update(|open| *open = !*open))
                .child((
                    html::span().child(speech::said(named)),
                    (views.here != Some(Viewing::Feedback)).then(|| counted(pending)),
                    icons::mark(Icon::Open),
                )),
            move || {
                open.get().then(|| {
                    html::nav()
                        .class("views-list")
                        .attr("aria-label", speech::said("masthead-views"))
                        .on(ev::click, move |_| open.set(false))
                        .child(views.tabs())
                })
            },
        ))
}

/// How much feedback is still to be worked through, when there is any.
fn counted(pending: LocalResource<u32>) -> impl IntoView {
    move || {
        pending.get().filter(|count| *count > 0).map(|count| {
            let mut args = FluentArgs::new();
            args.set("count", count);

            view! {
                <span class="count" title=speech::say_with("feedback-pending-tab", &args)>
                    {count.to_string()}
                </span>
            }
        })
    }
}

const AVATARS_SHOWN: usize = 3;

fn members_link(project: String) -> impl IntoView {
    let session = expect_context::<CurrentSession>();
    let signed_in = Memo::new(move |_| matches!(session.state(), SignInState::SignedIn(_)));

    move || signed_in.get().then(|| members_stack(project.clone()))
}

fn members_stack(project: String) -> impl IntoView {
    let speech = expect_context::<Speech>();
    let known = RwSignal::new(Vec::<Member>::new());
    let id = route::project_id(&project).to_string();
    task::spawn_local(async move {
        if let Ok(found) = members::members_of(&id).await {
            _ = known.try_set(found.people);
        }
    });
    let labelled = move || {
        let held = known.get();
        if held.is_empty() {
            return speech.say("members-link");
        }
        let mut told = FluentArgs::new();
        told.set(
            "names",
            held.iter()
                .map(|member| {
                    member
                        .name
                        .clone()
                        .unwrap_or_else(|| speech.say("members-unnamed"))
                })
                .collect::<Vec<_>>()
                .join(", "),
        );
        speech.say_with("members-link-of", &told)
    };

    view! {
        <A
            href=route::members(&project)
            attr:class="members-link"
            attr:aria-label=labelled
            attr:title=labelled
        >
            {move || {
                let held = known.get();
                if held.is_empty() {
                    Either::Left(icons::mark(Icon::People))
                } else {
                    Either::Right(stacked(held))
                }
            }}
        </A>
    }
}

fn stacked(held: Vec<Member>) -> impl IntoView {
    let more = |shown: usize| {
        let beyond = held.len().saturating_sub(shown);
        (beyond > 0).then(|| format!("+{beyond}"))
    };
    let beyond = more(AVATARS_SHOWN);

    html::span().class("stack").child((
        held.iter()
            .take(AVATARS_SHOWN)
            .map(|member| {
                avatar::avatar(
                    &member.account,
                    member.name.as_deref(),
                    member.picture.as_deref(),
                )
            })
            .collect::<Vec<_>>(),
        beyond.map(|beyond| {
            html::span()
                .class("avatar more")
                .attr("aria-hidden", "true")
                .child(beyond)
        }),
    ))
}

/// A view's tab, the feedback's counting what is still to be worked through.
fn tab(
    view: Viewing,
    project: String,
    here: Option<Viewing>,
    pending: LocalResource<u32>,
) -> impl IntoView {
    let at = view.at(&project);
    let here = here == Some(view);
    let counted = (view == Viewing::Feedback).then(|| counted(pending));

    view! {
        <A
            href=at
            attr:class=move || if here { "view here" } else { "view" }
            attr:data-label=speech::said(view.message())
        >
            <span>{speech::said(view.message())}</span>
            {counted}
        </A>
    }
}
