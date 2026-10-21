use gloo_net::http::Request;
use statistics_contract::WordCountDTO;

use crate::http::{ApiError, parsed, sent};
use crate::projects::model::ProjectId;
use crate::statistics::model::SceneWordCount;

const WORD_COUNTS: &str = "/api/statistics/word-counts";
const SUBJECT: &str = "word-counts";

pub async fn word_counts(project: &ProjectId) -> Result<Vec<SceneWordCount>, ApiError> {
    let response = sent(Request::get(&format!("{WORD_COUNTS}?project={project}"))).await?;
    let counted: Vec<WordCountDTO> = parsed(response, SUBJECT).await?;

    Ok(counted.into_iter().map(SceneWordCount::from).collect())
}
