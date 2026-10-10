use crate::{IdeaLink, SceneId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneChange {
    IdeaLinked {
        scene: SceneId,
        idea: IdeaLink,
        version: u64,
    },
    IdeaUnlinked {
        scene: SceneId,
        idea: IdeaLink,
        version: u64,
    },
    Deleted {
        scene: SceneId,
    },
}

impl SceneChange {
    pub fn scene(&self) -> SceneId {
        match self {
            Self::IdeaLinked { scene, .. }
            | Self::IdeaUnlinked { scene, .. }
            | Self::Deleted { scene } => *scene,
        }
    }
}
