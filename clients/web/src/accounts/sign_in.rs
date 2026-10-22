use accounts_contract::refused;
use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::{IntoView, component, ev, html};

use crate::accounts::{CurrentSession, service};
use crate::http::ApiError;
use crate::problem::problem;
use crate::shell::masthead;
use crate::speech::{Speech, said};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Form {
    SignIn,
    SignUp,
    EnterCode,
    AskReset,
    Reset,
}

#[derive(Clone, Copy)]
struct Page {
    form: RwSignal<Form>,
    email: RwSignal<String>,
    notice: RwSignal<Option<&'static str>>,
}

impl Page {
    fn turn_to(self, form: Form, notice: Option<&'static str>) {
        self.notice.set(notice);
        self.form.set(form);
    }
}

#[component]
pub fn SignInPage() -> impl IntoView {
    let page = Page {
        form: RwSignal::new(Form::SignIn),
        email: RwSignal::new(String::new()),
        notice: RwSignal::new(None),
    };

    (
        masthead(None),
        html::main()
            .class("signing-in")
            .child(move || match page.form.get() {
                Form::SignIn => sign_in_form(page).into_any(),
                Form::SignUp => sign_up_form(page).into_any(),
                Form::EnterCode => code_form(page).into_any(),
                Form::AskReset => ask_reset_form(page).into_any(),
                Form::Reset => reset_form(page).into_any(),
            }),
    )
}

fn sign_in_form(page: Page) -> impl IntoView {
    let session = expect_context::<CurrentSession>();
    let password = RwSignal::new(String::new());
    let refused = RwSignal::new(None::<ApiError>);
    let asking = RwSignal::new(false);

    let sign_in = move || {
        let (given, typed) = (page.email.get_untracked(), password.get_untracked());
        asked(asking, refused, async move {
            service::sign_in(&given, &typed)
                .await
                .map(|account| session.signed_in(account))
        });
    };
    let not_verified = move || {
        matches!(refused.get(), Some(ApiError::Rejected(refusal))
            if refusal.kind.as_deref() == Some(refused::NOT_VERIFIED))
    };

    (
        html::h1().child(said("sign-in-heading")),
        notice(page),
        form(
            sign_in,
            (
                field(said("sign-in-email"), "email", "username", page.email),
                field(
                    said("sign-in-password"),
                    "password",
                    "current-password",
                    password,
                ),
                move || refused.get().map(problem),
                move || {
                    not_verified().then(|| {
                        turning(said("code-enter"), move || {
                            page.turn_to(Form::EnterCode, None)
                        })
                    })
                },
                submitting(said("sign-in-confirm"), asking),
            ),
        ),
        turning(said("reset-instead"), move || {
            page.turn_to(Form::AskReset, None)
        }),
        turning(said("sign-up-instead"), move || {
            page.turn_to(Form::SignUp, None)
        }),
    )
}

fn sign_up_form(page: Page) -> impl IntoView {
    let name = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let refused = RwSignal::new(None::<ApiError>);
    let asking = RwSignal::new(false);

    let sign_up = move || {
        let (email, named, typed) = (
            page.email.get_untracked(),
            name.get_untracked(),
            password.get_untracked(),
        );
        asked(asking, refused, async move {
            service::sign_up(&email, &named, &typed)
                .await
                .map(|()| page.turn_to(Form::EnterCode, None))
        });
    };

    (
        html::h1().child(said("sign-up-heading")),
        form(
            sign_up,
            (
                field(said("sign-up-name"), "text", "name", name),
                field(said("sign-in-email"), "email", "email", page.email),
                field(
                    said("sign-in-password"),
                    "password",
                    "new-password",
                    password,
                ),
                html::small()
                    .class("hint")
                    .child(said("sign-up-password-hint")),
                move || refused.get().map(problem),
                submitting(said("sign-up-confirm"), asking),
            ),
        ),
        turning(said("sign-in-instead"), move || {
            page.turn_to(Form::SignIn, None)
        }),
    )
}

fn code_form(page: Page) -> impl IntoView {
    let code = RwSignal::new(String::new());
    let refused = RwSignal::new(None::<ApiError>);
    let asking = RwSignal::new(false);

    let verify = move || {
        let (email, typed) = (page.email.get_untracked(), code.get_untracked());
        asked(asking, refused, async move {
            let verified = service::verify(&email, &typed).await;
            if verified.is_err() {
                code.set(String::new());
            }
            verified.map(|()| page.turn_to(Form::SignIn, Some("code-confirmed")))
        });
    };
    let send_again = move || {
        let email = page.email.get_untracked();
        asked(asking, refused, async move {
            service::send_code_again(&email).await.map(|()| {
                code.set(String::new());
                page.notice.set(Some("code-sent-again"));
            })
        });
    };

    Effect::new(move |_| {
        if is_whole(&code.get()) {
            verify();
        }
    });

    (
        html::h1().child(said("code-heading")),
        sent_to(page),
        notice(page),
        form(
            verify,
            (
                code_boxes(code),
                move || refused.get().map(problem),
                submitting(said("code-confirm"), asking),
            ),
        ),
        turning(said("code-again"), send_again),
        turning(said("back-to-sign-in"), move || {
            page.turn_to(Form::SignIn, None)
        }),
    )
}

fn ask_reset_form(page: Page) -> impl IntoView {
    let refused = RwSignal::new(None::<ApiError>);
    let asking = RwSignal::new(false);

    let ask = move || {
        let email = page.email.get_untracked();
        asked(asking, refused, async move {
            service::ask_to_reset_password(&email)
                .await
                .map(|()| page.turn_to(Form::Reset, None))
        });
    };

    (
        html::h1().child(said("reset-heading")),
        form(
            ask,
            (
                field(said("sign-in-email"), "email", "username", page.email),
                move || refused.get().map(problem),
                submitting(said("reset-ask"), asking),
            ),
        ),
        turning(said("back-to-sign-in"), move || {
            page.turn_to(Form::SignIn, None)
        }),
    )
}

fn reset_form(page: Page) -> impl IntoView {
    let code = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let refused = RwSignal::new(None::<ApiError>);
    let asking = RwSignal::new(false);

    let reset = move || {
        let (email, typed, chosen) = (
            page.email.get_untracked(),
            code.get_untracked(),
            password.get_untracked(),
        );
        asked(asking, refused, async move {
            let reset = service::reset_password(&email, &typed, &chosen).await;
            if refused_the_code(&reset) {
                code.set(String::new());
            }
            reset.map(|()| page.turn_to(Form::SignIn, Some("reset-done")))
        });
    };
    let send_again = move || {
        let email = page.email.get_untracked();
        asked(asking, refused, async move {
            service::ask_to_reset_password(&email).await.map(|()| {
                code.set(String::new());
                page.notice.set(Some("code-sent-again"));
            })
        });
    };

    (
        html::h1().child(said("reset-heading")),
        sent_to(page),
        notice(page),
        form(
            reset,
            (
                code_boxes(code),
                field(said("reset-password"), "password", "new-password", password),
                html::small()
                    .class("hint")
                    .child(said("sign-up-password-hint")),
                move || refused.get().map(problem),
                submitting(said("reset-confirm"), asking),
            ),
        ),
        turning(said("code-again"), send_again),
        turning(said("back-to-sign-in"), move || {
            page.turn_to(Form::SignIn, None)
        }),
    )
}

fn refused_the_code(answer: &Result<(), ApiError>) -> bool {
    matches!(answer, Err(ApiError::Rejected(refusal))
    if refusal.kind.as_deref().is_some_and(|kind| {
        [refused::CODE_WRONG, refused::CODE_EXPIRED, refused::CODE_EXHAUSTED].contains(&kind)
    }))
}

fn sent_to(page: Page) -> impl IntoView {
    let speech = expect_context::<Speech>();

    html::p().class("sent-to").child(move || {
        let mut args = FluentArgs::new();
        args.set("email", page.email.get());
        speech.say_with("code-sent", &args)
    })
}

const CODE_LENGTH: usize = 6;

fn code_boxes(code: RwSignal<String>) -> impl IntoView {
    let focused = RwSignal::new(false);

    html::label().class("field").child((
        html::span().child(said("code-field")),
        html::div().class("code-boxes").child((
            html::input()
                .r#type("text")
                .attr("inputmode", "numeric")
                .attr("autocomplete", "one-time-code")
                .required(true)
                .prop("value", move || code.get())
                .on(ev::input, move |event| {
                    code.set(digits_of(&event_target_value(&event)))
                })
                .on(ev::focus, move |_| focused.set(true))
                .on(ev::blur, move |_| focused.set(false)),
            (0..CODE_LENGTH)
                .map(|place| {
                    html::span()
                        .class("code-box")
                        .class(("next", move || focused.get() && code.get().len() == place))
                        .attr("aria-hidden", "true")
                        .child(move || code.get().chars().nth(place).map(String::from))
                })
                .collect_view(),
        )),
    ))
}

fn digits_of(typed: &str) -> String {
    typed
        .chars()
        .filter(char::is_ascii_digit)
        .take(CODE_LENGTH)
        .collect()
}

fn is_whole(code: &str) -> bool {
    code.len() == CODE_LENGTH
}

fn asked(
    asking: RwSignal<bool>,
    refused: RwSignal<Option<ApiError>>,
    answer: impl Future<Output = Result<(), ApiError>> + 'static,
) {
    if asking.get_untracked() {
        return;
    }
    asking.set(true);
    refused.set(None);

    spawn_local(async move {
        if let Err(failed) = answer.await {
            refused.set(Some(failed));
        }
        asking.set(false);
    });
}

fn form(submit: impl Fn() + 'static, fields: impl IntoView + 'static) -> impl IntoView {
    html::form()
        .on(ev::submit, move |event| {
            event.prevent_default();
            submit();
        })
        .child(fields)
}

fn notice(page: Page) -> impl IntoView {
    move || {
        page.notice.get().map(|message| {
            html::p()
                .class("notice")
                .role("status")
                .child(said(message))
        })
    }
}

fn submitting(
    label: impl Fn() -> String + Send + Sync + 'static,
    asking: RwSignal<bool>,
) -> impl IntoView {
    html::button()
        .r#type("submit")
        .class("primary")
        .disabled(move || asking.get())
        .child(label)
}

fn turning(
    label: impl Fn() -> String + Send + Sync + 'static,
    turn: impl Fn() + 'static,
) -> impl IntoView {
    html::button()
        .r#type("button")
        .class("turning")
        .on(ev::click, move |_| turn())
        .child(label)
}

fn field(
    label: impl Fn() -> String + Send + Sync + 'static,
    kind: &'static str,
    autocomplete: &'static str,
    value: RwSignal<String>,
) -> impl IntoView {
    html::label().class("field").child((
        html::span().child(label),
        html::input()
            .r#type(kind)
            .attr("autocomplete", autocomplete)
            .required(true)
            .prop("value", move || value.get())
            .on(ev::input, move |event| {
                value.set(event_target_value(&event))
            }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_digits_count_in_a_code_however_it_was_pasted() {
        assert_eq!(digits_of("123 456"), "123456");
        assert_eq!(digits_of("12ab34"), "1234");
        assert_eq!(
            digits_of("1234567"),
            "123456",
            "a code is six digits, and a seventh is a slip of the finger"
        );
    }

    #[test]
    fn a_code_is_whole_at_six_digits_and_not_before() {
        assert!(!is_whole("12345"));
        assert!(is_whole("123456"));
    }
}
