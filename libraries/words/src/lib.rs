mod fitting;

pub use fitting::{TextRules, Unfit};

const OPENING: usize = 40;

pub fn opening_of(text: &str) -> String {
    let saying = text.split_whitespace().collect::<Vec<_>>().join(" ");

    match saying.char_indices().nth(OPENING) {
        None => saying,
        Some((cut, _)) => format!("{}…", saying[..cut].trim_end()),
    }
}

pub fn words_in(text: &str) -> usize {
    text.split_whitespace()
        .filter(|word| word.chars().any(char::is_alphanumeric))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_written_is_no_words() {
        assert_eq!(words_in(""), 0);
        assert_eq!(words_in(" \n\t "), 0);
    }

    #[test]
    fn words_are_what_stands_between_spaces() {
        assert_eq!(words_in("The loom stood silent."), 4);
        assert_eq!(words_in("  The   loom\nstood\tsilent  "), 4);
    }

    #[test]
    fn punctuation_standing_alone_is_not_a_word() {
        assert_eq!(words_in("Wait — then … go !"), 3);
        assert_eq!(words_in("„ “ - * …"), 0);
    }

    #[test]
    fn punctuation_inside_a_word_keeps_it_one_word() {
        assert_eq!(words_in("well-known don't 3.5 e-mail"), 4);
        assert_eq!(words_in("first—second"), 1);
    }

    #[test]
    fn a_short_text_opens_with_all_of_itself() {
        assert_eq!(
            opening_of("The loom stood silent."),
            "The loom stood silent."
        );
        assert_eq!(opening_of(""), "");
    }

    #[test]
    fn an_opening_runs_the_text_together_on_single_spaces() {
        assert_eq!(
            opening_of(
                "  The loom

stood   silent.  "
            ),
            "The loom stood silent.",
            "paragraphs and runs of space would read as gaps in a scene's name"
        );
    }

    #[test]
    fn a_long_text_is_cut_after_forty_characters_and_says_so() {
        assert_eq!(
            opening_of("The loom stood silent in the grey light of morning while she counted."),
            "The loom stood silent in the grey light…"
        );
    }

    #[test]
    fn an_opening_is_cut_between_characters_not_inside_one() {
        let opening = opening_of(&"ä".repeat(60));

        assert_eq!(opening, format!("{}…", "ä".repeat(40)));
    }

    #[test]
    fn letters_and_digits_of_any_script_make_words() {
        assert_eq!(words_in("Zählen: drei, 3 und mehr"), 5);
        assert_eq!(words_in("Ελληνικά слова 漢字"), 3);
        assert_eq!(words_in("1984"), 1);
    }
}
