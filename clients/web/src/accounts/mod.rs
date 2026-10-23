mod account_menu;
pub mod instance;
mod listed;
mod listing;
mod lock_dialog;
mod roster;
mod service;
pub mod sign_in;
pub mod unreachable;

pub use account_menu::account_in_masthead;

use leptos::prelude::*;
use leptos::task;

use crate::http;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedInAccount {
    pub name: String,
    pub email: String,
    pub operator: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignInState {
    Unknown,
    Unreachable,
    NoAccounts,
    SignedOut,
    SignedIn(SignedInAccount),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Blank,
    Unreachable,
    SignIn,
    Work,
}

#[derive(Clone, Copy)]
pub struct CurrentSession {
    state: RwSignal<SignInState>,
}

impl CurrentSession {
    pub fn asked() -> Self {
        let session = Self {
            state: RwSignal::new(SignInState::Unknown),
        };
        http::when_no_longer_signed_in(move || session.state.set(SignInState::SignedOut));
        session.ask();

        session
    }

    pub fn ask_again(self) {
        self.state.set(SignInState::Unknown);
        self.ask();
    }

    fn ask(self) {
        task::spawn_local(async move {
            self.state
                .set(service::current().await.unwrap_or(SignInState::Unreachable));
        });
    }

    pub fn screen(self) -> Memo<Screen> {
        Memo::new(move |_| screen_for(&self.state.get()))
    }

    pub fn signed_in(self, account: SignedInAccount) {
        self.state.set(SignInState::SignedIn(account));
    }

    fn became_operator(self) {
        self.state.update(|state| {
            if let SignInState::SignedIn(account) = state {
                account.operator = true;
            }
        });
    }

    fn sign_out(self) {
        task::spawn_local(async move {
            if service::sign_out().await.is_ok() {
                self.state.set(SignInState::SignedOut);
            }
        });
    }
}

fn screen_for(state: &SignInState) -> Screen {
    match state {
        SignInState::Unknown => Screen::Blank,
        SignInState::Unreachable => Screen::Unreachable,
        SignInState::SignedOut => Screen::SignIn,
        SignInState::NoAccounts | SignInState::SignedIn(_) => Screen::Work,
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
    fn a_server_that_cannot_be_reached_says_so() {
        assert_eq!(
            screen_for(&SignInState::Unreachable),
            Screen::Unreachable,
            "a server out of reach is not one without accounts"
        );
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
                operator: false,
            })),
            Screen::Work
        );
    }
}
