use crate::scenes::model::SceneId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneWordCount {
    pub scene: SceneId,
    pub words: usize,
}
