use crate::pieces::model::PieceId;
use crate::projects::model::ProjectId;

pub const WORKSPACE: &str = "/";

const SLUG_CHARS: usize = 60;

pub fn project(id: &ProjectId, named: &str) -> String {
    let slug = slugify(named);

    if slug.is_empty() {
        format!("/projects/{id}")
    } else {
        format!("/projects/{slug}-{id}")
    }
}

pub fn board(project: &str) -> String {
    format!("/projects/{project}")
}

pub fn outline(project: &str) -> String {
    format!("/projects/{project}/outline")
}

pub fn pool(project: &str) -> String {
    format!("/projects/{project}/pieces")
}

pub fn piece_segment(id: &PieceId, named: &str) -> String {
    let slug = slugify(named);

    if slug.is_empty() {
        id.to_string()
    } else {
        format!("{slug}-{id}")
    }
}

pub fn piece(project: &str, id: &PieceId, named: &str) -> String {
    format!("/projects/{project}/pieces/{}", piece_segment(id, named))
}

pub fn named(segment: &str) -> String {
    match segment.rsplit_once('-') {
        Some((slug, trailing)) if trailing.contains('_') && !slug.is_empty() => slug
            .split('-')
            .map(capitalised)
            .collect::<Vec<_>>()
            .join(" "),
        _ => "Untitled".to_owned(),
    }
}

fn capitalised(word: &str) -> String {
    let mut letters = word.chars();

    match letters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + letters.as_str(),
        None => String::new(),
    }
}

pub fn piece_id(segment: &str) -> PieceId {
    PieceId::from(trailing_id(segment))
}

pub fn project_id(segment: &str) -> ProjectId {
    ProjectId::from(trailing_id(segment))
}

fn trailing_id(segment: &str) -> String {
    match segment.rsplit_once('-') {
        Some((_, trailing)) if trailing.contains('_') => trailing.to_owned(),
        _ => segment.to_owned(),
    }
}

fn slugify(named: &str) -> String {
    let mut slug = String::new();
    let mut open = false;

    for letter in named.chars().take(SLUG_CHARS) {
        if letter.is_alphanumeric() {
            slug.extend(letter.to_lowercase());
            open = true;
        } else if open {
            slug.push('-');
            open = false;
        }
    }

    slug.trim_end_matches('-').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slugged_address_reads_back_as_a_title() {
        assert_eq!(named("the-silent-loom-project_034Jlq"), "The Silent Loom");
    }

    #[test]
    fn one_word_is_still_a_title() {
        assert_eq!(named("lautstille-project_034Jlq"), "Lautstille");
    }

    #[test]
    fn an_address_with_no_slug_has_no_name_to_show() {
        assert_eq!(named("project_034Jlq"), "Untitled");
    }

    #[test]
    fn an_id_that_never_had_an_underscore_is_not_mistaken_for_a_slug() {
        assert_eq!(named("the-silent-loom"), "Untitled");
    }

    #[test]
    fn a_title_survives_the_round_trip_through_an_address() {
        let id = ProjectId::from("project_034Jlq".to_owned());
        let address = project(&id, "The Silent Loom");
        let segment = address.trim_start_matches("/projects/");

        assert_eq!(named(segment), "The Silent Loom");
        assert_eq!(project_id(segment), id);
    }
}
