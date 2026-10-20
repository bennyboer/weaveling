use gloo_net::http::Request;
use statistics_contract::WordCountDTO;

use crate::http::{ApiError, parsed, sent};
use crate::projects::model::ProjectId;
use crate::scenes::model::SceneId;
use crate::statistics::model::SceneWordCount;

const WORD_COUNTS: &str = "/api/statistics/word-counts";
const SUBJECT: &str = "word-counts";

pub async fn word_counts(project: &ProjectId) -> Result<Vec<SceneWordCount>, ApiError> {
    let response = sent(Request::get(&format!("{WORD_COUNTS}?project={project}"))).await?;
    let counted: Vec<WordCountDTO> = parsed(response, SUBJECT).await?;

    Ok(counted.into_iter().map(as_word_count).collect())
}

fn as_word_count(dto: WordCountDTO) -> SceneWordCount {
    SceneWordCount {
        scene: SceneId::from(dto.scene),
        words: dto.words as usize,
    }
}
