use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::{IntoView, ev, html, view};
use leptos_router::components::A;

use crate::accounts::{CurrentSession, SignInState, SignedInAccount};
use crate::assets::service as assets;
use crate::avatar;
use crate::route;
use crate::speech;
use crate::speech::Speech;

/// The width asked for the picture in the masthead, which shows it 32 pixels wide.
const MASTHEAD_PICTURE: u32 = 96;
pub fn account_in_masthead() -> impl IntoView {
    let session = expect_context::<CurrentSession>();
    let open = RwSignal::new(false);

    let clicking_away = window_event_listener(ev::click, move |_| open.set(false));
    let escaping = window_event_listener(ev::keydown, move |event| {
        if event.key() == "Escape" {
            open.set(false);
        }
    });
    on_cleanup(move || {
        clicking_away.remove();
        escaping.remove();
    });

    move || match session.state.get() {
        SignInState::SignedIn(account) => Some(account_menu(session, account, open)),
        _ => None,
    }
}

fn account_menu(
    session: CurrentSession,
    account: SignedInAccount,
    open: RwSignal<bool>,
) -> impl IntoView {
    let speech = expect_context::<Speech>();
    let named = account.name.clone();
    let labelled = move || {
        let mut args = FluentArgs::new();
        args.set("name", named.clone());
        speech.say_with("account-menu", &args)
    };

    html::div()
        .class("account")
        .on(ev::click, |event| event.stop_propagation())
        .child((
            html::button()
                .r#type("button")
                .class(format!(
                    "account-button {}",
                    avatar::tint_of(&account.account)
                ))
                .attr("aria-label", labelled)
                .attr("aria-haspopup", "menu")
                .attr("aria-expanded", move || open.get().to_string())
                .on(ev::click, move |_| open.update(|open| *open = !*open))
                .child(match &account.picture {
                    Some(picture) => html::img()
                        .class("picture")
                        .src(assets::content(picture, MASTHEAD_PICTURE))
                        .alt("")
                        .into_any(),
                    None => avatar::initial_of(&account.name).into_any(),
                }),
            move || {
                open.get().then(|| {
                    html::div().class("menu").role("menu").child((
                        html::p().class("account-name").child(account.name.clone()),
                        html::p()
                            .class("account-email")
                            .child(account.email.clone()),
                        html::hr(),
                        view! {
                            <A href=route::ACCOUNT attr:role="menuitem">
                                {speech::said("your-account-heading")}
                            </A>
                        },
                        (session.totp_offered()
                            || session.keys_offered()
                            || account.second_factor.is_some())
                        .then(|| {
                            view! {
                                <A href=route::SECURITY attr:role="menuitem">
                                    {speech::said("security-heading")}
                                </A>
                            }
                        }),
                        view! {
                            <A href=route::STORAGE attr:role="menuitem">
                                {speech::said("storage-heading")}
                            </A>
                        },
                        account.operator.then(|| {
                            view! {
                                <A href=route::INSTANCE_ACCOUNTS attr:role="menuitem">
                                    {speech::said("account-menu-operator")}
                                </A>
                            }
                        }),
                        (!account.operator && session.operator_wanted()).then(|| {
                            view! {
                                <A href=route::SETUP attr:role="menuitem">
                                    {speech::said("set-up-instead")}
                                </A>
                            }
                        }),
                        html::button()
                            .r#type("button")
                            .role("menuitem")
                            .on(ev::click, move |_| session.sign_out())
                            .child(speech::said("sign-out")),
                    ))
                })
            },
        ))
}
