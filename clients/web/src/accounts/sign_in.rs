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
    let speech = expect_context::<Speech>();
    let code = RwSignal::new(String::new());
    let refused = RwSignal::new(None::<ApiError>);
    let asking = RwSignal::new(false);

    let verify = move || {
        let (email, typed) = (page.email.get_untracked(), code.get_untracked());
        asked(asking, refused, async move {
            service::verify(&email, &typed)
                .await
                .map(|()| page.turn_to(Form::SignIn, Some("code-confirmed")))
        });
    };
    let send_again = move || {
        let email = page.email.get_untracked();
        asked(asking, refused, async move {
            service::send_code_again(&email)
                .await
                .map(|()| page.notice.set(Some("code-sent-again")))
        });
    };
    let sent_to = move || {
        let mut args = FluentArgs::new();
        args.set("email", page.email.get());
        speech.say_with("code-sent", &args)
    };

    (
        html::h1().child(said("code-heading")),
        html::p().class("sent-to").child(sent_to),
        notice(page),
        form(
            verify,
            (
                html::label().class("field").child((
                    html::span().child(said("code-field")),
                    html::input()
                        .r#type("text")
                        .attr("inputmode", "numeric")
                        .attr("autocomplete", "one-time-code")
                        .required(true)
                        .prop("value", move || code.get())
                        .on(ev::input, move |event| code.set(event_target_value(&event))),
                )),
                move || refused.get().map(problem),
                submitting(said("code-confirm"), asking),
            ),
        ),
        turning(said("code-again"), send_again),
        turning(said("sign-in-instead"), move || {
            page.turn_to(Form::SignIn, None)
        }),
    )
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
