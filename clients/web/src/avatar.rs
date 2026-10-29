use leptos::prelude::*;
use leptos::{IntoView, html};

const TINTS: usize = 5;

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

pub fn avatar(account: &str, name: Option<&str>) -> impl IntoView + use<> {
    html::span()
        .class(format!("avatar {}", tint_of(account)))
        .attr("aria-hidden", "true")
        .child(name.map(initial_of).unwrap_or_else(|| "?".to_owned()))
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

    #[test]
    fn an_account_keeps_its_tint_and_accounts_are_spread_over_the_tints() {
        assert_eq!(tint_of("account_1"), tint_of("account_1"));

        let tinted: Vec<String> = (1..=10)
            .map(|account| tint_of(&format!("account_{account}")))
            .collect();
        for tint in 0..TINTS {
            assert!(
                tinted.contains(&format!("tint-{tint}")),
                "tint-{tint} among {tinted:?}"
            );
        }
    }
}
