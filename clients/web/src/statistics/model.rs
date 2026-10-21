use statistics_contract::WordCountDTO;

use crate::scenes::model::SceneId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneWordCount {
    pub scene: SceneId,
    pub words: usize,
}

impl From<WordCountDTO> for SceneWordCount {
    fn from(dto: WordCountDTO) -> Self {
        Self {
            scene: SceneId::from(dto.scene),
            words: dto.words as usize,
        }
    }
}
