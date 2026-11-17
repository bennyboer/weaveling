use statistics_contract::WordCountDTO;
use statistics_core::WordCount;

pub(crate) fn to_dto(count: &WordCount) -> WordCountDTO {
    WordCountDTO {
        scene: count.scene.to_string(),
        words: count.words,
    }
}
