use fluent_bundle::FluentArgs;
use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev};

use crate::icons::{Icon, mark};
use crate::speech::language::Language;
use crate::speech::{Speech, said};

pub fn picker() -> impl IntoView {
    let speech = expect_context::<Speech>();

    html::div().class("speaking").child((
        html::span()
            .attr("aria-hidden", "true")
            .child((move || speech.language().short_name(), mark(Icon::Open))),
        choices(speech),
    ))
}

fn choices(speech: Speech) -> impl IntoView {
    html::select()
        .attr("aria-label", said("language"))
        .on(ev::change, move |event| {
            if let Some(chosen) = Language::told(&event_target_value(&event)) {
                speech.speak(chosen);
            }
        })
        .child(
            Language::EVERY
                .into_iter()
                .map(|language| {
                    html::option()
                        .value(language.tag())
                        .prop("selected", move || speech.language() == language)
                        .child(move || named(language, speech))
                })
                .collect::<Vec<_>>(),
        )
}

fn named(language: Language, speech: Speech) -> String {
    if !language.machine_translated() {
        return language.own_name().to_owned();
    }
    let mut args = FluentArgs::new();
    args.set("language", language.own_name());

    speech.say_with("language-machine-translated", &args)
}
