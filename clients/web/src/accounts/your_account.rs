use accounts_contract::refused;
use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::task;
use leptos::{IntoView, component, ev, html, view};
use leptos_router::components::A;
use time::{Duration, OffsetDateTime};

use crate::accounts::{
    CurrentSession, SecondFactorInUse, SignInState, SignedInAccount, picture, service,
};
use crate::act::{Act, Here};
use crate::assets::service as assets;
use crate::assets::storage_page;
use crate::dialog;
use crate::http::{ApiError, Particular};
use crate::moment;
use crate::problem;
use crate::projects::service as projects;
use crate::route;
use crate::shell;
use crate::speech;
use crate::speech::Speech;

const KEPT_BEFORE_DELETION: Duration = Duration::days(30);

/// What the account deletion shows.
/// It shows nothing yet, the confirmation question, or the projects that must be handed over first.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Deleting {
    Not,
    Asking,
    HandingOver(Vec<ToHandOver>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToHandOver {
    id: String,
    name: String,
}

#[component]
pub fn YourAccountPage() -> impl IntoView {
    let session = expect_context::<CurrentSession>();

    (
        shell::masthead(None),
        html::main()
            .class("operating your-account")
            .child(move || match session.state.get() {
                SignInState::SignedIn(account) => Some(overview(session, account)),
                _ => None,
            }),
    )
}

fn overview(session: CurrentSession, account: SignedInAccount) -> impl IntoView {
    let deleting = RwSignal::new(Deleting::Not);
    let close = move || deleting.set(Deleting::Not);
    let security_offered =
        session.totp_offered() || session.keys_offered() || account.second_factor.is_some();

    (
        html::h1().child(speech::said("your-account-heading")),
        html::div().class("account-cards").child((
            picture::profile_card(session, account.clone()),
            security_offered.then(|| security_card(account.second_factor)),
            storage_card(),
        )),
        html::section().class("card deleting").child((
            html::div().child((
                html::h2().child(speech::said("your-account-delete-heading")),
                html::p()
                    .class("hint")
                    .child(speech::said("your-account-delete-hint")),
            )),
            html::button()
                .r#type("button")
                .class("danger")
                .on(ev::click, move |_| deleting.set(Deleting::Asking))
                .child(speech::said("your-account-delete")),
        )),
        move || match deleting.get() {
            Deleting::Not => None,
            Deleting::Asking => Some(asking(session, deleting, close).into_any()),
            Deleting::HandingOver(projects) => Some(handing_over(projects, close).into_any()),
        },
    )
}

/// The card that says how the account signs in, and leads to its security page.
fn security_card(factor: Option<SecondFactorInUse>) -> impl IntoView {
    let on = factor.is_some();
    let details = move || {
        let factor = factor.as_ref()?;
        let mut ways = Vec::new();
        if factor.answered_by_totp {
            ways.push(speech::say("security-app"));
        }
        if factor.answered_by_key {
            ways.push(speech::say("another-way-key"));
        }
        let mut args = FluentArgs::new();
        args.set("left", factor.recovery_codes_left);
        ways.push(speech::say_with("your-account-codes-left", &args));

        Some(ways.join(" · "))
    };

    html::section().class("card security").child((
        html::h2().child(speech::said("security-heading")),
        html::p().class("status").child(if on {
            speech::said("your-account-security-on")
        } else {
            speech::said("your-account-security-off")
        }),
        move || details().map(|details| html::p().class("hint").child(details)),
        view! { <A href=route::SECURITY>{speech::said("your-account-security-manage")}</A> },
    ))
}

/// The card that says how much storage the account's media use, and leads to them.
fn storage_card() -> impl IntoView {
    let speech = expect_context::<Speech>();
    let usage = RwSignal::new(None);
    task::spawn_local(async move {
        if let Ok(told) = assets::usage().await {
            usage.set(Some(told));
        }
    });

    html::section().class("card storage").child((
        html::h2().child(speech::said("storage-heading")),
        move || usage.get().map(|usage| storage_page::used(usage, speech)),
        view! { <A href=route::STORAGE>{speech::said("your-account-storage-link")}</A> },
    ))
}

fn asking(
    session: CurrentSession,
    deleting: RwSignal<Deleting>,
    close: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let password = RwSignal::new(String::new());
    let refused = RwSignal::new(None::<ApiError>);
    let deletion = Act::new(move |(), _: Here| {
        refused.set(None);
        let typed = password.get_untracked();

        async move {
            match service::ask_deletion(&typed).await {
                Ok(()) => session.state.set(SignInState::SignedOut),
                Err(failed) => match to_hand_over(&failed) {
                    Some(projects) => deleting.set(Deleting::HandingOver(named(projects).await)),
                    None => refused.set(Some(failed)),
                },
            }
        }
    });
    let until = moment::human_day(OffsetDateTime::now_utc() + KEPT_BEFORE_DELETION);

    dialog::modal(
        Signal::derive(|| speech::say("your-account-delete-question")),
        "deleting-account",
        close,
        html::form()
            .on(ev::submit, move |event| {
                event.prevent_default();
                deletion.start(());
            })
            .child((
                html::p().child(move || {
                    let mut args = FluentArgs::new();
                    args.set("until", until.clone());
                    speech::say_with("your-account-delete-kept", &args)
                }),
                html::p()
                    .class("hint")
                    .child(speech::said("your-account-delete-alone")),
                html::label().class("field").child((
                    html::span().child(speech::said("your-account-delete-password")),
                    html::input()
                        .r#type("password")
                        .attr("autocomplete", "current-password")
                        .required(true)
                        .prop("value", move || password.get())
                        .on(ev::input, move |event| {
                            password.set(event_target_value(&event))
                        }),
                )),
                move || refused.get().map(problem::problem),
                html::div().class("dialog-actions").child((
                    html::button()
                        .r#type("button")
                        .on(ev::click, move |_| close())
                        .child(speech::said("your-account-keep")),
                    html::button()
                        .r#type("submit")
                        .class("danger")
                        .attr("aria-busy", move || deletion.busy())
                        .child(speech::said("your-account-delete-confirm")),
                )),
            )),
    )
}

fn handing_over(
    projects: Vec<ToHandOver>,
    close: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    dialog::modal(
        Signal::derive(|| speech::say("your-account-hand-over-heading")),
        "handing-over",
        close,
        (
            html::p().child(speech::said("your-account-hand-over-hint")),
            html::ul().class("to-hand-over").child(
                projects
                    .into_iter()
                    .map(|project| {
                        html::li().child((
                            html::span().child(project.name),
                            view! {
                                <A href=route::members(&project.id)>
                                    {speech::said("your-account-hand-over-members")}
                                </A>
                            },
                        ))
                    })
                    .collect_view(),
            ),
            html::div().class("dialog-actions").child(
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| close())
                    .child(speech::said("your-account-hand-over-close")),
            ),
        ),
    )
}

/// Returns the projects that must be handed over first, if that is why the deletion was refused.
fn to_hand_over(failed: &ApiError) -> Option<Vec<String>> {
    let ApiError::Rejected(refusal) = failed else {
        return None;
    };
    if refusal.kind.as_deref() != Some(refused::PROJECTS_TO_HAND_OVER) {
        return None;
    }

    refusal
        .particulars
        .iter()
        .find_map(|(name, particular)| match (name.as_str(), particular) {
            ("projects", Particular::Texts(projects)) => Some(projects.clone()),
            _ => None,
        })
}

/// Returns the projects with their names.
/// A project that is not in the project list is shown by its id instead.
async fn named(ids: Vec<String>) -> Vec<ToHandOver> {
    let listed = projects::list().await.unwrap_or_default();

    ids.into_iter()
        .map(|id| ToHandOver {
            name: listed
                .iter()
                .find(|project| project.id.to_string() == id)
                .map_or_else(|| id.clone(), |project| project.name.clone()),
            id,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::http::Refusal;

    use super::*;

    fn refused_as(kind: &str, particulars: Vec<(String, Particular)>) -> ApiError {
        ApiError::Rejected(Refusal {
            kind: Some(kind.to_owned()),
            detail: kind.to_owned(),
            particulars,
        })
    }

    #[test]
    fn the_projects_to_hand_over_are_read_from_the_refusal_naming_them() {
        // given: a refusal naming two projects to hand over
        let failed = refused_as(
            refused::PROJECTS_TO_HAND_OVER,
            vec![(
                "projects".to_owned(),
                Particular::Texts(vec!["project_1".to_owned(), "project_2".to_owned()]),
            )],
        );

        // expect: those projects are read from it
        assert_eq!(
            to_hand_over(&failed),
            Some(vec!["project_1".to_owned(), "project_2".to_owned()])
        );
    }

    #[test]
    fn any_other_refusal_names_nothing_to_hand_over() {
        // expect: a wrong password names no projects
        assert_eq!(
            to_hand_over(&refused_as(refused::PASSWORD_WRONG, Vec::new())),
            None
        );
    }
}
