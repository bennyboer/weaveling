use scenes_contract::{QuotationRestylingDTO, SceneSummaryDTO};
use scenes_core::{QuotationRestylingProgress, SceneSummary};

pub(crate) fn to_restyling_dto(progress: QuotationRestylingProgress) -> QuotationRestylingDTO {
    QuotationRestylingDTO {
        change: progress.change,
        total: progress.total,
        done: progress.done.min(progress.total),
        finished: progress.finished,
    }
}

pub(crate) fn to_summary_dto(summary: &SceneSummary) -> SceneSummaryDTO {
    SceneSummaryDTO {
        id: summary.id.to_string(),
        project: summary.project.to_string(),
        title: summary.title.to_string(),
        opening: summary.opening.clone(),
    }
}
