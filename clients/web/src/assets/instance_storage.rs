use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::{IntoView, ev, html};

use crate::act::{Act, Here};
use crate::assets::service;
use crate::assets::service::{AccountStorage, Usage};
use crate::assets::storage_page;
use crate::dialog;
use crate::http::ApiError;
use crate::problem;
use crate::size;
use crate::speech;
use crate::speech::Speech;

const MEGABYTE: u64 = 1024 * 1024;
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

/// What the storage overview shows and is doing.
/// `asked` counts the lists asked for, so an answer to an older one is ignored.
#[derive(Clone, Copy)]
struct Overview {
    instance: RwSignal<Option<Usage>>,
    search: RwSignal<String>,
    accounts: RwSignal<Vec<AccountStorage>>,
    next: RwSignal<Option<String>>,
    loading: RwSignal<bool>,
    refused: RwSignal<Option<ApiError>>,
    limiting: RwSignal<Option<AccountStorage>>,
    asked: StoredValue<u64>,
}

impl Overview {
    fn asked() -> Self {
        let overview = Self {
            instance: RwSignal::new(None),
            search: RwSignal::new(String::new()),
            accounts: RwSignal::new(Vec::new()),
            next: RwSignal::new(None),
            loading: RwSignal::new(false),
            refused: RwSignal::new(None),
            limiting: RwSignal::new(None),
            asked: StoredValue::new(0),
        };
        overview.load(None);
        spawn_local(async move {
            match service::instance().await {
                Ok(usage) => overview.instance.set(Some(usage)),
                Err(failed) => overview.refused.set(Some(failed)),
            }
        });

        overview
    }

    /// Loads a page of accounts.
    /// Without a cursor it replaces the list, with one it adds the next page to it.
    fn load(self, after: Option<String>) {
        self.asked.update_value(|asked| *asked += 1);
        let asking = self.asked.get_value();
        let search = self.search.get_untracked();
        self.loading.set(true);

        spawn_local(async move {
            let told = service::accounts(&search, after.as_deref()).await;
            if self.asked.get_value() != asking {
                return;
            }
            match told {
                Ok(page) => {
                    match after {
                        Some(_) => self.accounts.update(|shown| shown.extend(page.accounts)),
                        None => self.accounts.set(page.accounts),
                    }
                    self.next.set(page.next);
                }
                Err(failed) => self.refused.set(Some(failed)),
            }
            self.loading.set(false);
        });
    }
}

/// The operator's overview of the storage: what the instance uses, and each account that uses any.
pub fn storage_overview() -> impl IntoView {
    let overview = Overview::asked();
    let speech = expect_context::<Speech>();

    (
        move || {
            overview
                .instance
                .get()
                .map(|usage| storage_page::used(usage, speech))
        },
        html::p()
            .class("hint")
            .child(speech::said("instance-storage-hint")),
        move || {
            overview
                .limiting
                .with(Option::is_none)
                .then(|| overview.refused.get().map(problem::problem))
        },
        html::h2().child(speech::said("instance-storage-accounts")),
        html::label().class("field finding").child((
            html::span().child(speech::said("instance-storage-find")),
            html::input()
                .r#type("search")
                .attr("placeholder", move || {
                    speech.say("instance-storage-find-hint")
                })
                .prop("value", move || overview.search.get())
                .on(ev::input, move |event| {
                    overview.search.set(event_target_value(&event));
                    overview.load(None);
                }),
        )),
        move || {
            let nothing = overview.accounts.with(Vec::is_empty)
                && overview.next.with(Option::is_none)
                && !overview.loading.get();
            nothing.then(|| {
                let searching = !overview.search.with(|term| term.trim().is_empty());
                html::p().class("hint").child(if searching {
                    speech::said("instance-storage-no-match")
                } else {
                    speech::said("instance-storage-none")
                })
            })
        },
        html::ul().class("accounts storage-use").child(move || {
            overview
                .accounts
                .get()
                .into_iter()
                .map(|account| listed(overview, account, speech))
                .collect_view()
        }),
        move || {
            overview.next.get().map(|next| {
                html::button()
                    .r#type("button")
                    .class("more")
                    .disabled(move || overview.loading.get())
                    .on(ev::click, move |_| overview.load(Some(next.clone())))
                    .child(speech::said("storage-more"))
            })
        },
        move || {
            overview
                .limiting
                .get()
                .map(|account| limit_dialog(overview, account))
        },
    )
}

fn named(account: &AccountStorage) -> String {
    account
        .name
        .clone()
        .unwrap_or_else(|| account.account.clone())
}

fn listed(overview: Overview, account: AccountStorage, speech: Speech) -> impl IntoView {
    let usage = Usage {
        used: account.used,
        most: account.most,
    };
    let told = move || {
        let mut args = FluentArgs::new();
        args.set("used", size::told(usage.used, speech.language()));
        match usage.most {
            Some(most) => {
                args.set("most", size::told(most, speech.language()));
                speech.say_with("storage-used", &args)
            }
            None => speech.say_with("storage-used-unbounded", &args),
        }
    };
    let limitable = account.most.is_some();
    let asked = account.clone();

    html::li().child((
        html::div().class("who").child((
            html::p().class("name").child(named(&account)),
            account
                .email
                .clone()
                .map(|email| html::p().class("email").child(email)),
            html::p().class("used").child(told),
        )),
        html::div().class("marks").child(account.own_limit.then(|| {
            html::span()
                .class("mark")
                .child(speech::said("instance-storage-own-limit"))
        })),
        html::div().class("actions").child(limitable.then(|| {
            html::button()
                .r#type("button")
                .on(ev::click, move |_| {
                    overview.refused.set(None);
                    overview.limiting.set(Some(asked.clone()));
                })
                .child(speech::said("instance-storage-change-limit"))
        })),
    ))
}

/// What the limit dialog asks of the server.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Limiting {
    Give(u64),
    TakeBack,
}

fn limit_dialog(overview: Overview, account: AccountStorage) -> impl IntoView {
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
                    overview.load(None);
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
