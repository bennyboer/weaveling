use std::cell::OnceCell;

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use leptos::logging::warn;
use unic_langid::LanguageIdentifier;

use crate::speech::language::Language;

thread_local! {
    static CATALOGUES: [OnceCell<FluentBundle<FluentResource>>; Language::EVERY.len()] =
        Default::default();
}

pub fn worded(language: Language, message: &str, args: Option<&FluentArgs>) -> String {
    in_the_catalogue_of(language, |catalogue| {
        let Some(pattern) = catalogue
            .get_message(message)
            .and_then(|found| found.value())
        else {
            return message.to_owned();
        };
        let mut errors = Vec::new();
        let worded = catalogue
            .format_pattern(pattern, args, &mut errors)
            .into_owned();
        if !errors.is_empty() {
            warn!("{message} in {language:?} is worded with errors: {errors:?}");
        }

        worded
    })
}

pub fn knows(language: Language, message: &str) -> bool {
    in_the_catalogue_of(language, |catalogue| catalogue.has_message(message))
}

fn in_the_catalogue_of<T>(
    language: Language,
    read: impl FnOnce(&FluentBundle<FluentResource>) -> T,
) -> T {
    CATALOGUES.with(|catalogues| {
        read(catalogues[language as usize].get_or_init(|| catalogue_of(language)))
    })
}

fn catalogue_of(language: Language) -> FluentBundle<FluentResource> {
    let tag: LanguageIdentifier = language.tag().parse().unwrap_or_default();
    let mut catalogue = FluentBundle::new(vec![tag]);
    catalogue.set_use_isolating(false);
    let source = FluentResource::try_new(source_of(language).to_owned())
        .unwrap_or_else(|(partly, _)| partly);
    _ = catalogue.add_resource(source);

    catalogue
}

fn source_of(language: Language) -> &'static str {
    match language {
        Language::English => include_str!("../../catalogue/en.ftl"),
        Language::German => include_str!("../../catalogue/de.ftl"),
        Language::Spanish => include_str!("../../catalogue/es.ftl"),
        Language::French => include_str!("../../catalogue/fr.ftl"),
        Language::Portuguese => include_str!("../../catalogue/pt-BR.ftl"),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use fluent_syntax::ast::{Entry, Expression, InlineExpression, Pattern, PatternElement};
    use fluent_syntax::parser;

    use super::*;

    fn messages_of(language: Language) -> Vec<(String, BTreeSet<String>)> {
        let parsed = parser::parse(source_of(language))
            .unwrap_or_else(|(_, errors)| panic!("{language:?} does not parse: {errors:?}"));

        parsed
            .body
            .into_iter()
            .filter_map(|entry| match entry {
                Entry::Message(message) => Some(message),
                _ => None,
            })
            .map(|message| {
                let mut asked_for = BTreeSet::new();
                if let Some(pattern) = &message.value {
                    gather(pattern, &mut asked_for);
                }

                (message.id.name.to_owned(), asked_for)
            })
            .collect()
    }

    fn gather(pattern: &Pattern<&str>, asked_for: &mut BTreeSet<String>) {
        for element in &pattern.elements {
            match element {
                PatternElement::Placeable {
                    expression: Expression::Inline(inline),
                } => asked(inline, asked_for),
                PatternElement::Placeable {
                    expression: Expression::Select { selector, variants },
                } => {
                    asked(selector, asked_for);
                    for variant in variants {
                        gather(&variant.value, asked_for);
                    }
                }
                PatternElement::TextElement { .. } => {}
            }
        }
    }

    fn asked(inline: &InlineExpression<&str>, asked_for: &mut BTreeSet<String>) {
        if let InlineExpression::VariableReference { id } = inline {
            asked_for.insert(id.name.to_owned());
        }
    }

    #[test]
    fn the_names_a_message_asks_for_are_found_in_its_choices_too() {
        let parsed = parser::parse(
            "chosen = { $count ->
    [one] { $name }
   *[other] { $place }
}
",
        )
        .unwrap_or_else(|(_, errors)| panic!("{errors:?}"));
        let Some(Entry::Message(message)) = parsed.body.first() else {
            panic!("no message");
        };
        let mut asked_for = BTreeSet::new();

        gather(message.value.as_ref().unwrap(), &mut asked_for);

        assert_eq!(
            asked_for,
            BTreeSet::from(["count".to_owned(), "name".to_owned(), "place".to_owned()])
        );
    }

    #[test]
    fn no_language_words_a_message_twice() {
        for language in Language::EVERY {
            let mut seen = BTreeSet::new();

            for (message, _) in messages_of(language) {
                assert!(
                    seen.insert(message.clone()),
                    "{language:?} words {message} twice"
                );
            }
        }
    }

    #[test]
    fn every_language_words_every_message_english_does_asking_for_the_same() {
        let english: BTreeSet<_> = messages_of(Language::English).into_iter().collect();

        for language in Language::EVERY {
            let worded: BTreeSet<_> = messages_of(language).into_iter().collect();

            assert_eq!(worded, english, "{language:?} differs from English");
        }
    }

    #[test]
    fn a_message_is_worded_in_the_language_asked_for() {
        assert_eq!(
            worded(Language::German, "workspace-heading", None),
            "Deine Projekte"
        );
        assert_eq!(
            worded(Language::English, "workspace-heading", None),
            "Your projects"
        );
    }

    #[test]
    fn a_name_is_set_into_its_message_without_marks_around_it() {
        let mut args = FluentArgs::new();
        args.set("name", "The loom");

        assert_eq!(
            worded(Language::German, "delete-project-fate", Some(&args)),
            "„The loom“ ist dann für immer weg. Löschen lässt sich nicht rückgängig machen."
        );
    }

    #[test]
    fn only_the_language_spoken_is_read() {
        worded(Language::German, "workspace-heading", None);

        CATALOGUES.with(|catalogues| {
            for language in Language::EVERY {
                assert_eq!(
                    catalogues[language as usize].get().is_some(),
                    language == Language::German,
                    "{language:?}"
                );
            }
        });
    }

    #[test]
    fn a_count_takes_the_form_its_language_gives_it() {
        let rewriting = |done: u32, total: u32| {
            let mut args = FluentArgs::new();
            args.set("done", done);
            args.set("total", total);

            worded(Language::German, "restyling-running", Some(&args))
        };

        assert_eq!(
            rewriting(0, 1),
            "Die Anführungszeichen werden neu gesetzt: 0 von 1 Szene."
        );
        assert_eq!(
            rewriting(1, 3),
            "Die Anführungszeichen werden neu gesetzt: 1 von 3 Szenen."
        );
    }

    #[test]
    fn every_refusal_the_server_names_is_worded() {
        let every = projects_contract::refused::EVERY
            .into_iter()
            .chain(accounts_contract::refused::EVERY)
            .chain(boards_contract::refused::EVERY)
            .chain(ideas_contract::refused::EVERY)
            .chain(outline_contract::refused::EVERY)
            .chain(scenes_contract::refused::EVERY);

        for kind in every {
            assert!(
                knows(Language::English, &format!("refused-{kind}")),
                "{kind} is not worded"
            );
        }
    }

    #[test]
    fn a_message_no_catalogue_has_is_not_known() {
        assert!(knows(Language::German, "workspace-heading"));
        assert!(!knows(Language::German, "nowhere-worded"));
    }

    #[test]
    fn a_moment_is_told_in_each_language_s_own_order() {
        let mut args = FluentArgs::new();
        args.set("day", 10);
        args.set("month", 10);
        args.set("year", "2026");
        args.set("hour", "09");
        args.set("minute", "01");
        let told = |language| worded(language, "moment", Some(&args));

        assert_eq!(told(Language::English), "10 Oct 2026, 09:01");
        assert_eq!(told(Language::German), "10. Okt. 2026, 09:01");
        assert_eq!(told(Language::Spanish), "10 oct 2026, 09:01");
        assert_eq!(told(Language::French), "10 oct. 2026 à 09:01");
        assert_eq!(told(Language::Portuguese), "10 de out. de 2026, 09:01");
    }

    #[test]
    fn a_message_no_catalogue_knows_shows_its_name() {
        assert_eq!(
            worded(Language::German, "nowhere-worded", None),
            "nowhere-worded"
        );
    }
}
