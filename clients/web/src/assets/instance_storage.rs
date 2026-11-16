use std::future::Future;
use std::ops::Deref;
use std::time::Duration;

use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::{IntoView, ev, html};

use crate::act::{Act, Here};
use crate::assets::paged::Paged;
use crate::assets::service;
use crate::assets::service::{AccountStorage, ProjectStorage, Usage};
use crate::assets::storage_page;
use crate::avatar;
use crate::blocks;
use crate::dialog;
use crate::http::ApiError;
use crate::problem;
use crate::size;
use crate::speech;
use crate::speech::Speech;

const MEGABYTE: u64 = 1024 * 1024;

/// How long typing in a search field must pause before the list is asked for.
const SEARCH_AFTER: Duration = Duration::from_millis(300);
const GIGABYTE: u64 = 1024 * MEGABYTE;

/// The unit a limit is entered in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LimitUnit {
    Megabytes,
    Gigabytes,
}

impl LimitUnit {
    fn bytes(self) -> u64 {
        match self {
            Self::Megabytes => MEGABYTE,
            Self::Gigabytes => GIGABYTE,
        }
    }

    fn named(self) -> &'static str {
        match self {
            Self::Megabytes => "megabytes",
            Self::Gigabytes => "gigabytes",
        }
    }

    fn of(named: &str) -> Self {
        match named {
            "gigabytes" => Self::Gigabytes,
            _ => Self::Megabytes,
        }
    }
}

/// Reads a limit as entered: a number greater than 0, with a point or a comma before its decimals.
/// Returns the limit in bytes, or nothing if the entry is not such a number.
fn limit_entered(amount: &str, unit: LimitUnit) -> Option<u64> {
    let amount: f64 = amount.trim().replace(',', ".").parse().ok()?;
    let bytes = (amount * unit.bytes() as f64).round();

    (amount.is_finite() && bytes >= 1.0 && bytes < u64::MAX as f64).then_some(bytes as u64)
}

/// Shows a limit for editing: in gigabytes from one gigabyte on, else in megabytes.
/// At most two decimals are shown, and trailing zeros are left out.
fn limit_shown(most: u64) -> (String, LimitUnit) {
    let unit = if most >= GIGABYTE {
        LimitUnit::Gigabytes
    } else {
        LimitUnit::Megabytes
    };
    let shown = format!("{:.2}", most as f64 / unit.bytes() as f64);
    let shown = shown.trim_end_matches('0').trim_end_matches('.').to_owned();

    (shown, unit)
}

/// A list the operator can search and page through.
/// It reads as the list itself, and adds the term searched for.
struct Searched<T: Send + Sync + 'static> {
    search: RwSignal<String>,
    list: Paged<T>,
}

impl<T: Send + Sync + 'static> Clone for Searched<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Send + Sync + 'static> Copy for Searched<T> {}

impl<T: Send + Sync + 'static> Deref for Searched<T> {
    type Target = Paged<T>;

    fn deref(&self) -> &Paged<T> {
        &self.list
    }
}

impl<T: Clone + Send + Sync + 'static> Searched<T> {
    fn new() -> Self {
        Self {
            search: RwSignal::new(String::new()),
            list: Paged::new(),
        }
    }

    /// Loads a page with `fetch`, which is given the search term and the cursor.
    /// Without a cursor the page replaces the list, with one it is added to it.
    fn load<Fu>(
        self,
        after: Option<String>,
        refused: RwSignal<Option<ApiError>>,
        fetch: impl FnOnce(String, Option<String>) -> Fu + 'static,
    ) where
        Fu: Future<Output = Result<(Vec<T>, Option<String>), ApiError>> + 'static,
    {
        let search = self.search.get_untracked();
        self.list.load(
            after,
            move |failed| refused.set(Some(failed)),
            move |after| fetch(search, after),
        );
    }

    fn searching(self) -> bool {
        !self.search.with(|term| term.trim().is_empty())
    }
}

/// What the storage overview shows and is doing.
#[derive(Clone, Copy)]
struct InstanceStorageState {
    instance: RwSignal<Option<Usage>>,
    accounts: Searched<AccountStorage>,
    projects: Searched<ProjectStorage>,
    refused: RwSignal<Option<ApiError>>,
    limiting: RwSignal<Option<AccountStorage>>,
}

impl InstanceStorageState {
    fn open() -> Self {
        let overview = Self {
            instance: RwSignal::new(None),
            accounts: Searched::new(),
            projects: Searched::new(),
            refused: RwSignal::new(None),
            limiting: RwSignal::new(None),
        };
        overview.load_accounts(None);
        overview.load_projects(None);
        spawn_local(async move {
            match service::instance().await {
                Ok(usage) => overview.instance.set(Some(usage)),
                Err(failed) => overview.refused.set(Some(failed)),
            }
        });

        overview
    }

    fn load_accounts(self, after: Option<String>) {
        self.accounts
            .load(after, self.refused, |search, after| async move {
                service::accounts(&search, after.as_deref())
                    .await
                    .map(|page| (page.accounts, page.next))
            });
    }

    fn load_projects(self, after: Option<String>) {
        self.projects
            .load(after, self.refused, |search, after| async move {
                service::projects(&search, after.as_deref())
                    .await
                    .map(|page| (page.projects, page.next))
            });
    }
}

/// The search field in the head of a list.
/// Its label is only read out; the hint inside the field shows what to type.
/// The list is asked for only once typing has paused for `SEARCH_AFTER`, not at every key.
fn finding<T: Clone + Send + Sync + 'static>(
    paged: Searched<T>,
    label: &'static str,
    hint: &'static str,
    load: impl Fn() + Copy + 'static,
) -> impl IntoView {
    let waiting = StoredValue::new(None::<TimeoutHandle>);
    on_cleanup(move || {
        if let Some(handle) = waiting.get_value() {
            handle.clear();
        }
    });

    html::label().class("finding").child((
        html::span()
            .class("visually-hidden")
            .child(speech::said(label)),
        html::input()
            .r#type("search")
            .attr("placeholder", move || speech::say(hint))
            .prop("value", move || paged.search.get())
            .on(ev::input, move |event| {
                paged.search.set(event_target_value(&event));
                if let Some(handle) = waiting.get_value() {
                    handle.clear();
                }
                waiting.set_value(
                    set_timeout(
                        move || {
                            waiting.set_value(None);
                            load();
                        },
                        SEARCH_AFTER,
                    )
                    .ok(),
                );
            }),
    ))
}

/// What is said when a list is empty: that nothing matches the search, or that there is nothing at all.
fn empty<T: Clone + Send + Sync + 'static>(
    paged: Searched<T>,
    no_match: &'static str,
    none: &'static str,
) -> impl IntoView {
    move || {
        paged.nothing().then(|| {
            html::p()
                .class("hint")
                .child(speech::said(if paged.searching() {
                    no_match
                } else {
                    none
                }))
        })
    }
}

/// The button that loads the next page of a list, if there is one.
fn more<T: Clone + Send + Sync + 'static>(
    paged: Searched<T>,
    load: impl Fn(String) + Clone + Send + Sync + 'static,
) -> impl IntoView {
    move || {
        let load = load.clone();
        paged.next.get().map(|next| {
            blocks::more(
                "storage-more",
                move || paged.loading.get(),
                move |_| load(next.clone()),
            )
        })
    }
}

/// The operator's overview of the storage: what the instance uses, and each account and project that uses any.
pub fn storage_overview() -> impl IntoView {
    let overview = InstanceStorageState::open();
    let speech = expect_context::<Speech>();

    (
        move || {
            overview.instance.get().map(|usage| {
                blocks::usage_card(
                    storage_page::used(usage, speech),
                    speech::said("instance-storage-hint"),
                )
            })
        },
        move || {
            overview
                .limiting
                .with(Option::is_none)
                .then(|| overview.refused.get().map(problem::problem))
        },
        blocks::cards((
            blocks::list_card(
                "storage-accounts",
                speech::said("instance-storage-accounts"),
                finding(
                    overview.accounts,
                    "instance-storage-find",
                    "instance-storage-find-hint",
                    move || overview.load_accounts(None),
                ),
                (
                    empty(
                        overview.accounts,
                        "instance-storage-no-match",
                        "instance-storage-none",
                    ),
                    blocks::rows(move || {
                        overview
                            .accounts
                            .listed
                            .get()
                            .into_iter()
                            .map(|account| listed(overview, account, speech))
                            .collect_view()
                    }),
                ),
                more(overview.accounts, move |next| {
                    overview.load_accounts(Some(next))
                }),
            ),
            blocks::list_card(
                "storage-projects",
                speech::said("instance-storage-projects"),
                finding(
                    overview.projects,
                    "instance-storage-find-project",
                    "instance-storage-find-project-hint",
                    move || overview.load_projects(None),
                ),
                (
                    empty(
                        overview.projects,
                        "instance-storage-no-project-match",
                        "instance-storage-no-projects",
                    ),
                    blocks::rows(move || {
                        overview
                            .projects
                            .listed
                            .get()
                            .into_iter()
                            .map(|project| project_listed(project, speech))
                            .collect_view()
                    }),
                ),
                more(overview.projects, move |next| {
                    overview.load_projects(Some(next))
                }),
            ),
        )),
        move || {
            overview
                .limiting
                .get()
                .map(|account| limit_dialog(overview, account))
        },
    )
}

/// How much a row uses, as a small bar with the words beside it.
/// Without a limit there is no bar, only the words.
fn gauge(usage: Usage, speech: Speech) -> impl IntoView {
    html::span().class("gauge").child((
        usage.most.map(|most| {
            html::meter()
                .attr("min", 0)
                .attr("max", most as f64)
                .attr("value", usage.used as f64)
                .attr("aria-hidden", "true")
        }),
        html::span().class("used").child(usage_told(usage, speech)),
    ))
}

/// How much is used of a limit, for example "2 MB of 1 GB used", or only "2 MB used" without a limit.
fn usage_told(usage: Usage, speech: Speech) -> impl Fn() -> String + Send + Sync + 'static {
    move || {
        let mut args = FluentArgs::new();
        args.set("used", size::told(usage.used, speech.language()));
        match usage.most {
            Some(most) => {
                args.set("most", size::told(most, speech.language()));
                speech.say_with("storage-used", &args)
            }
            None => speech.say_with("storage-used-unbounded", &args),
        }
    }
}

fn project_listed(project: ProjectStorage, speech: Speech) -> impl IntoView {
    let owners = project
        .owners
        .iter()
        .map(|owner| match (&owner.name, &owner.email) {
            (Some(name), Some(email)) => format!("{name} ({email})"),
            (Some(name), None) => name.clone(),
            _ => owner.account.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ");
    let has_owners = !project.owners.is_empty();
    let name = project
        .name
        .clone()
        .unwrap_or_else(|| project.project.clone());

    blocks::row(
        avatar::avatar(&project.project, Some(&name), None),
        name.clone(),
        (
            has_owners.then(|| {
                html::span().class("owners").child(move || {
                    let mut args = FluentArgs::new();
                    args.set("names", owners.clone());
                    speech.say_with("instance-storage-owners", &args)
                })
            }),
            gauge(
                Usage {
                    used: project.used,
                    most: project.most,
                },
                speech,
            ),
        ),
        (),
        (),
    )
}

fn named(account: &AccountStorage) -> String {
    account
        .name
        .clone()
        .unwrap_or_else(|| account.account.clone())
}

fn listed(
    overview: InstanceStorageState,
    account: AccountStorage,
    speech: Speech,
) -> impl IntoView {
    let usage = Usage {
        used: account.used,
        most: account.most,
    };
    let limitable = account.most.is_some();
    let asked = account.clone();
    let name = named(&account);

    blocks::row(
        avatar::avatar(&account.account, account.name.as_deref(), None),
        name,
        (
            account
                .email
                .clone()
                .map(|email| html::span().class("email").child(email)),
            gauge(usage, speech),
        ),
        account
            .own_limit
            .then(|| blocks::mark(speech::said("instance-storage-own-limit"))),
        limitable.then(|| {
            html::button()
                .r#type("button")
                .on(ev::click, move |_| {
                    overview.refused.set(None);
                    overview.limiting.set(Some(asked.clone()));
                })
                .child(speech::said("instance-storage-change-limit"))
        }),
    )
}

/// What the limit dialog asks of the server.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Limiting {
    Give(u64),
    TakeBack,
}

fn limit_dialog(overview: InstanceStorageState, account: AccountStorage) -> impl IntoView {
    let (shown, unit) = limit_shown(account.most.unwrap_or(GIGABYTE));
    let amount = RwSignal::new(shown.replace('.', &speech::say("decimal-point")));
    let unit = RwSignal::new(unit);
    let unreadable = RwSignal::new(false);
    let close = move || {
        overview.refused.set(None);
        overview.limiting.set(None);
    };
    let id = account.account.clone();
    let limiting = Act::new(move |asked: Limiting, _: Here| {
        let id = id.clone();
        async move {
            let done = match asked {
                Limiting::Give(most) => service::give_limit(&id, most).await,
                Limiting::TakeBack => service::take_back_limit(&id).await,
            };
            match done {
                Ok(()) => {
                    overview.limiting.set(None);
                    overview.load_accounts(None);
                }
                Err(failed) => overview.refused.set(Some(failed)),
            }
        }
    });
    let name = named(&account);
    let heading = Signal::derive(move || {
        let mut args = FluentArgs::new();
        args.set("name", name.clone());
        speech::say_with("instance-storage-limit-heading", &args)
    });

    dialog::modal(
        heading,
        "limiting",
        close,
        html::form()
            .on(ev::submit, move |event| {
                event.prevent_default();
                match limit_entered(&amount.get_untracked(), unit.get_untracked()) {
                    Some(most) => {
                        unreadable.set(false);
                        limiting.start(Limiting::Give(most));
                    }
                    None => unreadable.set(true),
                }
            })
            .child((
                html::p()
                    .class("hint")
                    .child(speech::said("instance-storage-limit-hint")),
                html::div().class("limit").child((
                    html::label().class("field").child((
                        html::span().child(speech::said("instance-storage-limit-amount")),
                        html::input()
                            .r#type("text")
                            .attr("inputmode", "decimal")
                            .required(true)
                            .prop("value", move || amount.get())
                            .on(ev::input, move |event| {
                                amount.set(event_target_value(&event))
                            }),
                    )),
                    html::label().class("field").child((
                        html::span().child(speech::said("instance-storage-limit-unit")),
                        html::select()
                            .on(ev::change, move |event| {
                                unit.set(LimitUnit::of(&event_target_value(&event)))
                            })
                            .child(
                                [LimitUnit::Megabytes, LimitUnit::Gigabytes]
                                    .into_iter()
                                    .map(|each| {
                                        html::option()
                                            .value(each.named())
                                            .selected(move || unit.get() == each)
                                            .child(move || {
                                                speech::say(match each {
                                                    LimitUnit::Megabytes => "size-unit-megabytes",
                                                    LimitUnit::Gigabytes => "size-unit-gigabytes",
                                                })
                                            })
                                    })
                                    .collect_view(),
                            ),
                    )),
                )),
                move || {
                    unreadable.get().then(|| {
                        html::p()
                            .class("problem")
                            .role("alert")
                            .child(speech::said("instance-storage-limit-invalid"))
                    })
                },
                move || overview.refused.get().map(problem::problem),
                html::div().class("dialog-actions").child((
                    html::button()
                        .r#type("button")
                        .on(ev::click, move |_| close())
                        .child(speech::said("instance-storage-limit-cancel")),
                    account.own_limit.then(|| {
                        html::button()
                            .r#type("button")
                            .attr("aria-busy", move || limiting.busy_on(&Limiting::TakeBack))
                            .on(ev::click, move |_| limiting.start(Limiting::TakeBack))
                            .child(speech::said("instance-storage-limit-default"))
                    }),
                    html::button()
                        .r#type("submit")
                        .attr("aria-busy", move || limiting.busy())
                        .child(speech::said("instance-storage-limit-save")),
                )),
            )),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_limit_is_read_with_a_point_or_a_comma() {
        // expect: whole and decimal amounts in either unit
        assert_eq!(
            limit_entered("500", LimitUnit::Megabytes),
            Some(500 * MEGABYTE)
        );
        assert_eq!(
            limit_entered(" 1.5 ", LimitUnit::Gigabytes),
            Some(GIGABYTE + GIGABYTE / 2)
        );
        assert_eq!(
            limit_entered("1,5", LimitUnit::Gigabytes),
            Some(GIGABYTE + GIGABYTE / 2)
        );
    }

    #[test]
    fn a_limit_that_is_no_number_above_nothing_is_refused() {
        // expect: nothing, zero, a negative, words and an endless amount are refused
        for entered in ["", "0", "-1", "lots", "inf", "1e400"] {
            assert_eq!(
                limit_entered(entered, LimitUnit::Gigabytes),
                None,
                "{entered}"
            );
        }
    }

    #[test]
    fn a_limit_is_shown_in_gigabytes_from_one_gigabyte_on() {
        // expect: below a gigabyte in megabytes, from one on in gigabytes
        assert_eq!(
            limit_shown(500 * MEGABYTE),
            ("500".to_owned(), LimitUnit::Megabytes)
        );
        assert_eq!(
            limit_shown(GIGABYTE),
            ("1".to_owned(), LimitUnit::Gigabytes)
        );
        assert_eq!(
            limit_shown(GIGABYTE + GIGABYTE / 2),
            ("1.5".to_owned(), LimitUnit::Gigabytes)
        );
    }

    #[test]
    fn a_unit_reads_back_from_its_name() {
        // expect: each unit survives its name
        for unit in [LimitUnit::Megabytes, LimitUnit::Gigabytes] {
            assert_eq!(LimitUnit::of(unit.named()), unit);
        }
    }
}
