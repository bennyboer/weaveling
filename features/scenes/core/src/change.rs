use crate::{IdeaLink, ProjectLink, SceneId, SceneTitle};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneChange {
    Created {
        scene: SceneId,
        project: ProjectLink,
    },
    Retitled {
        scene: SceneId,
        project: ProjectLink,
        title: SceneTitle,
        version: u64,
    },
    OpeningChanged {
        scene: SceneId,
        project: ProjectLink,
        opening: String,
    },
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
    WordsCounted {
        scene: SceneId,
        project: ProjectLink,
        words: usize,
    },
}

impl SceneChange {
    pub fn scene(&self) -> SceneId {
        match self {
            Self::IdeaLinked { scene, .. }
            | Self::IdeaUnlinked { scene, .. }
            | Self::Deleted { scene }
            | Self::Created { scene, .. }
            | Self::Retitled { scene, .. }
            | Self::OpeningChanged { scene, .. }
            | Self::WordsCounted { scene, .. } => *scene,
        }
    }
}
