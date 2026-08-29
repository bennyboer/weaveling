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

pub fn piece_segment(id: &PieceId, named: &str) -> String {
    let slug = slugify(named);

    if slug.is_empty() {
        id.to_string()
    } else {
        format!("{slug}-{id}")
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
