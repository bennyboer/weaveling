use leptos::prelude::*;
use leptos::{IntoView, html};

use crate::assets::service as assets;

const TINTS: usize = 5;

/// The width asked for a picture in an avatar.
/// Avatars are at most 40 pixels wide, so this stays sharp on screens with twice the pixel density.
const PICTURE: u32 = 96;

pub fn initial_of(name: &str) -> String {
    name.chars()
        .find(|sign| sign.is_alphanumeric())
        .map(|first| first.to_uppercase().collect())
        .unwrap_or_default()
}

pub fn tint_of(account: &str) -> String {
    let summed = account.bytes().fold(0_usize, |sum, byte| {
        sum.wrapping_mul(31).wrapping_add(usize::from(byte))
    });

    format!("tint-{}", summed % TINTS)
}

/// Shows an account by its picture, or by the first letter of its name where it has none.
/// The letter stands on the account's own tint.
/// The avatar is only decoration, as the name is always written beside it.
pub fn avatar(account: &str, name: Option<&str>, picture: Option<&str>) -> impl IntoView + use<> {
    match picture {
        Some(picture) => html::img()
            .class("avatar")
            .src(assets::content(picture, PICTURE))
            .alt("")
            .attr("aria-hidden", "true")
            .into_any(),
        None => html::span()
            .class(format!("avatar {}", tint_of(account)))
            .attr("aria-hidden", "true")
            .child(name.map(initial_of).unwrap_or_else(|| "?".to_owned()))
            .into_any(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_account_is_shown_by_the_first_letter_of_its_name_in_capitals() {
        // expect: the first letter in capitals, whatever its alphabet
        assert_eq!(initial_of("Bennybär"), "B");
        assert_eq!(initial_of("ådne"), "Å");
    }

    #[test]
    fn what_comes_before_the_first_letter_is_passed_over() {
        // expect: quotes and spaces before the name give no initial
        assert_eq!(initial_of("\u{201e}Ada\u{201c}"), "A");
        assert_eq!(initial_of("  ada"), "A");
    }

    #[test]
    fn an_account_keeps_its_tint_and_accounts_are_spread_over_the_tints() {
        // expect: an account is tinted alike each time
        assert_eq!(tint_of("account_1"), tint_of("account_1"));

        // when: ten accounts are tinted
        let tinted: Vec<String> = (1..=10)
            .map(|account| tint_of(&format!("account_{account}")))
            .collect();

        // then: every tint is among them
        for tint in 0..TINTS {
            assert!(
                tinted.contains(&format!("tint-{tint}")),
                "tint-{tint} among {tinted:?}"
            );
        }
    }
}
