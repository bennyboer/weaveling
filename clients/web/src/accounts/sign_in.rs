use accounts_contract::refused;
use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::task;
use leptos::{IntoView, component, ev, html};
use leptos_router::NavigateOptions;
use leptos_router::hooks;

use crate::accounts::{CurrentSession, SigningUp, service};
use crate::http::ApiError;
use crate::problem;
use crate::route;
use crate::shell;
use crate::speech;
use crate::speech::Speech;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Form {
    SignIn,
    SignUp,
    SetUp,
    Accept,
    EnterCode,
    AskReset,
    Reset,
}

#[derive(Clone, Copy)]
struct Page {
    form: RwSignal<Form>,
    email: RwSignal<String>,
    notice: RwSignal<Option<&'static str>>,
    invitation: RwSignal<Option<String>>,
    operator_code: RwSignal<Option<String>>,
}

impl Page {
    fn turn_to(self, form: Form, notice: Option<&'static str>) {
        self.notice.set(notice);
        self.form.set(form);
    }
}

#[component]
pub fn SignInPage() -> impl IntoView {
    let location = hooks::use_location();
    let invitation = (location.pathname.get_untracked() == route::INVITATION)
        .then(|| {
            location
                .hash
                .get_untracked()
                .trim_start_matches('#')
                .to_owned()
        })
        .filter(|token| !token.is_empty());
    let page = Page {
        form: RwSignal::new(if invitation.is_some() {
            Form::Accept
        } else if location.pathname.get_untracked() == route::SETUP
            && expect_context::<CurrentSession>().operator_wanted()
        {
            Form::SetUp
        } else {
            Form::SignIn
        }),
        email: RwSignal::new(String::new()),
        notice: RwSignal::new(None),
        invitation: RwSignal::new(invitation),
        operator_code: RwSignal::new(None),
    };

    (
        shell::masthead(None),
        html::main()
            .class("signing-in")
            .child(move || match page.form.get() {
                Form::SignIn => sign_in_form(page).into_any(),
                Form::SignUp => sign_up_form(page).into_any(),
                Form::SetUp => set_up_form(page).into_any(),
                Form::Accept => accept_form(page).into_any(),
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

    let navigate = hooks::use_navigate();
    let sign_in = move || {
        let (given, typed, operating) = (
            page.email.get_untracked(),
            password.get_untracked(),
            page.operator_code.get_untracked(),
        );
        let navigate = navigate.clone();
        asked(asking, refused, async move {
            let mut account = service::sign_in(&given, &typed).await?;
            if let Some(code) = operating
                && service::become_operator(&code).await.is_ok()
            {
                account.operator = true;
                navigate(route::INSTANCE_ACCOUNTS, NavigateOptions::default());
            }
            session.signed_in(account);

            Ok(())
        });
    };
    let not_verified = move || {
        matches!(refused.get(), Some(ApiError::Rejected(refusal))
            if refusal.kind.as_deref() == Some(refused::NOT_VERIFIED))
    };

    (
        html::h1().child(speech::said("sign-in-heading")),
        notice(page),
        form(
            sign_in,
            (
                field(
                    speech::said("sign-in-email"),
                    "email",
                    "username",
                    page.email,
                ),
                field(
                    speech::said("sign-in-password"),
                    "password",
                    "current-password",
                    password,
                ),
                move || refused.get().map(problem::problem),
                move || {
                    not_verified().then(|| {
                        turning(speech::said("code-enter"), move || {
                            page.turn_to(Form::EnterCode, None)
                        })
                    })
                },
                submitting(speech::said("sign-in-confirm"), asking),
            ),
        ),
        turning(speech::said("reset-instead"), move || {
            page.turn_to(Form::AskReset, None)
        }),
        move || {
            (session.signing_up() == SigningUp::Open).then(|| {
                turning(speech::said("sign-up-instead"), move || {
                    page.turn_to(Form::SignUp, None)
                })
            })
        },
        move || {
            session.operator_wanted().then(|| {
                turning(speech::said("set-up-instead"), move || {
                    page.turn_to(Form::SetUp, None)
                })
            })
        },
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
            service::sign_up(&email, &named, &typed, None)
                .await
                .map(|()| page.turn_to(Form::EnterCode, None))
        });
    };

    (
        html::h1().child(speech::said("sign-up-heading")),
        form(
            sign_up,
            (
                field(speech::said("sign-up-name"), "text", "name", name),
                field(speech::said("sign-in-email"), "email", "email", page.email),
                field(
                    speech::said("sign-in-password"),
                    "password",
                    "new-password",
                    password,
                ),
                html::small()
                    .class("hint")
                    .child(speech::said("sign-up-password-hint")),
                move || refused.get().map(problem::problem),
                submitting(speech::said("sign-up-confirm"), asking),
            ),
        ),
        turning(speech::said("sign-in-instead"), move || {
            page.turn_to(Form::SignIn, None)
        }),
    )
}

fn set_up_form(page: Page) -> impl IntoView {
    let name = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let code = RwSignal::new(String::new());
    let refused = RwSignal::new(None::<ApiError>);
    let asking = RwSignal::new(false);

    let set_up = move || {
        let (email, named, typed, logged) = (
            page.email.get_untracked(),
            name.get_untracked(),
            password.get_untracked(),
            code.get_untracked(),
        );
        asked(asking, refused, async move {
            service::sign_up(&email, &named, &typed, Some(&logged))
                .await
                .map(|()| {
                    page.operator_code.set(Some(logged));
                    page.turn_to(Form::EnterCode, None);
                })
        });
    };

    (
        html::h1().child(speech::said("set-up-heading")),
        html::p().class("hint").child(speech::said("set-up-hint")),
        form(
            set_up,
            (
                field(speech::said("set-up-code"), "text", "off", code),
                field(speech::said("sign-up-name"), "text", "name", name),
                field(speech::said("sign-in-email"), "email", "email", page.email),
                field(
                    speech::said("sign-in-password"),
                    "password",
                    "new-password",
                    password,
                ),
                html::small()
                    .class("hint")
                    .child(speech::said("sign-up-password-hint")),
                move || refused.get().map(problem::problem),
                submitting(speech::said("sign-up-confirm"), asking),
            ),
        ),
        turning(speech::said("sign-in-instead"), move || {
            page.turn_to(Form::SignIn, None)
        }),
    )
}

fn accept_form(page: Page) -> impl IntoView {
    let session = expect_context::<CurrentSession>();
    let navigate = hooks::use_navigate();
    let name = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let refused = RwSignal::new(None::<ApiError>);
    let asking = RwSignal::new(false);

    let accept = move || {
        let (token, named, typed) = (
            page.invitation.get_untracked().unwrap_or_default(),
            name.get_untracked(),
            password.get_untracked(),
        );
        let navigate = navigate.clone();
        asked(asking, refused, async move {
            service::sign_up_by_invitation(&token, &named, &typed)
                .await
                .map(|account| {
                    navigate(
                        route::WORKSPACE,
                        NavigateOptions {
                            replace: true,
                            ..Default::default()
                        },
                    );
                    session.signed_in(account);
                })
        });
    };

    (
        html::h1().child(speech::said("invitation-accept-heading")),
        html::p()
            .class("hint")
            .child(speech::said("invitation-accept-hint")),
        form(
            accept,
            (
                field(speech::said("sign-up-name"), "text", "name", name),
                field(
                    speech::said("sign-in-password"),
                    "password",
                    "new-password",
                    password,
                ),
                html::small()
                    .class("hint")
                    .child(speech::said("sign-up-password-hint")),
                move || refused.get().map(problem::problem),
                submitting(speech::said("sign-up-confirm"), asking),
            ),
        ),
        turning(speech::said("sign-in-instead"), move || {
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
        html::h1().child(speech::said("code-heading")),
        sent_to(page),
        notice(page),
        form(
            verify,
            (
                code_boxes(code),
                move || refused.get().map(problem::problem),
                submitting(speech::said("code-confirm"), asking),
            ),
        ),
        turning(speech::said("code-again"), send_again),
        turning(speech::said("back-to-sign-in"), move || {
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
        html::h1().child(speech::said("reset-heading")),
        form(
            ask,
            (
                field(
                    speech::said("sign-in-email"),
                    "email",
                    "username",
                    page.email,
                ),
                move || refused.get().map(problem::problem),
                submitting(speech::said("reset-ask"), asking),
            ),
        ),
        turning(speech::said("back-to-sign-in"), move || {
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
        html::h1().child(speech::said("reset-heading")),
        sent_to(page),
        notice(page),
        form(
            reset,
            (
                code_boxes(code),
                field(
                    speech::said("reset-password"),
                    "password",
                    "new-password",
                    password,
                ),
                html::small()
                    .class("hint")
                    .child(speech::said("sign-up-password-hint")),
                move || refused.get().map(problem::problem),
                submitting(speech::said("reset-confirm"), asking),
            ),
        ),
        turning(speech::said("code-again"), send_again),
        turning(speech::said("back-to-sign-in"), move || {
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
        html::span().child(speech::said("code-field")),
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

    task::spawn_local(async move {
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
                .child(speech::said(message))
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
