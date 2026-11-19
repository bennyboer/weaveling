use std::collections::HashMap;

use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::{IntoView, ev, html};
use leptos_router::NavigateOptions;
use leptos_router::hooks;
use readings_contract::SharedDTO;
use time::macros::format_description;
use time::{Date, Duration, OffsetDateTime, UtcOffset};

use crate::act::{Act, Here};
use crate::dialog;
use crate::http::ApiError;
use crate::moment;
use crate::problem;
use crate::readings::model::OutlinedSection;
use crate::readings::service;
use crate::readings::sharing;
use crate::readings::sharing::{KeptFor, OpenFor};
use crate::route;
use crate::scenes::model::SceneId;
use crate::scenes::service as scenes;
use crate::speech;

const OPEN_BY_DEFAULT: i64 = 30;
const KEPT_BY_DEFAULT: i64 = 90;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum What {
    Book,
    Sections,
    Scenes,
}

pub(crate) fn said_with(key: &'static str, args: &[(&'static str, String)]) -> String {
    let mut told = FluentArgs::new();
    for (name, value) in args {
        told.set(*name, value.clone());
    }

    speech::say_with(key, &told)
}

fn day_written(day: Date) -> String {
    day.format(format_description!("[year]-[month]-[day]"))
        .expect("a day is always written as a date")
}

fn day_read(written: &str) -> Option<Date> {
    Date::parse(written, format_description!("[year]-[month]-[day]")).ok()
}

/// The dialog an author makes a reading copy in: what of the work to share, its name, and how long it is open and kept.
/// Once made, the copy is opened.
pub fn share_dialog(
    slug: String,
    close: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let project = route::project_id(&slug);
    let now = OffsetDateTime::now_utc();
    let here = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    let today = now.to_offset(here).date();
    let sections = LocalResource::new({
        let project = project.clone();

        move || {
            let project = project.clone();

            async move {
                service::live_book(&project)
                    .await
                    .map(|book| book.sections)
                    .unwrap_or_default()
            }
        }
    });
    let names = LocalResource::new({
        let project = project.clone();

        move || {
            let project = project.clone();

            async move {
                scenes::in_project(&project)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|summary| (summary.id.clone(), summary.shown_as()))
                    .collect::<HashMap<SceneId, String>>()
            }
        }
    });
    let what = RwSignal::new(What::Book);
    let run = RwSignal::new(None::<(usize, usize)>);
    let scene_run = RwSignal::new(None::<(usize, usize)>);
    let name = RwSignal::new(said_with(
        "share-name-default",
        &[("day", moment::human_day(now))],
    ));
    let open = RwSignal::new(OpenFor::Days(OPEN_BY_DEFAULT));
    let kept = RwSignal::new(KeptFor::Days(KEPT_BY_DEFAULT));
    let times = Memo::new(move |_| sharing::times(now, here, open.get(), kept.get()));
    let failed = RwSignal::new(None::<ApiError>);
    let navigate = hooks::use_navigate();
    let making = Act::new(move |(), here_still: Here| {
        let project = project.clone();
        let slug = slug.clone();
        let navigate = navigate.clone();
        let shared = shared(
            what.get_untracked(),
            run.get_untracked(),
            scene_run.get_untracked(),
            sections,
        );
        let named = name.get_untracked();
        let (open_until, deleted_on) = times.get_untracked();

        async move {
            let Some(shared) = shared else {
                return;
            };
            match service::make_copy(&project, named.trim(), shared, open_until, deleted_on).await {
                Ok(made) => {
                    if here_still.still() {
                        close();
                        navigate(
                            &route::reading_copy(&slug, &made.id.to_string()),
                            NavigateOptions::default(),
                        );
                    }
                }
                Err(failure) => _ = failed.try_set(Some(failure)),
            }
        }
    });
    let unusable = move || {
        name.with(|named| named.trim().is_empty())
            || match what.get() {
                What::Book => false,
                What::Sections => run.get().is_none(),
                What::Scenes => scene_run.get().is_none(),
            }
    };

    dialog::modal(
        Signal::derive(speech::said("share-copy")),
        "share-copy",
        close,
        (
            html::p()
                .class("intro")
                .child(speech::said("share-copy-intro")),
            html::div().class("halves").child((
                html::div()
                    .class("half")
                    .child((what_to_share(what), move || match what.get() {
                        What::Book => None,
                        What::Sections => Some(sections_to_share(sections, run).into_any()),
                        What::Scenes => {
                            Some(scenes_to_share(sections, names, scene_run).into_any())
                        }
                    })),
                html::div().class("half").child((
                    html::label().class("field").child((
                        html::span()
                            .class("label")
                            .child(speech::said("share-name")),
                        html::input()
                            .r#type("text")
                            .prop("value", move || name.get())
                            .on(ev::input, move |event| name.set(event_target_value(&event))),
                    )),
                    how_long_open(open, today, times),
                    how_long_kept(kept, today, times),
                )),
            )),
            move || failed.get().map(problem::problem),
            html::div().class("dialog-actions").child((
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| close())
                    .child(speech::said("share-cancel")),
                html::button()
                    .r#type("button")
                    .class("primary")
                    .disabled(unusable)
                    .attr("aria-busy", move || making.busy())
                    .on(ev::click, move |_| making.start(()))
                    .child(speech::said("share-make")),
            )),
        ),
    )
}

/// What to send for the choice: the whole work, or the run of sections or of scenes from its first to its last.
fn shared(
    what: What,
    run: Option<(usize, usize)>,
    scene_run: Option<(usize, usize)>,
    sections: LocalResource<Vec<OutlinedSection>>,
) -> Option<SharedDTO> {
    match what {
        What::Book => Some(SharedDTO::Book),
        What::Sections => {
            let (first, last) = run?;
            sections.with_untracked(|held| {
                let held = held.as_ref()?;

                Some(SharedDTO::Sections {
                    first: held.get(first)?.section.to_string(),
                    last: held.get(last)?.section.to_string(),
                })
            })
        }
        What::Scenes => {
            let (first, last) = scene_run?;
            sections.with_untracked(|held| {
                let in_order: Vec<&SceneId> = held
                    .as_ref()?
                    .iter()
                    .flat_map(|outlined| &outlined.scenes)
                    .collect();

                Some(SharedDTO::Scenes {
                    first: in_order.get(first)?.to_string(),
                    last: in_order.get(last)?.to_string(),
                })
            })
        }
    }
}

pub(crate) fn chosen_among<T: Copy + PartialEq + Send + Sync + 'static>(
    named: &'static str,
    choice: RwSignal<T>,
    options: Vec<(&'static str, T)>,
    is: fn(T, T) -> bool,
) -> impl IntoView {
    html::div()
        .class("choices")
        .role("group")
        .attr("aria-label", speech::said(named))
        .child(
            options
                .into_iter()
                .map(|(label, option)| {
                    html::button()
                        .r#type("button")
                        .attr("aria-pressed", move || is(choice.get(), option).to_string())
                        .on(ev::click, move |_| choice.set(option))
                        .child(speech::said(label))
                })
                .collect::<Vec<_>>(),
        )
}

fn what_to_share(what: RwSignal<What>) -> impl IntoView {
    let options = [
        ("book", "share-whole", What::Book),
        ("sections", "share-sections", What::Sections),
        ("scenes", "share-scenes", What::Scenes),
    ];

    html::label().class("field").child((
        html::span()
            .class("label")
            .child(speech::said("share-what")),
        html::select()
            .on(ev::change, move |event| {
                let chosen = event_target_value(&event);
                if let Some((_, _, option)) = options.iter().find(|(named, _, _)| *named == chosen)
                {
                    what.set(*option);
                }
            })
            .child(
                options
                    .iter()
                    .map(|(named, label, option)| {
                        let option = *option;

                        html::option()
                            .value(*named)
                            .prop("selected", move || what.get() == option)
                            .child(speech::said(label))
                    })
                    .collect::<Vec<_>>(),
            ),
    ))
}

fn sections_to_share(
    sections: LocalResource<Vec<OutlinedSection>>,
    run: RwSignal<Option<(usize, usize)>>,
) -> impl IntoView {
    let in_run = move |at: usize| {
        run.get()
            .is_some_and(|(first, last)| first <= at && at <= last)
    };

    (
        html::ul()
            .class("tree")
            .attr("aria-label", speech::said("share-sections"))
            .child(move || {
                sections.get().map(|held| {
                    held.iter()
                        .enumerate()
                        .map(|(at, outlined)| {
                            html::li()
                                .class(move || if in_run(at) { "in" } else { "" })
                                .style(("--depth", outlined.depth.to_string()))
                                .child(
                                    html::label().child((
                                        html::input()
                                            .r#type("checkbox")
                                            .prop("checked", move || in_run(at))
                                            .on(ev::change, move |_| {
                                                run.update(|chosen| {
                                                    *chosen = sharing::ticked(*chosen, at);
                                                });
                                            }),
                                        html::span().child(outlined.title.clone()),
                                    )),
                                )
                        })
                        .collect::<Vec<_>>()
                })
            }),
        html::p().class("hint").child(move || match run.get() {
            Some((first, last)) => {
                let mut told = FluentArgs::new();
                told.set("count", last - first + 1);

                speech::say_with("share-sections-chosen", &told)
            }
            None => speech::say("share-sections-none"),
        }),
    )
}

pub(crate) fn date_field(
    label: &'static str,
    shown: impl Fn() -> Option<Date> + Send + Sync + 'static,
    chosen: impl Fn(Date) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    move || {
        shown().map(|day| {
            html::label().class("date").child((
                html::span().child(speech::said(label)),
                html::input()
                    .r#type("date")
                    .prop("value", day_written(day))
                    .on(ev::change, move |event| {
                        if let Some(day) = day_read(&event_target_value(&event)) {
                            chosen(day);
                        }
                    }),
            ))
        })
    }
}

fn how_long_open(
    open: RwSignal<OpenFor>,
    today: Date,
    times: Memo<(Option<OffsetDateTime>, Option<OffsetDateTime>)>,
) -> impl IntoView {
    let until = today + Duration::days(OPEN_BY_DEFAULT);

    html::div().class("field").child((
        html::span()
            .class("label")
            .child(speech::said("share-open")),
        chosen_among(
            "share-open",
            open,
            vec![
                ("share-open-7", OpenFor::Days(7)),
                ("share-open-30", OpenFor::Days(30)),
                ("share-open-90", OpenFor::Days(90)),
                ("share-open-no-end", OpenFor::NoEnd),
                ("share-open-until", OpenFor::Until(until)),
            ],
            |chosen, option| match (chosen, option) {
                (OpenFor::Until(_), OpenFor::Until(_)) => true,
                (chosen, option) => chosen == option,
            },
        ),
        date_field(
            "share-open-on",
            move || match open.get() {
                OpenFor::Until(day) => Some(day),
                _ => None,
            },
            move |day| open.set(OpenFor::Until(day)),
        ),
        html::p().class("hint").child(move || match times.get().0 {
            Some(until) => said_with("share-open-hint", &[("day", moment::human_day(until))]),
            None => speech::say("share-open-hint-no-end"),
        }),
    ))
}

fn how_long_kept(
    kept: RwSignal<KeptFor>,
    today: Date,
    times: Memo<(Option<OffsetDateTime>, Option<OffsetDateTime>)>,
) -> impl IntoView {
    let on = today + Duration::days(KEPT_BY_DEFAULT);

    html::div().class("field").child((
        html::span()
            .class("label")
            .child(speech::said("share-kept")),
        chosen_among(
            "share-kept",
            kept,
            vec![
                ("share-kept-90", KeptFor::Days(90)),
                ("share-kept-365", KeptFor::Days(365)),
                ("share-kept-ever", KeptFor::Ever),
                ("share-kept-until", KeptFor::Until(on)),
            ],
            |chosen, option| match (chosen, option) {
                (KeptFor::Until(_), KeptFor::Until(_)) => true,
                (chosen, option) => chosen == option,
            },
        ),
        date_field(
            "share-kept-on",
            move || match kept.get() {
                KeptFor::Until(day) => Some(day),
                _ => None,
            },
            move |day| kept.set(KeptFor::Until(day)),
        ),
        html::p().class("hint").child(move || match times.get().1 {
            Some(on) => said_with("share-kept-hint", &[("day", moment::human_day(on))]),
            None => speech::say("share-kept-hint-ever"),
        }),
    ))
}

/// The scenes in reading order under their sections, ticked as a run like the sections; a section's title is not ticked itself.
fn scenes_to_share(
    sections: LocalResource<Vec<OutlinedSection>>,
    names: LocalResource<HashMap<SceneId, String>>,
    run: RwSignal<Option<(usize, usize)>>,
) -> impl IntoView {
    let in_run = move |at: usize| {
        run.get()
            .is_some_and(|(first, last)| first <= at && at <= last)
    };

    (
        html::ul()
            .class("tree")
            .attr("aria-label", speech::said("share-scenes"))
            .child(move || {
                let named = names.get().unwrap_or_default();
                sections.get().map(|held| {
                    let mut at = 0;
                    held.iter()
                        .flat_map(|outlined| {
                            let next = at;
                            let between = move || next > 0 && in_run(next - 1) && in_run(next);
                            let heading = html::li()
                                .class(move || if between() { "heading in" } else { "heading" })
                                .style(("--depth", outlined.depth.to_string()))
                                .child(outlined.title.clone())
                                .into_any();
                            let scenes = outlined.scenes.iter().map(|scene| {
                                let here = at;
                                at += 1;

                                html::li()
                                    .class(move || if in_run(here) { "in" } else { "" })
                                    .style(("--depth", (outlined.depth + 1).to_string()))
                                    .child(
                                        html::label().child((
                                            html::input()
                                                .r#type("checkbox")
                                                .prop("checked", move || in_run(here))
                                                .on(ev::change, move |_| {
                                                    run.update(|chosen| {
                                                        *chosen = sharing::ticked(*chosen, here);
                                                    });
                                                }),
                                            html::span().child(
                                                named
                                                    .get(scene)
                                                    .cloned()
                                                    .unwrap_or_else(|| speech::say("scene-empty")),
                                            ),
                                        )),
                                    )
                                    .into_any()
                            });

                            std::iter::once(heading).chain(scenes).collect::<Vec<_>>()
                        })
                        .collect::<Vec<_>>()
                })
            }),
        html::p().class("hint").child(move || match run.get() {
            Some((first, last)) => {
                let mut told = FluentArgs::new();
                told.set("count", last - first + 1);

                speech::say_with("share-scenes-chosen", &told)
            }
            None => speech::say("share-scenes-none"),
        }),
    )
}
