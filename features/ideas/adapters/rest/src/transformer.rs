use ideas_contract::IdeaDTO;
use ideas_core::IdeaSummary;

pub(crate) fn to_dto(summary: &IdeaSummary) -> IdeaDTO {
    IdeaDTO {
        id: summary.id.to_string(),
        version: summary.version.count(),
        project: summary.project.to_string(),
        title: summary.title.to_string(),
    }
}
