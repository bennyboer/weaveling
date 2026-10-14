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
    fn letters_and_digits_of_any_script_make_words() {
        assert_eq!(words_in("Zählen: drei, 3 und mehr"), 5);
        assert_eq!(words_in("Ελληνικά слова 漢字"), 3);
        assert_eq!(words_in("1984"), 1);
    }
}
