use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::{IntoView, ev, html, view};
use leptos_router::components::A;

use crate::accounts::{CurrentSession, SignInState, SignedInAccount};
use crate::route;
use crate::speech::{Speech, said};

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
                .class("account-button")
                .attr("aria-label", labelled)
                .attr("aria-haspopup", "menu")
                .attr("aria-expanded", move || open.get().to_string())
                .on(ev::click, move |_| open.update(|open| *open = !*open))
                .child(initial_of(&account.name)),
            move || {
                open.get().then(|| {
                    html::div().class("menu").role("menu").child((
                        html::p().class("account-name").child(account.name.clone()),
                        html::p()
                            .class("account-email")
                            .child(account.email.clone()),
                        html::hr(),
                        account.operator.then(|| {
                            view! {
                                <A href=route::INSTANCE attr:role="menuitem">
                                    {said("account-menu-operator")}
                                </A>
                            }
                        }),
                        html::button()
                            .r#type("button")
                            .role("menuitem")
                            .on(ev::click, move |_| session.sign_out())
                            .child(said("sign-out")),
                    ))
                })
            },
        ))
}

fn initial_of(name: &str) -> String {
    name.chars()
        .find(|sign| sign.is_alphanumeric())
        .map(|first| first.to_uppercase().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_account_is_shown_by_the_first_letter_of_its_name_in_capitals() {
        assert_eq!(initial_of("Bennybär"), "B");
        assert_eq!(initial_of("ådne"), "Å");
    }

    #[test]
    fn what_comes_before_the_first_letter_is_passed_over() {
        assert_eq!(initial_of("\u{201e}Ada\u{201c}"), "A");
        assert_eq!(initial_of("  ada"), "A");
    }
}
