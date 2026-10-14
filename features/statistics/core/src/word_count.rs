use crate::link::SceneLink;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordCount {
    pub scene: SceneLink,
    pub words: u64,
}
