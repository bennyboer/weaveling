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

pub fn project_id(segment: &str) -> ProjectId {
    match segment.rsplit_once('-') {
        Some((_, trailing)) if trailing.contains('_') => ProjectId::from(trailing.to_owned()),
        _ => ProjectId::from(segment.to_owned()),
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
