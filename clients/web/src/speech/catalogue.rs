use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use leptos::logging::warn;
use unic_langid::LanguageIdentifier;

use crate::speech::language::Language;

thread_local! {
    static CATALOGUES: Vec<FluentBundle<FluentResource>> =
        Language::EVERY.into_iter().map(catalogue_of).collect();
}

pub fn worded(language: Language, message: &str, args: Option<&FluentArgs>) -> String {
    CATALOGUES.with(|catalogues| {
        let catalogue = &catalogues[language as usize];
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

    use fluent_syntax::ast::{Entry, Expression, InlineExpression, PatternElement};
    use fluent_syntax::parser::parse;

    use super::*;

    fn messages_of(language: Language) -> Vec<(String, BTreeSet<String>)> {
        let parsed = parse(source_of(language))
            .unwrap_or_else(|(_, errors)| panic!("{language:?} does not parse: {errors:?}"));

        parsed
            .body
            .into_iter()
            .filter_map(|entry| match entry {
                Entry::Message(message) => Some(message),
                _ => None,
            })
            .map(|message| {
                let asked_for = message
                    .value
                    .map(|pattern| {
                        pattern
                            .elements
                            .into_iter()
                            .filter_map(|element| match element {
                                PatternElement::Placeable {
                                    expression:
                                        Expression::Inline(InlineExpression::VariableReference {
                                            id,
                                        }),
                                } => Some(id.name.to_owned()),
                                _ => None,
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                (message.id.name.to_owned(), asked_for)
            })
            .collect()
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
    fn a_message_no_catalogue_knows_shows_its_name() {
        assert_eq!(
            worded(Language::German, "nowhere-worded", None),
            "nowhere-worded"
        );
    }
}
