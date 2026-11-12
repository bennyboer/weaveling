use std::fmt::{self, Display, Formatter};

use eventsourcing::{
    AccountId, Agent, Aggregate, AggregateContext, AggregateId, AggregateType, Event,
    EventMetadata, EventName, Version,
};
use projects_contract::context;
use thiserror::Error;
use time::OffsetDateTime;

use crate::id::AssetId;

pub const KIND: AggregateType = AggregateType::of("asset");

impl From<&AssetId> for AggregateId {
    fn from(id: &AssetId) -> Self {
        AggregateId::from(id.to_string())
    }
}

const UPLOADED: EventName = EventName::of("UPLOADED");
const DELETED: EventName = EventName::of("DELETED");
const SNAPSHOTTED: EventName = EventName::of("SNAPSHOTTED");

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectLink(String);

/// Whose an asset is: a project's, seen by those who may read it, or an account's, seen by those who share a project with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Owner {
    Project(ProjectLink),
    Account(AccountId),
}

/// What kind of image the original was, known by its bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Jpeg,
    Png,
    Gif,
    Webp,
}

/// An image as it was uploaded: its kind, how large it is, and the widths it was made into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub format: Format,
    pub width: u32,
    pub height: u32,
    pub original_size: u64,
    pub variants: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetCommand {
    Upload {
        owner: Owner,
        picture: Picture,
        total_size: u64,
    },
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetEvent {
    Uploaded {
        owner: Owner,
        picture: Picture,
        total_size: u64,
    },
    Deleted,
    Snapshotted {
        owner: Owner,
        picture: Picture,
        total_size: u64,
        uploaded_at: OffsetDateTime,
        deleted_at: Option<OffsetDateTime>,
    },
}

/// A file Weaveling keeps that is not text, an image for now: whose it is, the picture, and every byte kept for it, the original and its variants, counted against what the installation may hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    owner: Owner,
    picture: Picture,
    total_size: u64,
    uploaded_at: OffsetDateTime,
    deleted_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AssetError {
    #[error("an asset is uploaded once")]
    AlreadyUploaded,
    #[error("an asset must be uploaded before anything is done with it")]
    NotUploadedYet,
    #[error("the asset was deleted")]
    Deleted,
}

impl ProjectLink {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ProjectLink {
    fn from(given: &str) -> Self {
        Self(given.to_owned())
    }
}

impl Display for ProjectLink {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Format {
    pub fn content_type(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
        }
    }
}

impl Asset {
    pub fn owner(&self) -> &Owner {
        &self.owner
    }

    pub fn picture(&self) -> &Picture {
        &self.picture
    }

    pub fn total_size(&self) -> u64 {
        self.total_size
    }

    pub fn uploaded_at(&self) -> OffsetDateTime {
        self.uploaded_at
    }

    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    pub fn deleted_at(&self) -> Option<OffsetDateTime> {
        self.deleted_at
    }
}

impl Event for AssetEvent {
    fn name(&self) -> EventName {
        match self {
            Self::Uploaded { .. } => UPLOADED,
            Self::Deleted => DELETED,
            Self::Snapshotted { .. } => SNAPSHOTTED,
        }
    }

    fn version(&self) -> Version {
        Version::ZERO
    }

    fn is_snapshot(&self) -> bool {
        matches!(self, Self::Snapshotted { .. })
    }
}

impl Aggregate for Asset {
    type Command = AssetCommand;
    type Event = AssetEvent;
    type Error = AssetError;

    const KIND: AggregateType = KIND;

    fn context(&self) -> AggregateContext {
        match &self.owner {
            Owner::Project(project) => {
                AggregateContext::default().with(context::PROJECT, project.to_string())
            }
            Owner::Account(_) => AggregateContext::default(),
        }
    }

    fn begin(command: AssetCommand, _agent: &Agent) -> Result<Vec<AssetEvent>, AssetError> {
        match command {
            AssetCommand::Upload {
                owner,
                picture,
                total_size,
            } => Ok(vec![AssetEvent::Uploaded {
                owner,
                picture,
                total_size,
            }]),
            AssetCommand::Delete => Err(AssetError::NotUploadedYet),
        }
    }

    fn from_first(event: &AssetEvent, metadata: &EventMetadata) -> Option<Self> {
        match event {
            AssetEvent::Uploaded {
                owner,
                picture,
                total_size,
            } => Some(Self {
                owner: owner.clone(),
                picture: picture.clone(),
                total_size: *total_size,
                uploaded_at: metadata.occurred_at,
                deleted_at: None,
            }),
            AssetEvent::Snapshotted {
                owner,
                picture,
                total_size,
                uploaded_at,
                deleted_at,
            } => Some(Self {
                owner: owner.clone(),
                picture: picture.clone(),
                total_size: *total_size,
                uploaded_at: *uploaded_at,
                deleted_at: *deleted_at,
            }),
            AssetEvent::Deleted => None,
        }
    }

    fn decide(&self, command: AssetCommand, _agent: &Agent) -> Result<Vec<AssetEvent>, AssetError> {
        match command {
            AssetCommand::Delete if self.is_deleted() => Ok(Vec::new()),
            AssetCommand::Delete => Ok(vec![AssetEvent::Deleted]),
            AssetCommand::Upload { .. } if self.is_deleted() => Err(AssetError::Deleted),
            AssetCommand::Upload { .. } => Err(AssetError::AlreadyUploaded),
        }
    }

    fn apply(&mut self, event: &AssetEvent, metadata: &EventMetadata) {
        match event {
            AssetEvent::Uploaded { .. } => {}
            AssetEvent::Deleted => self.deleted_at = Some(metadata.occurred_at),
            AssetEvent::Snapshotted {
                owner,
                picture,
                total_size,
                uploaded_at,
                deleted_at,
            } => {
                self.owner = owner.clone();
                self.picture = picture.clone();
                self.total_size = *total_size;
                self.uploaded_at = *uploaded_at;
                self.deleted_at = *deleted_at;
            }
        }
    }

    fn snapshot(&self) -> AssetEvent {
        AssetEvent::Snapshotted {
            owner: self.owner.clone(),
            picture: self.picture.clone(),
            total_size: self.total_size,
            uploaded_at: self.uploaded_at,
            deleted_at: self.deleted_at,
        }
    }
}
