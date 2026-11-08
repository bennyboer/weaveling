use accounts_contract::refused;
use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::{IntoView, component, ev, html};
use leptos_router::NavigateOptions;
use leptos_router::hooks;

use crate::accounts::authenticator::{self, NoKey};
use crate::accounts::{
    CurrentSession, PreferredWay, SecondFactorInUse, SecondStepWay, SignInState, SignedInAccount,
    SigningUp, service,
};
use crate::act::{Act, Here};
use crate::http::ApiError;
use crate::icons::{self, Icon};
use crate::invitations::service as invitations;
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
    Declined,
    EnterCode,
    AskReset,
    Reset,
    SecondStep,
    KeyStep,
    RecoveryStep,
    AnotherWay,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InvitationAnswer {
    Accept,
    Decline,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CodeAsk {
    Confirm,
    SendAgain,
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
    let session = expect_context::<CurrentSession>();
    let owing = match session.state() {
        SignInState::OwingSecondFactor(account) => Some(account),
        _ => None,
    };
    let page = Page {
        form: RwSignal::new(if let Some(account) = &owing {
            asked_form(account)
        } else if invitation.is_some() {
            Form::Accept
        } else if location.pathname.get_untracked() == route::SETUP
            && expect_context::<CurrentSession>().operator_wanted()
        {
            Form::SetUp
        } else {
            Form::SignIn
        }),
        email: RwSignal::new(owing.map(|account| account.email).unwrap_or_default()),
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
                Form::Declined => declined_form(page).into_any(),
                Form::EnterCode => code_form(page).into_any(),
                Form::AskReset => ask_reset_form(page).into_any(),
                Form::Reset => reset_form(page).into_any(),
                Form::SecondStep => second_step_form(page).into_any(),
                Form::KeyStep => key_step_form(page).into_any(),
                Form::RecoveryStep => recovery_step_form(page).into_any(),
                Form::AnotherWay => another_way_form(page).into_any(),
            }),
    )
}

fn sign_in_form(page: Page) -> impl IntoView {
    let session = expect_context::<CurrentSession>();
    let password = RwSignal::new(String::new());
    let refused = RwSignal::new(None::<ApiError>);

    let navigate = hooks::use_navigate();
    let signing_in = Act::new(move |(), here: Here| {
        let (given, typed, operating) = (
            page.email.get_untracked(),
            password.get_untracked(),
            page.operator_code.get_untracked(),
        );
        let navigate = navigate.clone();
        asked(refused, async move {
            let mut account = service::sign_in(&given, &typed).await?;
            if account.owes_second_factor {
                let first = asked_form(&account);
                session.owes_second_factor(account);
                page.turn_to(first, None);
                return Ok(());
            }
            if let Some(code) = operating
                && service::become_operator(&code).await.is_ok()
            {
                account.operator = true;
                if here.still() {
                    navigate(route::INSTANCE_ACCOUNTS, NavigateOptions::default());
                }
            }
            session.signed_in(account);

            Ok(())
        })
    });
    let not_verified = move || {
        matches!(refused.get(), Some(ApiError::Rejected(refusal))
            if refusal.kind.as_deref() == Some(refused::NOT_VERIFIED))
    };

    (
        html::h1().child(speech::said("sign-in-heading")),
        notice(page),
        form(
            move || signing_in.start(()),
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
                submitting(speech::said("sign-in-confirm"), signing_in, ()),
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

    let signing_up = Act::new(move |(), _: Here| {
        let (email, named, typed) = (
            page.email.get_untracked(),
            name.get_untracked(),
            password.get_untracked(),
        );
        asked(refused, async move {
            service::sign_up(&email, &named, &typed, None)
                .await
                .map(|()| page.turn_to(Form::EnterCode, None))
        })
    });

    (
        html::h1().child(speech::said("sign-up-heading")),
        form(
            move || signing_up.start(()),
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
                submitting(speech::said("sign-up-confirm"), signing_up, ()),
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

    let setting_up = Act::new(move |(), _: Here| {
        let (email, named, typed, logged) = (
            page.email.get_untracked(),
            name.get_untracked(),
            password.get_untracked(),
            code.get_untracked(),
        );
        asked(refused, async move {
            service::sign_up(&email, &named, &typed, Some(&logged))
                .await
                .map(|()| {
                    page.operator_code.set(Some(logged));
                    page.turn_to(Form::EnterCode, None);
                })
        })
    });

    (
        html::h1().child(speech::said("set-up-heading")),
        html::p().class("hint").child(speech::said("set-up-hint")),
        form(
            move || setting_up.start(()),
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
                submitting(speech::said("sign-up-confirm"), setting_up, ()),
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

    let answering = Act::new(move |answer: InvitationAnswer, here: Here| {
        let (token, named, typed) = (
            page.invitation.get_untracked().unwrap_or_default(),
            name.get_untracked(),
            password.get_untracked(),
        );
        let navigate = navigate.clone();
        asked(refused, async move {
            match answer {
                InvitationAnswer::Accept => service::sign_up_by_invitation(&token, &named, &typed)
                    .await
                    .map(|account| {
                        if here.still() {
                            navigate(
                                route::WORKSPACE,
                                NavigateOptions {
                                    replace: true,
                                    ..Default::default()
                                },
                            );
                        }
                        session.signed_in(account);
                    }),
                InvitationAnswer::Decline => invitations::decline_by_token(&token)
                    .await
                    .map(|()| page.turn_to(Form::Declined, None)),
            }
        })
    });

    (
        html::h1().child(speech::said("invitation-accept-heading")),
        html::p()
            .class("hint")
            .child(speech::said("invitation-accept-hint")),
        form(
            move || answering.start(InvitationAnswer::Accept),
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
                submitting(
                    speech::said("sign-up-confirm"),
                    answering,
                    InvitationAnswer::Accept,
                ),
            ),
        ),
        turning_asking(
            speech::said("invitation-decline-instead"),
            answering,
            InvitationAnswer::Decline,
        ),
        turning(speech::said("sign-in-instead"), move || {
            page.turn_to(Form::SignIn, None)
        }),
    )
}

fn declined_form(page: Page) -> impl IntoView {
    (
        html::h1().child(speech::said("invitation-declined-heading")),
        html::p()
            .class("hint")
            .child(speech::said("invitation-declined-hint")),
        turning(speech::said("sign-in-instead"), move || {
            page.turn_to(Form::SignIn, None)
        }),
    )
}

fn code_form(page: Page) -> impl IntoView {
    let code = RwSignal::new(String::new());
    let refused = RwSignal::new(None::<ApiError>);

    let verifying = Act::new(move |ask: CodeAsk, _: Here| {
        let (email, typed) = (page.email.get_untracked(), code.get_untracked());
        asked(refused, async move {
            match ask {
                CodeAsk::Confirm => {
                    let verified = service::verify(&email, &typed).await;
                    if verified.is_err() {
                        code.set(String::new());
                    }
                    verified.map(|()| page.turn_to(Form::SignIn, Some("code-confirmed")))
                }
                CodeAsk::SendAgain => service::send_code_again(&email).await.map(|()| {
                    code.set(String::new());
                    page.notice.set(Some("code-sent-again"));
                }),
            }
        })
    });

    Effect::new(move |_| {
        if is_whole(&code.get()) {
            verifying.start(CodeAsk::Confirm);
        }
    });

    (
        html::h1().child(speech::said("code-heading")),
        sent_to(page),
        notice(page),
        form(
            move || verifying.start(CodeAsk::Confirm),
            (
                code_boxes(speech::said("code-field"), code),
                move || refused.get().map(problem::problem),
                submitting(speech::said("code-confirm"), verifying, CodeAsk::Confirm),
            ),
        ),
        turning_asking(speech::said("code-again"), verifying, CodeAsk::SendAgain),
        turning(speech::said("back-to-sign-in"), move || {
            page.turn_to(Form::SignIn, None)
        }),
    )
}

fn ask_reset_form(page: Page) -> impl IntoView {
    let refused = RwSignal::new(None::<ApiError>);

    let asking = Act::new(move |(), _: Here| {
        let email = page.email.get_untracked();
        asked(refused, async move {
            service::ask_to_reset_password(&email)
                .await
                .map(|()| page.turn_to(Form::Reset, None))
        })
    });

    (
        html::h1().child(speech::said("reset-heading")),
        form(
            move || asking.start(()),
            (
                field(
                    speech::said("sign-in-email"),
                    "email",
                    "username",
                    page.email,
                ),
                move || refused.get().map(problem::problem),
                submitting(speech::said("reset-ask"), asking, ()),
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

    let resetting = Act::new(move |ask: CodeAsk, _: Here| {
        let (email, typed, chosen) = (
            page.email.get_untracked(),
            code.get_untracked(),
            password.get_untracked(),
        );
        asked(refused, async move {
            match ask {
                CodeAsk::Confirm => {
                    let reset = service::reset_password(&email, &typed, &chosen).await;
                    if refused_the_code(&reset) {
                        code.set(String::new());
                    }
                    reset.map(|()| page.turn_to(Form::SignIn, Some("reset-done")))
                }
                CodeAsk::SendAgain => service::ask_to_reset_password(&email).await.map(|()| {
                    code.set(String::new());
                    page.notice.set(Some("code-sent-again"));
                }),
            }
        })
    });

    (
        html::h1().child(speech::said("reset-heading")),
        sent_to(page),
        notice(page),
        form(
            move || resetting.start(CodeAsk::Confirm),
            (
                code_boxes(speech::said("code-field"), code),
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
                submitting(speech::said("reset-confirm"), resetting, CodeAsk::Confirm),
            ),
        ),
        turning_asking(speech::said("code-again"), resetting, CodeAsk::SendAgain),
        turning(speech::said("back-to-sign-in"), move || {
            page.turn_to(Form::SignIn, None)
        }),
    )
}

fn second_step_form(page: Page) -> impl IntoView {
    let session = expect_context::<CurrentSession>();
    let code = RwSignal::new(String::new());
    let refused = RwSignal::new(None::<ApiError>);
    let first_from_now_on = RwSignal::new(false);
    let answering = Act::new(move |(), _: Here| {
        let typed = code.get_untracked();
        let making_first = first_from_now_on.get_untracked();

        asked(refused, async move {
            match service::answer_with_totp(&typed).await {
                Ok(account) => {
                    if making_first {
                        _ = service::choose_preferred_way(PreferredWay::Chosen(
                            SecondStepWay::Totp,
                        ))
                        .await;
                    }
                    session.signed_in(account);
                    Ok(())
                }
                Err(failed) => {
                    code.set(String::new());
                    Err(ran_out_or(page, failed))
                }
            }
        })
    });

    Effect::new(move |_| {
        if is_whole(&code.get()) {
            answering.start(());
        }
    });

    (
        html::h1().child(speech::said("sign-in-heading")),
        owing_as(page),
        form(
            move || answering.start(()),
            (
                code_boxes(speech::said("second-step-code"), code),
                first_from_now_on_box(SecondStepWay::Totp, first_from_now_on),
                move || refused.get().map(problem::problem),
                submitting(speech::said("second-step-confirm"), answering, ()),
            ),
        ),
        another_way(page),
    )
}

/// Asks for a touch of one of the account's keys; the browser's own dialog opens only once asked for, so one who meant another way is not interrupted.
fn key_step_form(page: Page) -> impl IntoView {
    let session = expect_context::<CurrentSession>();
    let refused = RwSignal::new(None::<ApiError>);
    let said = RwSignal::new(None::<&'static str>);
    let first_from_now_on = RwSignal::new(false);
    let answering = Act::new(move |(), _: Here| {
        said.set(None);
        let making_first = first_from_now_on.get_untracked();

        asked(refused, async move {
            let asked = service::begin_answering_with_key()
                .await
                .map_err(|failed| ran_out_or(page, failed))?;
            let touch = match authenticator::touched(&asked).await {
                Ok(touch) => touch,
                Err(NoKey::Cancelled | NoKey::HeldAlready) => {
                    said.set(Some("second-step-key-not-touched"));
                    return Ok(());
                }
                Err(NoKey::Failed) => {
                    said.set(Some("second-step-key-failed"));
                    return Ok(());
                }
            };
            let account = service::answer_with_key(touch)
                .await
                .map_err(|failed| ran_out_or(page, failed))?;
            if making_first {
                _ = service::choose_preferred_way(PreferredWay::Chosen(SecondStepWay::Key)).await;
            }
            session.signed_in(account);

            Ok(())
        })
    });

    (
        html::h1().child(speech::said("sign-in-heading")),
        owing_as(page),
        html::section().class("key-asked").child((
            icons::mark(Icon::Key),
            html::p().child(speech::said("second-step-key-touch")),
            move || {
                said.get().map(|message| {
                    html::p()
                        .class("hint")
                        .role("status")
                        .child(speech::said(message))
                })
            },
            move || refused.get().map(problem::problem),
            html::button()
                .r#type("button")
                .class("primary")
                .attr("aria-busy", move || answering.busy())
                .on(ev::click, move |_| answering.start(()))
                .child(speech::said("second-step-key-use")),
            first_from_now_on_box(SecondStepWay::Key, first_from_now_on),
        )),
        another_way(page),
    )
}

/// Offers to ask this way first from now on, where another is asked first; the choice is kept only once the step succeeds.
fn first_from_now_on_box(way: SecondStepWay, checked: RwSignal<bool>) -> impl IntoView {
    owed().is_some_and(|owed| owed.asked_way != way).then(|| {
        html::label().class("first-from-now-on").child((
            html::input()
                .r#type("checkbox")
                .prop("checked", move || checked.get())
                .on(ev::change, move |event| {
                    checked.set(event_target_checked(&event))
                }),
            html::span().child(speech::said("second-step-first-from-now-on")),
        ))
    })
}

fn another_way(page: Page) -> impl IntoView {
    turning(speech::said("second-step-another-way"), move || {
        page.turn_to(Form::AnotherWay, None)
    })
}

/// Every way that answers the account's second step, the one asked first among them, recovery codes last.
fn another_way_form(page: Page) -> impl IntoView {
    let owed = owed();
    let way = move |form: Form, title: &'static str, hint: &'static str, first: bool| {
        html::li().child(
            html::button()
                .r#type("button")
                .class("way")
                .on(ev::click, move |_| page.turn_to(form, None))
                .child((
                    html::span().class("title").child(speech::said(title)),
                    html::span().class("hint").child(speech::said(if first {
                        "another-way-asked-first"
                    } else {
                        hint
                    })),
                )),
        )
    };
    let asked_way = owed.map(|owed| owed.asked_way);

    (
        html::h1().child(speech::said("another-way-heading")),
        owing_as(page),
        html::p()
            .class("hint")
            .child(speech::said("another-way-hint")),
        html::ul().class("other-ways").child((
            owed.is_some_and(|owed| owed.answered_by_key).then(|| {
                way(
                    Form::KeyStep,
                    "another-way-key",
                    "another-way-key-hint",
                    asked_way == Some(SecondStepWay::Key),
                )
            }),
            owed.is_none_or(|owed| owed.answered_by_totp).then(|| {
                way(
                    Form::SecondStep,
                    "another-way-app",
                    "another-way-app-hint",
                    asked_way == Some(SecondStepWay::Totp),
                )
            }),
            way(
                Form::RecoveryStep,
                "another-way-recovery",
                "another-way-recovery-hint",
                false,
            ),
        )),
    )
}

/// The way the second step is asked first, as the server told it.
fn asked_form(account: &SignedInAccount) -> Form {
    match account.second_factor {
        Some(factor) if factor.asked_way == SecondStepWay::Key => Form::KeyStep,
        _ => Form::SecondStep,
    }
}

/// The second step the session owes, as the account told it.
fn owed() -> Option<SecondFactorInUse> {
    match expect_context::<CurrentSession>().state.get_untracked() {
        SignInState::OwingSecondFactor(account) => account.second_factor,
        _ => None,
    }
}

fn recovery_step_form(page: Page) -> impl IntoView {
    let session = expect_context::<CurrentSession>();
    let typed = RwSignal::new(String::new());
    let refused = RwSignal::new(None::<ApiError>);
    let answering = Act::new(move |(), _: Here| {
        let given = typed.get_untracked();

        asked(refused, async move {
            let account = service::answer_with_recovery_code(&given)
                .await
                .map_err(|failed| ran_out_or(page, failed))?;
            session.answered_by_recovery_code(account);

            Ok(())
        })
    });

    (
        html::h1().child(speech::said("sign-in-heading")),
        owing_as(page),
        form(
            move || answering.start(()),
            (
                html::label().class("field").child((
                    html::span().child(speech::said("second-step-recovery-code")),
                    html::input()
                        .r#type("text")
                        .class("recovery-code")
                        .attr("autocomplete", "off")
                        .attr("spellcheck", "false")
                        .attr("autocapitalize", "off")
                        .required(true)
                        .prop("value", move || typed.get())
                        .on(ev::input, move |event| {
                            typed.set(event_target_value(&event))
                        }),
                )),
                html::small()
                    .class("hint")
                    .child(speech::said("second-step-recovery-hint")),
                move || refused.get().map(problem::problem),
                submitting(speech::said("second-step-confirm"), answering, ()),
            ),
        ),
        another_way(page),
        html::p()
            .class("hint")
            .child(speech::said("second-step-no-codes")),
    )
}

fn ran_out_or(page: Page, failed: ApiError) -> ApiError {
    if matches!(&failed, ApiError::Rejected(refusal) if refusal.kind.as_deref() == Some(refused::NOT_SIGNED_IN))
    {
        page.turn_to(Form::SignIn, Some("second-step-ran-out"));
    }

    failed
}

fn owing_as(page: Page) -> impl IntoView {
    let session = expect_context::<CurrentSession>();

    html::p().class("sent-to").child((
        move || page.email.get(),
        " · ",
        html::button()
            .r#type("button")
            .class("turning inline")
            .on(ev::click, move |_| {
                session.sign_out();
                page.turn_to(Form::SignIn, None);
            })
            .child(speech::said("second-step-not-you")),
    ))
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

pub(crate) fn code_boxes(
    label: impl Fn() -> String + Send + Sync + 'static,
    code: RwSignal<String>,
) -> impl IntoView {
    let focused = RwSignal::new(false);

    html::label().class("field").child((
        html::span().child(label),
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

pub(crate) fn is_whole(code: &str) -> bool {
    code.len() == CODE_LENGTH
}

fn asked(
    refused: RwSignal<Option<ApiError>>,
    answer: impl Future<Output = Result<(), ApiError>> + 'static,
) -> impl Future<Output = ()> + 'static {
    refused.set(None);

    async move {
        if let Err(failed) = answer.await {
            refused.set(Some(failed));
        }
    }
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

fn submitting<T: Clone + PartialEq + Send + Sync + 'static>(
    label: impl Fn() -> String + Send + Sync + 'static,
    act: Act<T>,
    about: T,
) -> impl IntoView {
    html::button()
        .r#type("submit")
        .class("primary")
        .attr("aria-busy", move || act.busy_on(&about))
        .child(label)
}

fn turning_asking<T: Clone + PartialEq + Send + Sync + 'static>(
    label: impl Fn() -> String + Send + Sync + 'static,
    act: Act<T>,
    about: T,
) -> impl IntoView {
    let pressed = about.clone();

    html::button()
        .r#type("button")
        .class("turning")
        .attr("aria-busy", move || act.busy_on(&pressed))
        .on(ev::click, move |_| act.start(about.clone()))
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
        // expect: spaces and letters are dropped from a code
        assert_eq!(digits_of("123 456"), "123456");
        assert_eq!(digits_of("12ab34"), "1234");
        // and: digits past the sixth
        assert_eq!(
            digits_of("1234567"),
            "123456",
            "a code is six digits, and a seventh is a slip of the finger"
        );
    }

    #[test]
    fn a_code_is_whole_at_six_digits_and_not_before() {
        // expect: five digits are not a whole code and six are
        assert!(!is_whole("12345"));
        assert!(is_whole("123456"));
    }
}
