use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::http::{ApiError, Particular, Refusal};
use crate::size;
use crate::speech;
use crate::speech::Speech;

pub fn problem(failure: ApiError) -> impl IntoView {
    html::p()
        .class("problem")
        .role("alert")
        .child(told(failure))
}

pub fn problem_to_dismiss(
    failure: ApiError,
    dismiss: impl Fn() + Send + Sync + 'static,
) -> impl IntoView {
    html::p().class("problem").role("alert").child((
        told(failure),
        html::button()
            .r#type("button")
            .class("dismiss")
            .attr("aria-label", speech::said("problem-dismiss"))
            .on(ev::click, move |_| dismiss())
            .child("\u{00d7}"),
    ))
}

fn told(failure: ApiError) -> impl Fn() -> String + Send + Sync + 'static {
    let speech = expect_context::<Speech>();

    move || match &failure {
        ApiError::Rejected(refusal) => refused(refusal, speech),
        ApiError::NotFound(subject) => about("problem-not-found", subject, speech),
        ApiError::Outdated(subject) => about("problem-outdated", subject, speech),
        ApiError::Overtaken => speech.say("problem-overtaken"),
        ApiError::Offline => speech.say("problem-offline"),
        ApiError::Unexpected => speech.say("problem-unexpected"),
    }
}

fn about(message: &str, subject: &'static str, speech: Speech) -> String {
    let mut args = FluentArgs::new();
    args.set("subject", subject);

    speech.say_with(message, &args)
}

/// The particulars of a refusal that are sizes in bytes, so they are told as sizes.
const SIZES: [(&str, &[&str]); 1] = [(assets_contract::refused::LIMIT_REACHED, &["most", "used"])];

fn is_a_size(kind: Option<&str>, name: &str) -> bool {
    SIZES
        .iter()
        .any(|(refusal, sizes)| Some(*refusal) == kind && sizes.contains(&name))
}

fn refused(refusal: &Refusal, speech: Speech) -> String {
    let message = refusal
        .kind
        .as_ref()
        .map(|kind| format!("refused-{kind}"))
        .filter(|message| speech.knows(message));
    let Some(message) = message else {
        return refusal.detail.clone();
    };
    let mut args = FluentArgs::new();
    for (name, particular) in &refusal.particulars {
        match particular {
            Particular::Count(count) if is_a_size(refusal.kind.as_deref(), name) => args.set(
                name.clone(),
                size::told(u64::try_from(*count).unwrap_or(0), speech.language()),
            ),
            Particular::Count(count) => args.set(name.clone(), *count),
            Particular::Text(text) => args.set(name.clone(), text.clone()),
            Particular::Texts(texts) => args.set(name.clone(), texts.join(", ")),
        }
    }

    speech.say_with(&message, &args)
}
