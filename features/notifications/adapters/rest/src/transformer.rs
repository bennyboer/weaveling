use clock::text;
use notifications_contract::{AboutDTO, NoticeDTO, role};
use notifications_core::{About, Names, NoticeSummary, Role};
use std::collections::BTreeMap;

pub(crate) fn to_dto(summary: &NoticeSummary, names: &Names) -> NoticeDTO {
    NoticeDTO {
        names: named_in(&summary.about, names),
        id: summary.id.to_string(),
        version: summary.version.count(),
        noted_at: text::serialize(summary.noted_at),
        about: match &summary.about {
            About::RoleChanged { project, role, by } => AboutDTO::RoleChanged {
                project: project.to_string(),
                role: named(*role).to_owned(),
                by: by.as_ref().map(ToString::to_string),
            },
        },
    }
}

fn named_in(about: &About, names: &Names) -> BTreeMap<String, String> {
    let project = about.project().and_then(|project| {
        names
            .projects
            .get(project)
            .map(|name| (project.to_string(), name.clone()))
    });
    let people = about.accounts().into_iter().filter_map(|account| {
        names
            .people
            .get(account)
            .map(|name| (account.to_string(), name.clone()))
    });

    project.into_iter().chain(people).collect()
}

fn named(given: Role) -> &'static str {
    match given {
        Role::Owner => role::OWNER,
        Role::Author => role::AUTHOR,
        Role::Reader => role::READER,
    }
}
