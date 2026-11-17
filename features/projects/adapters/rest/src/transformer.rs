use clock::text;
use projects_contract::{ProjectDTO, TrashedProjectDTO};
use projects_core::ProjectSummary;

pub(crate) fn to_dto(summary: &ProjectSummary) -> ProjectDTO {
    ProjectDTO {
        id: summary.id.to_string(),
        version: summary.version.count(),
        name: summary.name.to_string(),
        created_at: text::serialize(summary.created_at),
        updated_at: text::serialize(summary.updated_at),
    }
}

pub(crate) fn to_trashed_dto(summary: &ProjectSummary) -> Option<TrashedProjectDTO> {
    Some(TrashedProjectDTO {
        id: summary.id.to_string(),
        version: summary.version.count(),
        name: summary.name.to_string(),
        until: text::serialize(summary.trashed_until?),
    })
}
