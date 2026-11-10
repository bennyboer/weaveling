use leptos::prelude::*;
use leptos::{IntoView, html, view};
use leptos_router::components::A;
use leptos_router::hooks;

use crate::accounts::{CurrentSession, Screen};
use crate::icons;
use crate::icons::Icon;
use crate::projects::open_project::OpenProject;
use crate::route;
use crate::speech;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Label {
    Home,
    Said(&'static str),
    Project(String),
    Idea(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Crumb {
    label: Label,
    leads_to: Option<String>,
}

/// The path from one's projects to the page shown, read from its address, every step but the last a way back.
pub fn path_bar() -> impl IntoView {
    let session = expect_context::<CurrentSession>();
    let screen = session.screen();
    let location = hooks::use_location();

    move || {
        (screen.get() == Screen::Work).then(|| {
            let crumbs = crumbs_of(&location.pathname.get());
            let last = crumbs.len().saturating_sub(1);

            html::nav()
                .class("path")
                .attr("aria-label", speech::said("path"))
                .child(
                    html::ol().child(
                        crumbs
                            .into_iter()
                            .enumerate()
                            .map(|(at, crumb)| step(crumb, at == last))
                            .collect_view(),
                    ),
                )
        })
    }
}

fn step(crumb: Crumb, here: bool) -> impl IntoView {
    let home = crumb.label == Label::Home;
    let named = move || {
        if home {
            (
                icons::mark(Icon::Home),
                html::span().class("unseen").child(named(&crumb.label)),
            )
                .into_any()
        } else {
            named(&crumb.label).into_any()
        }
    };

    let shown = match (crumb.leads_to, here) {
        (_, true) => html::span()
            .attr("aria-current", "page")
            .child(named)
            .into_any(),
        (Some(href), false) => view! { <A href=href exact=true>{named}</A> }.into_any(),
        (None, false) => html::span().class("passing").child(named).into_any(),
    };

    html::li().class(("home", home)).child((
        shown,
        (!here).then(|| {
            html::span()
                .class("between")
                .attr("aria-hidden", "true")
                .child(icons::mark(Icon::Closed))
        }),
    ))
}

fn named(label: &Label) -> String {
    match label {
        Label::Home => speech::say("path-home"),
        Label::Said(message) => speech::say(message),
        Label::Project(segment) => expect_context::<OpenProject>()
            .name()
            .unwrap_or_else(|| route::named(segment)),
        Label::Idea(segment) => route::named(segment),
    }
}

fn crumbs_of(pathname: &str) -> Vec<Crumb> {
    let segments: Vec<&str> = pathname.split('/').filter(|s| !s.is_empty()).collect();
    let projects = Crumb {
        label: Label::Home,
        leads_to: Some(route::WORKSPACE.to_owned()),
    };
    let said = |message: &'static str, leads_to: Option<String>| Crumb {
        label: Label::Said(message),
        leads_to,
    };

    let further = match segments.as_slice() {
        [] => vec![],
        ["trash"] => vec![said("trash-heading", None)],
        ["invitation"] => vec![said("path-invitation", None)],
        ["setup"] => vec![said("set-up-heading", None)],
        ["account"] => vec![said("path-account", None)],
        ["account", "security"] => vec![
            said("path-account", Some(route::ACCOUNT.to_owned())),
            said("security-heading", None),
        ],
        ["instance", rest @ ..] => {
            let manage = said("account-menu-operator", Some("/instance".to_owned()));
            match rest {
                [] | ["accounts"] => vec![manage, said("instance-accounts-heading", None)],
                ["invitations"] => vec![manage, said("instance-invitations-heading", None)],
                _ => vec![said("path-missing", None)],
            }
        }
        ["projects", project, rest @ ..] => {
            let named = Crumb {
                label: Label::Project((*project).to_owned()),
                leads_to: Some(route::writing(project)),
            };
            let view = |message, at: String| said(message, Some(at));
            match rest {
                [] => vec![named],
                ["write"] => vec![named, said("view-write", None)],
                ["write", "loose"] => vec![
                    named,
                    view("view-write", route::writing(project)),
                    said("writing-side-loose", None),
                ],
                ["board"] => vec![named, said("view-board", None)],
                ["outline"] => vec![named, said("view-outline", None)],
                ["ideas"] => vec![named, said("view-ideas", None)],
                ["ideas", idea] => vec![
                    named,
                    view("view-ideas", route::pool(project)),
                    Crumb {
                        label: Label::Idea((*idea).to_owned()),
                        leads_to: None,
                    },
                ],
                ["members"] => vec![named, said("members-heading", None)],
                ["members", "invited"] => vec![
                    named,
                    view("members-heading", route::members(project)),
                    said("members-invited", None),
                ],
                _ => vec![said("path-missing", None)],
            }
        }
        _ => vec![said("path-missing", None)],
    };

    std::iter::once(projects).chain(further).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(pathname: &str) -> Vec<Label> {
        crumbs_of(pathname)
            .into_iter()
            .map(|crumb| crumb.label)
            .collect()
    }

    #[test]
    fn every_path_begins_at_home() {
        // expect: home alone at the root
        assert_eq!(labels("/"), vec![Label::Home]);
        // and: first everywhere else
        for pathname in [
            "/trash",
            "/instance/accounts",
            "/projects/loom-project_1/board",
            "/nowhere",
        ] {
            assert_eq!(labels(pathname)[0], Label::Home, "{pathname}");
        }
    }

    #[test]
    fn a_projects_views_name_the_project_and_lead_back_to_its_writing() {
        // when: the board of a project is shown
        let crumbs = crumbs_of("/projects/the-loom-project_1/board");

        // then: the project leads to its writing, and the board is where one is
        assert_eq!(
            crumbs[1],
            Crumb {
                label: Label::Project("the-loom-project_1".to_owned()),
                leads_to: Some(route::writing("the-loom-project_1")),
            }
        );
        assert_eq!(
            crumbs[2],
            Crumb {
                label: Label::Said("view-board"),
                leads_to: None,
            }
        );
    }

    #[test]
    fn an_idea_lies_under_the_projects_ideas() {
        // expect: the ideas lead to the pool, and the idea is where one is
        assert_eq!(
            crumbs_of("/projects/loom-project_1/ideas/the-fairy-idea_2"),
            vec![
                Crumb {
                    label: Label::Home,
                    leads_to: Some("/".to_owned()),
                },
                Crumb {
                    label: Label::Project("loom-project_1".to_owned()),
                    leads_to: Some(route::writing("loom-project_1")),
                },
                Crumb {
                    label: Label::Said("view-ideas"),
                    leads_to: Some(route::pool("loom-project_1")),
                },
                Crumb {
                    label: Label::Idea("the-fairy-idea_2".to_owned()),
                    leads_to: None,
                },
            ]
        );
    }

    #[test]
    fn the_instance_pages_lie_under_managing_the_instance() {
        // expect: accounts and invitations under managing the instance
        assert_eq!(
            labels("/instance/invitations"),
            vec![
                Label::Home,
                Label::Said("account-menu-operator"),
                Label::Said("instance-invitations-heading"),
            ]
        );
    }

    #[test]
    fn one_s_account_is_found_and_security_lies_under_it() {
        // expect: one's account on its own
        assert_eq!(
            labels("/account"),
            vec![Label::Home, Label::Said("path-account")]
        );
        // and: security under it
        assert_eq!(
            labels("/account/security"),
            vec![
                Label::Home,
                Label::Said("path-account"),
                Label::Said("security-heading"),
            ]
        );
    }

    #[test]
    fn an_address_leading_nowhere_says_so() {
        // expect: nothing found, under one's projects
        assert_eq!(
            labels("/projects/loom-project_1/elsewhere"),
            vec![Label::Home, Label::Said("path-missing")]
        );
    }
}
