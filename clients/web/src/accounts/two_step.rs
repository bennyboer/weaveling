use accounts_contract::SettingUpSecondFactorDTO;
use leptos::prelude::*;
use leptos::{IntoView, component, ev, html, view};
use qrcodegen::{QrCode, QrCodeEcc};

use crate::accounts::sign_in::{code_boxes, is_whole};
use crate::accounts::{CurrentSession, SecondFactorInUse, SignInState, service};
use crate::act::{Act, Here};
use crate::copy_button::copy_button;
use crate::http::ApiError;
use crate::moment;
use crate::problem;
use crate::setup;
use crate::shell;
use crate::speech;

const QUIET_ZONE: i32 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    Overview,
    SettingUp(SettingUpSecondFactorDTO),
    Keeping {
        codes: Vec<String>,
        notice: &'static str,
    },
}

#[component]
pub fn TwoStepSignInPage() -> impl IntoView {
    let session = expect_context::<CurrentSession>();
    let step = RwSignal::new(Step::Overview);

    (
        shell::masthead(None),
        html::main()
            .class("operating two-step")
            .child(move || match step.get() {
                Step::Overview => overview(session, step).into_any(),
                Step::SettingUp(setting_up) => {
                    setting_up_view(session, setting_up, step).into_any()
                }
                Step::Keeping { codes, notice } => keeping(codes, notice, step).into_any(),
            }),
    )
}

fn overview(session: CurrentSession, step: RwSignal<Step>) -> impl IntoView {
    let refused = RwSignal::new(None::<ApiError>);
    let beginning = Act::new(move |(), _: Here| {
        refused.set(None);

        async move {
            match service::begin_setting_up_second_factor().await {
                Ok(setting_up) => step.set(Step::SettingUp(setting_up)),
                Err(failed) => refused.set(Some(failed)),
            }
        }
    });

    move || match session.state() {
        SignInState::SignedIn(account) => (
            html::h1().child(speech::said("two-step-heading")),
            match account.second_factor {
                Some(factor) => (in_use(factor), drawing(session, step)).into_any(),
                None => html::p()
                    .class("hint")
                    .child(speech::said("two-step-off-hint"))
                    .into_any(),
            },
            move || refused.get().map(problem::problem),
            if session.second_factors() {
                html::button()
                    .r#type("button")
                    .class("primary")
                    .attr("aria-busy", move || beginning.busy())
                    .on(ev::click, move |_| beginning.start(()))
                    .child(speech::said(if account.second_factor.is_some() {
                        "two-step-set-up-anew"
                    } else {
                        "two-step-set-up"
                    }))
                    .into_any()
            } else {
                html::p()
                    .class("hint")
                    .child(speech::said("two-step-unavailable"))
                    .into_any()
            },
        )
            .into_any(),
        _ => setup::without_accounts().into_any(),
    }
}

fn in_use(factor: SecondFactorInUse) -> impl IntoView {
    (
        html::p().class("state").child((
            html::span().class("on").child(speech::said("two-step-on")),
            speech::say_about(
                "two-step-on-since",
                "when",
                &moment::human_time(factor.since),
            ),
        )),
        html::p().class("hint").child(speech::say_about(
            "two-step-codes-left",
            "left",
            &factor.recovery_codes_left.to_string(),
        )),
        html::p()
            .class("hint")
            .child(speech::said("two-step-set-up-anew-hint")),
    )
}

fn setting_up_view(
    session: CurrentSession,
    setting_up: SettingUpSecondFactorDTO,
    step: RwSignal<Step>,
) -> impl IntoView {
    let code = RwSignal::new(String::new());
    let refused = RwSignal::new(None::<ApiError>);
    let finishing = Act::new(move |(), _: Here| {
        refused.set(None);
        let typed = code.get_untracked();

        async move {
            match service::finish_setting_up_second_factor(&typed).await {
                Ok(codes) => {
                    session.look_again();
                    step.set(Step::Keeping {
                        codes,
                        notice: "two-step-on-now",
                    });
                }
                Err(failed) => {
                    code.set(String::new());
                    refused.set(Some(failed));
                }
            }
        }
    });
    let secret = setting_up.secret.clone();

    Effect::new(move |_| {
        if is_whole(&code.get()) {
            finishing.start(());
        }
    });

    (
        html::h1().child(speech::said("two-step-set-up-heading")),
        html::p()
            .class("hint")
            .child(speech::said("two-step-set-up-hint")),
        html::div().class("steps").child((
            html::section().class("scanning").child((
                html::h2().child(speech::said("two-step-scan")),
                html::div().class("scan").child((
                    qr_code(&setting_up.otpauth_uri),
                    html::div().class("typing").child((
                        html::p()
                            .class("hint")
                            .child(speech::said("two-step-cannot-scan")),
                        html::code().class("key").child(grouped(&setting_up.secret)),
                        copy_button("two-step-copy-key", secret),
                    )),
                )),
            )),
            html::section().class("confirming").child((
                html::h2().child(speech::said("two-step-code")),
                html::form()
                    .on(ev::submit, move |event| {
                        event.prevent_default();
                        finishing.start(());
                    })
                    .child((
                        code_boxes(speech::said("code-field"), code),
                        move || refused.get().map(problem::problem),
                        html::div().class("buttons").child((
                            html::button()
                                .r#type("button")
                                .on(ev::click, move |_| step.set(Step::Overview))
                                .child(speech::said("two-step-cancel")),
                            html::button()
                                .r#type("submit")
                                .class("primary")
                                .attr("aria-busy", move || finishing.busy())
                                .child(speech::said("two-step-confirm")),
                        )),
                        html::p()
                            .class("hint")
                            .child(speech::said("two-step-nothing-yet")),
                    )),
            )),
        )),
    )
}

fn drawing(session: CurrentSession, step: RwSignal<Step>) -> impl IntoView {
    let asking = RwSignal::new(false);
    let code = RwSignal::new(String::new());
    let refused = RwSignal::new(None::<ApiError>);
    let drawing = Act::new(move |(), _: Here| {
        refused.set(None);
        let typed = code.get_untracked();

        async move {
            match service::draw_recovery_codes(&typed).await {
                Ok(codes) => {
                    session.look_again();
                    step.set(Step::Keeping {
                        codes,
                        notice: "two-step-codes-drawn",
                    });
                }
                Err(failed) => {
                    code.set(String::new());
                    refused.set(Some(failed));
                }
            }
        }
    });

    Effect::new(move |_| {
        if is_whole(&code.get()) {
            drawing.start(());
        }
    });

    html::section().class("drawing").child(move || {
        if asking.get() {
            html::form()
                .on(ev::submit, move |event| {
                    event.prevent_default();
                    drawing.start(());
                })
                .child((
                    html::p()
                        .class("hint")
                        .child(speech::said("two-step-draw-codes-hint")),
                    code_boxes(speech::said("two-step-draw-codes-code"), code),
                    move || refused.get().map(problem::problem),
                    html::div().class("buttons").child((
                        html::button()
                            .r#type("button")
                            .on(ev::click, move |_| {
                                code.set(String::new());
                                refused.set(None);
                                asking.set(false);
                            })
                            .child(speech::said("two-step-cancel")),
                        html::button()
                            .r#type("submit")
                            .class("primary")
                            .attr("aria-busy", move || drawing.busy())
                            .child(speech::said("two-step-draw-codes")),
                    )),
                ))
                .into_any()
        } else {
            html::button()
                .r#type("button")
                .on(ev::click, move |_| asking.set(true))
                .child(speech::said("two-step-draw-codes"))
                .into_any()
        }
    })
}

fn keeping(codes: Vec<String>, notice: &'static str, step: RwSignal<Step>) -> impl IntoView {
    let kept = RwSignal::new(false);
    let written = codes.join("\n");
    let to_copy = written.clone();

    (
        html::p()
            .class("notice")
            .role("status")
            .child(speech::said(notice)),
        html::h1().child(speech::said("two-step-codes-heading")),
        html::p()
            .class("hint")
            .child(speech::said("two-step-codes-hint")),
        html::ol().class("recovery-codes").child(
            codes
                .into_iter()
                .map(|code| html::li().child(code))
                .collect_view(),
        ),
        html::div().class("buttons").child((
            html::a()
                .class("button")
                .attr(
                    "href",
                    format!(
                        "data:text/plain;charset=utf-8,{}",
                        String::from(js_sys::encode_uri_component(&format!("{written}\n")))
                    ),
                )
                .attr("download", "weaveling-recovery-codes.txt")
                .child(speech::said("two-step-codes-download")),
            copy_button("two-step-codes-copy", to_copy),
            html::button()
                .r#type("button")
                .on(ev::click, |_| _ = window().print())
                .child(speech::said("two-step-codes-print")),
        )),
        html::div().class("done").child((
            html::label().class("kept").child((
                html::input()
                    .r#type("checkbox")
                    .prop("checked", move || kept.get())
                    .on(ev::change, move |event| {
                        kept.set(event_target_checked(&event))
                    }),
                html::span().child(speech::said("two-step-codes-kept")),
            )),
            html::button()
                .r#type("button")
                .class("primary")
                .prop("disabled", move || !kept.get())
                .on(ev::click, move |_| step.set(Step::Overview))
                .child(speech::said("two-step-done")),
        )),
    )
}

fn qr_code(text: &str) -> Option<impl IntoView + use<>> {
    let drawn = QrCode::encode_text(text, QrCodeEcc::Medium).ok()?;
    let size = drawn.size();
    let side = size + 2 * QUIET_ZONE;
    let dark: String = (0..size)
        .flat_map(|y| (0..size).map(move |x| (x, y)))
        .filter(|&(x, y)| drawn.get_module(x, y))
        .map(|(x, y)| format!("M{} {}h1v1h-1z", x + QUIET_ZONE, y + QUIET_ZONE))
        .collect();

    Some(view! {
        <svg
            class="qr-code"
            viewBox=format!("0 0 {side} {side}")
            role="img"
            aria-label=speech::said("two-step-qr")
        >
            <path d=dark />
        </svg>
    })
}

fn grouped(secret: &str) -> String {
    secret
        .as_bytes()
        .chunks(4)
        .map(|group| String::from_utf8_lossy(group).into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_to_type_is_shown_in_groups_of_four() {
        // expect: 32 characters read as eight groups
        assert_eq!(
            grouped("JBSWY3DPEHPK3PXPKZ4M2QHTJBSWY3DP"),
            "JBSW Y3DP EHPK 3PXP KZ4M 2QHT JBSW Y3DP"
        );
    }
}
