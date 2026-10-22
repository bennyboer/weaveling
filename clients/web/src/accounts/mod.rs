mod service;
pub mod sign_in;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::{IntoView, ev, html};

use crate::speech::said;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedInAccount {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignInState {
    Unknown,
    NoAccounts,
    SignedOut,
    SignedIn(SignedInAccount),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Blank,
    SignIn,
    Work,
}

#[derive(Clone, Copy)]
pub struct CurrentSession {
    state: RwSignal<SignInState>,
}

impl CurrentSession {
    pub fn asked() -> Self {
        let state = RwSignal::new(SignInState::Unknown);
        spawn_local(async move {
            state.set(service::current().await.unwrap_or(SignInState::NoAccounts));
        });

        Self { state }
    }

    pub fn screen(self) -> Memo<Screen> {
        Memo::new(move |_| screen_for(&self.state.get()))
    }

    pub fn signed_in(self, account: SignedInAccount) {
        self.state.set(SignInState::SignedIn(account));
    }

    fn sign_out(self) {
        spawn_local(async move {
            if service::sign_out().await.is_ok() {
                self.state.set(SignInState::SignedOut);
            }
        });
    }
}

fn screen_for(state: &SignInState) -> Screen {
    match state {
        SignInState::Unknown => Screen::Blank,
        SignInState::SignedOut => Screen::SignIn,
        SignInState::NoAccounts | SignInState::SignedIn(_) => Screen::Work,
    }
}

pub fn account_in_masthead() -> impl IntoView {
    let session = expect_context::<CurrentSession>();

    move || match session.state.get() {
        SignInState::SignedIn(account) => Some(
            html::div().class("signed-in").child((
                html::span()
                    .class("account")
                    .attr("title", account.email)
                    .child(account.name),
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| session.sign_out())
                    .child(said("sign-out")),
            )),
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_shown_until_the_server_has_said_who_is_signed_in() {
        assert_eq!(screen_for(&SignInState::Unknown), Screen::Blank);
    }

    #[test]
    fn signed_out_on_a_server_asks_to_sign_in() {
        assert_eq!(screen_for(&SignInState::SignedOut), Screen::SignIn);
    }

    #[test]
    fn signed_in_or_without_accounts_shows_the_work() {
        assert_eq!(screen_for(&SignInState::NoAccounts), Screen::Work);
        assert_eq!(
            screen_for(&SignInState::SignedIn(SignedInAccount {
                name: "Ada".to_owned(),
                email: "ada@example.com".to_owned(),
            })),
            Screen::Work
        );
    }
}
