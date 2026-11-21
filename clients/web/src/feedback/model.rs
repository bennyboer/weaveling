use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};

use feedback_contract::{
    AboutDTO, CommentBlockDTO, CommentRunDTO, FeedbackDTO, PendingDTO, ReadingCopyPlaceDTO,
};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::http::ApiError;
use crate::outline::model::SectionId;
use crate::readings::reading_copies::ReadingCopyId;
use crate::scenes::model::SceneId;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FeedbackId(String);

impl From<String> for FeedbackId {
    fn from(id: String) -> Self {
        Self(id)
    }
}

impl Display for FeedbackId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

/// What a piece of feedback is about: the thing it is given on, and where in it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum About {
    ReadingCopy {
        reading_copy: ReadingCopyId,
        place: ReadingCopyPlace,
    },
}

/// Where in a reading copy a piece of feedback is.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ReadingCopyPlace {
    Passage {
        scene: SceneId,
        block_path: Vec<u32>,
        from: u32,
        to: u32,
        quote: String,
    },
    Scene(SceneId),
    Section(SectionId),
    Whole,
}

/// What someone says in a piece of feedback.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CommentBlock {
    Paragraph(Vec<CommentRun>),
    List {
        ordered: bool,
        items: Vec<Vec<CommentRun>>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CommentRun {
    Text {
        text: String,
        emphasis: bool,
        strong: bool,
        link: Option<String>,
    },
    Break,
}

/// How much feedback given is not done yet in a project: on each scene, on it or a passage in it, and on each section itself.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pending {
    pub scenes: HashMap<SceneId, u32>,
    pub sections: HashMap<SectionId, u32>,
    /// All of it, on a whole copy too.
    pub total: u32,
}

impl From<PendingDTO> for Pending {
    fn from(dto: PendingDTO) -> Self {
        Self {
            scenes: dto
                .scenes
                .into_iter()
                .map(|(scene, count)| (SceneId::from(scene), count))
                .collect(),
            sections: dto
                .sections
                .into_iter()
                .map(|(section, count)| (SectionId::from(section), count))
                .collect(),
            total: dto.total,
        }
    }
}

/// A passage feedback is pending on, as its copy froze it: the scene, the path to its block, and the words it quotes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingPassage {
    pub scene: SceneId,
    pub block_path: Vec<u32>,
    pub quote: String,
}

impl PendingPassage {
    /// The passage a piece of feedback is about; none for one about a scene, a section or the whole.
    pub fn of(piece: &Feedback) -> Option<Self> {
        let About::ReadingCopy { place, .. } = &piece.about;
        match place {
            ReadingCopyPlace::Passage {
                scene,
                block_path,
                quote,
                ..
            } => Some(Self {
                scene: scene.clone(),
                block_path: block_path.clone(),
                quote: quote.clone(),
            }),
            _ => None,
        }
    }
}

/// How much feedback one subject holds, as you count it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FeedbackCount {
    /// The feedback submitted and not done yet.
    pub not_done: u32,
    /// Your own drafts on it.
    pub your_drafts: u32,
}

/// A piece of feedback, as whoever may see it finds it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Feedback {
    pub id: FeedbackId,
    pub about: About,
    pub comment: Vec<CommentBlock>,
    pub draft: bool,
    pub by_you: bool,
    /// Who wrote it, by name, where that is known.
    pub written_by: Option<String>,
    pub written_at: OffsetDateTime,
    pub submitted_at: Option<OffsetDateTime>,
    /// Whether what it says was changed after it was submitted.
    pub edited: bool,
    /// Whether the authors marked it done.
    pub done: bool,
}

fn moment(written: &str) -> Result<OffsetDateTime, ApiError> {
    OffsetDateTime::parse(written, &Rfc3339).map_err(|_| ApiError::Unexpected)
}

impl TryFrom<FeedbackDTO> for Feedback {
    type Error = ApiError;

    fn try_from(dto: FeedbackDTO) -> Result<Self, ApiError> {
        Ok(Self {
            id: FeedbackId::from(dto.id),
            about: About::from(dto.about),
            comment: dto.comment.into_iter().map(CommentBlock::from).collect(),
            draft: dto.draft,
            by_you: dto.by_you,
            written_by: dto.written_by,
            written_at: moment(&dto.written_at)?,
            submitted_at: dto.submitted_at.as_deref().map(moment).transpose()?,
            edited: dto.edited,
            done: dto.done,
        })
    }
}

impl From<AboutDTO> for About {
    fn from(dto: AboutDTO) -> Self {
        match dto {
            AboutDTO::ReadingCopy {
                reading_copy,
                place,
            } => Self::ReadingCopy {
                reading_copy: ReadingCopyId::from(reading_copy),
                place: ReadingCopyPlace::from(place),
            },
        }
    }
}

impl From<ReadingCopyPlaceDTO> for ReadingCopyPlace {
    fn from(dto: ReadingCopyPlaceDTO) -> Self {
        match dto {
            ReadingCopyPlaceDTO::Passage {
                scene,
                block_path,
                from,
                to,
                quote,
            } => Self::Passage {
                scene: SceneId::from(scene),
                block_path,
                from,
                to,
                quote,
            },
            ReadingCopyPlaceDTO::Scene { scene } => Self::Scene(SceneId::from(scene)),
            ReadingCopyPlaceDTO::Section { section } => Self::Section(SectionId::from(section)),
            ReadingCopyPlaceDTO::Whole => Self::Whole,
        }
    }
}

impl From<&About> for AboutDTO {
    fn from(about: &About) -> Self {
        match about {
            About::ReadingCopy {
                reading_copy,
                place,
            } => Self::ReadingCopy {
                reading_copy: reading_copy.to_string(),
                place: match place {
                    ReadingCopyPlace::Passage {
                        scene,
                        block_path,
                        from,
                        to,
                        quote,
                    } => ReadingCopyPlaceDTO::Passage {
                        scene: scene.to_string(),
                        block_path: block_path.clone(),
                        from: *from,
                        to: *to,
                        quote: quote.clone(),
                    },
                    ReadingCopyPlace::Scene(scene) => ReadingCopyPlaceDTO::Scene {
                        scene: scene.to_string(),
                    },
                    ReadingCopyPlace::Section(section) => ReadingCopyPlaceDTO::Section {
                        section: section.to_string(),
                    },
                    ReadingCopyPlace::Whole => ReadingCopyPlaceDTO::Whole,
                },
            },
        }
    }
}

impl From<CommentBlockDTO> for CommentBlock {
    fn from(dto: CommentBlockDTO) -> Self {
        match dto {
            CommentBlockDTO::Paragraph { runs } => {
                Self::Paragraph(runs.into_iter().map(CommentRun::from).collect())
            }
            CommentBlockDTO::List { ordered, items } => Self::List {
                ordered,
                items: items
                    .into_iter()
                    .map(|item| item.into_iter().map(CommentRun::from).collect())
                    .collect(),
            },
        }
    }
}

impl From<CommentRunDTO> for CommentRun {
    fn from(dto: CommentRunDTO) -> Self {
        match dto {
            CommentRunDTO::Text {
                text,
                emphasis,
                strong,
                link,
            } => Self::Text {
                text,
                emphasis,
                strong,
                link,
            },
            CommentRunDTO::Break => Self::Break,
        }
    }
}

impl From<&CommentBlock> for CommentBlockDTO {
    fn from(block: &CommentBlock) -> Self {
        match block {
            CommentBlock::Paragraph(runs) => Self::Paragraph {
                runs: runs.iter().map(CommentRunDTO::from).collect(),
            },
            CommentBlock::List { ordered, items } => Self::List {
                ordered: *ordered,
                items: items
                    .iter()
                    .map(|item| item.iter().map(CommentRunDTO::from).collect())
                    .collect(),
            },
        }
    }
}

impl From<&CommentRun> for CommentRunDTO {
    fn from(run: &CommentRun) -> Self {
        match run {
            CommentRun::Text {
                text,
                emphasis,
                strong,
                link,
            } => Self::Text {
                text: text.clone(),
                emphasis: *emphasis,
                strong: *strong,
                link: link.clone(),
            },
            CommentRun::Break => Self::Break,
        }
    }
}
