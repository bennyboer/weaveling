mod account_menu;
mod authenticator;
mod deletion_dialog;
pub mod deletion_pending;
pub mod invitation;
mod key_dialogs;
mod listed;
pub mod listing;
mod lock_dialog;
mod recovery_note;
mod role_dialog;
mod roster;
pub mod security;
pub mod service;
pub mod sign_in;
pub mod unreachable;
mod ways_in_dialog;
pub mod your_account;

pub use account_menu::account_in_masthead;
pub use recovery_note::recovery_note;

use accounts_contract::sign_up;
use leptos::prelude::*;
use leptos::task;
use time::OffsetDateTime;

use crate::http;
use crate::live;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedInAccount {
    pub account: String,
    pub name: String,
    pub email: String,
    pub operator: bool,
    pub operator_wanted: bool,
    pub second_factor: Option<SecondFactorInUse>,
    pub owes_second_factor: bool,
    pub deletion: Option<PendingDeletion>,
}

/// An account that is marked to be deleted on a certain day.
/// A session that signs in while the deletion is pending is told about it this way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingDeletion {
    pub until: OffsetDateTime,
    pub by_the_operator: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecondFactorInUse {
    pub since: OffsetDateTime,
    pub recovery_codes_left: usize,
    pub answered_by_totp: bool,
    pub answered_by_key: bool,
    pub preferred_way: PreferredWay,
    pub asked_way: SecondStepWay,
}

/// A way to answer the second sign-in step, not counting recovery codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecondStepWay {
    Totp,
    Key,
}

/// Which way of the second step the account wants to be asked first.
/// It is either one chosen way or the way that was used last.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreferredWay {
    Chosen(SecondStepWay),
    LastUsed,
}

/// A key of the account, as its Security page lists it.
/// It is either a passkey that signs in on its own or a key that answers the second step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldKey {
    pub id: String,
    pub name: String,
    pub since: OffsetDateTime,
    pub signs_in_alone: bool,
}

/// The ways an account can sign in, as the operator sees them.
/// The password is not listed, because the account always keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaysIn {
    pub keys: Vec<HeldKey>,
    pub totp_since: Option<OffsetDateTime>,
    pub recovery_codes_left: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignInState {
    Unknown,
    Unreachable,
    NoAccounts,
    SignedOut,
    OwingSecondFactor(SignedInAccount),
    DeletionPending(SignedInAccount),
    SignedIn(SignedInAccount),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SigningUp {
    Open,
    ByInvitation,
}

impl SigningUp {
    fn told(told: Option<&str>) -> Self {
        match told {
            None => Self::Open,
            Some(told) if told == sign_up::OPEN => Self::Open,
            Some(_) => Self::ByInvitation,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Blank,
    Unreachable,
    SignIn,
    DeletionPending,
    Work,
}

#[derive(Clone, Copy)]
pub struct CurrentSession {
    state: RwSignal<SignInState>,
    signing_up: RwSignal<SigningUp>,
    totp_offered: RwSignal<bool>,
    keys_offered: RwSignal<bool>,
    by_recovery_code: RwSignal<bool>,
}

impl CurrentSession {
    pub fn state(self) -> SignInState {
        self.state.get()
    }

    pub fn signing_up(self) -> SigningUp {
        self.signing_up.get()
    }

    /// Whether this Weaveling instance has no operator yet.
    /// Only a signed-in account is told this.
    pub fn operator_wanted(self) -> bool {
        matches!(self.state.get(), SignInState::SignedIn(account) if account.operator_wanted)
    }

    pub fn keys_offered(self) -> bool {
        self.keys_offered.get()
    }

    pub fn totp_offered(self) -> bool {
        self.totp_offered.get()
    }

    pub fn signed_in_by_recovery_code(self) -> bool {
        self.by_recovery_code.get()
    }

    pub fn asked() -> Self {
        let session = Self {
            state: RwSignal::new(SignInState::Unknown),
            signing_up: RwSignal::new(SigningUp::Open),
            totp_offered: RwSignal::new(false),
            keys_offered: RwSignal::new(false),
            by_recovery_code: RwSignal::new(false),
        };
        http::when_no_longer_signed_in(move || session.state.set(SignInState::SignedOut));
        live::when_session_ended(move || session.state.set(SignInState::SignedOut));
        session.ask();

        session
    }

    pub fn ask_again(self) {
        self.state.set(SignInState::Unknown);
        self.ask();
    }

    /// Asks the server again who is signed in.
    /// Until the answer comes, the last known state stays shown.
    pub fn look_again(self) {
        self.ask();
    }

    fn ask(self) {
        task::spawn_local(async move {
            match service::current().await {
                Ok((state, mode)) => {
                    let accounts = mode.accounts.as_ref();
                    self.signing_up.set(SigningUp::told(
                        accounts.map(|accounts| accounts.sign_up.as_str()),
                    ));
                    self.totp_offered
                        .set(accounts.is_some_and(|accounts| accounts.can_set_up_totp));
                    self.keys_offered
                        .set(accounts.is_some_and(|accounts| accounts.can_add_keys));
                    self.state.set(state);
                }
                Err(_) => self.state.set(SignInState::Unreachable),
            }
        });
    }

    pub fn screen(self) -> Memo<Screen> {
        Memo::new(move |_| screen_for(&self.state.get()))
    }

    pub fn signed_in(self, account: SignedInAccount) {
        if account.deletion.is_some() {
            self.state.set(SignInState::DeletionPending(account));
            return;
        }
        self.state.set(SignInState::SignedIn(account));
    }

    pub fn owes_second_factor(self, account: SignedInAccount) {
        self.state.set(SignInState::OwingSecondFactor(account));
    }

    pub fn answered_by_recovery_code(self, account: SignedInAccount) {
        self.by_recovery_code.set(true);
        self.signed_in(account);
    }

    pub fn recovery_code_noted(self) {
        self.by_recovery_code.set(false);
    }

    pub fn became_operator(self) {
        self.state.update(|state| {
            if let SignInState::SignedIn(account) = state {
                account.operator = true;
                account.operator_wanted = false;
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
        SignInState::SignedOut | SignInState::OwingSecondFactor(_) => Screen::SignIn,
        SignInState::DeletionPending(_) => Screen::DeletionPending,
        SignInState::NoAccounts | SignInState::SignedIn(_) => Screen::Work,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_shown_until_the_server_has_said_who_is_signed_in() {
        // expect: a blank screen while it is unknown who is signed in
        assert_eq!(screen_for(&SignInState::Unknown), Screen::Blank);
    }

    #[test]
    fn a_server_that_cannot_be_reached_says_so() {
        // expect: an unreachable server shows as unreachable
        assert_eq!(
            screen_for(&SignInState::Unreachable),
            Screen::Unreachable,
            "a server out of reach is not one without accounts"
        );
    }

    #[test]
    fn signed_out_on_a_server_asks_to_sign_in() {
        // expect: signed out shows the sign in
        assert_eq!(screen_for(&SignInState::SignedOut), Screen::SignIn);
    }

    #[test]
    fn signed_in_or_without_accounts_shows_the_work() {
        // expect: without accounts the work shows
        assert_eq!(screen_for(&SignInState::NoAccounts), Screen::Work);
        // and: signed in it does too
        assert_eq!(
            screen_for(&SignInState::SignedIn(SignedInAccount {
                account: "account_1".to_owned(),
                name: "Ada".to_owned(),
                email: "ada@example.com".to_owned(),
                operator: false,
                operator_wanted: false,
                second_factor: None,
                owes_second_factor: false,
                deletion: None,
            })),
            Screen::Work
        );
    }

    #[test]
    fn a_session_signed_in_while_deletion_is_pending_shows_only_the_deletion() {
        // expect: an account to be deleted shows its deletion, not the work
        assert_eq!(
            screen_for(&SignInState::DeletionPending(SignedInAccount {
                account: "account_1".to_owned(),
                name: "Ada".to_owned(),
                email: "ada@example.com".to_owned(),
                operator: false,
                operator_wanted: false,
                second_factor: None,
                owes_second_factor: false,
                deletion: Some(PendingDeletion {
                    until: time::macros::datetime!(2026-12-09 10:00 UTC),
                    by_the_operator: false,
                }),
            })),
            Screen::DeletionPending
        );
    }

    #[test]
    fn signing_up_is_open_unless_the_server_says_otherwise_and_closed_for_anything_unknown() {
        // expect: signing up is open when told so or told nothing
        assert_eq!(SigningUp::told(None), SigningUp::Open);
        assert_eq!(SigningUp::told(Some("open")), SigningUp::Open);
        // and: by invitation when told so
        assert_eq!(SigningUp::told(Some("invitation")), SigningUp::ByInvitation);
        // and: by invitation when told anything unknown
        assert_eq!(
            SigningUp::told(Some("by-application")),
            SigningUp::ByInvitation,
            "a way in this page does not know is no reason to offer the open one"
        );
    }
}
