use std::fmt::{self, Display, Formatter};

use indexmap::IndexMap;

use eventsourcing::{
    Agent, Aggregate, AggregateId, AggregateType, Event, EventMetadata, EventName, Version,
};
use thiserror::Error;

use crate::id::{OutlineId, SectionId};
use crate::title::SectionTitle;

pub const KIND: AggregateType = AggregateType::of("outline");

impl From<&OutlineId> for AggregateId {
    fn from(id: &OutlineId) -> Self {
        AggregateId::from(id.to_string())
    }
}

const STARTED: EventName = EventName::of("STARTED");
const SECTION_ADDED: EventName = EventName::of("SECTION_ADDED");
const SECTION_RETITLED: EventName = EventName::of("SECTION_RETITLED");
const SECTION_MOVED: EventName = EventName::of("SECTION_MOVED");
const SECTION_PROMOTED: EventName = EventName::of("SECTION_PROMOTED");
const SECTION_DEMOTED: EventName = EventName::of("SECTION_DEMOTED");
const SECTION_REMOVED: EventName = EventName::of("SECTION_REMOVED");
const PIECE_ATTACHED: EventName = EventName::of("PIECE_ATTACHED");
const PIECE_DETACHED: EventName = EventName::of("PIECE_DETACHED");
const SNAPSHOTTED: EventName = EventName::of("SNAPSHOTTED");

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectLink(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PieceLink(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedSection {
    pub section: SectionId,
    pub parent: Option<SectionId>,
    pub title: SectionTitle,
    pub pieces: Vec<PieceLink>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutlineCommand {
    Start {
        project: ProjectLink,
    },
    Add {
        section: SectionId,
        under: Option<SectionId>,
        after: Option<SectionId>,
        title: SectionTitle,
    },
    Retitle {
        section: SectionId,
        title: SectionTitle,
    },
    Move {
        section: SectionId,
        under: Option<SectionId>,
        after: Option<SectionId>,
    },
    Promote {
        section: SectionId,
    },
    Demote {
        section: SectionId,
    },
    Remove {
        section: SectionId,
    },
    AttachPiece {
        piece: PieceLink,
        to: SectionId,
        after: Option<PieceLink>,
    },
    DetachPiece {
        piece: PieceLink,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutlineEvent {
    Started {
        project: ProjectLink,
    },
    SectionAdded {
        section: SectionId,
        under: Option<SectionId>,
        after: Option<SectionId>,
        title: SectionTitle,
    },
    SectionRetitled {
        section: SectionId,
        title: SectionTitle,
    },
    SectionMoved {
        section: SectionId,
        under: Option<SectionId>,
        after: Option<SectionId>,
    },
    SectionPromoted {
        section: SectionId,
    },
    SectionDemoted {
        section: SectionId,
    },
    SectionRemoved {
        section: SectionId,
    },
    PieceAttached {
        piece: PieceLink,
        to: SectionId,
        after: Option<PieceLink>,
    },
    PieceDetached {
        piece: PieceLink,
    },
    Snapshotted {
        project: ProjectLink,
        sections: Vec<PlacedSection>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HeldSection {
    title: SectionTitle,
    parent: Option<SectionId>,
    children: Vec<SectionId>,
    pieces: Vec<PieceLink>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outline {
    project: ProjectLink,
    sections: IndexMap<SectionId, HeldSection>,
    children: Vec<SectionId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum OutlineError {
    #[error("an outline must be started before anything can be put in it")]
    NotStartedYet,
    #[error("an outline cannot be started twice")]
    AlreadyStarted,
    #[error("this outline has no such section")]
    NoSuchSection,
    #[error("this section is already in the outline")]
    AlreadyThere,
    #[error("a section cannot be moved inside itself")]
    WouldContainItself,
    #[error("there is nothing at that place to sit after")]
    NoSuchNeighbour,
    #[error("this piece is not in the outline")]
    NotAttached,
}

impl ProjectLink {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ProjectLink {
    fn from(given: String) -> Self {
        Self(given)
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

impl PieceLink {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for PieceLink {
    fn from(given: String) -> Self {
        Self(given)
    }
}

impl From<&str> for PieceLink {
    fn from(given: &str) -> Self {
        Self(given.to_owned())
    }
}

impl Display for PieceLink {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Outline {
    pub fn project(&self) -> &ProjectLink {
        &self.project
    }

    pub fn holds(&self, section: &SectionId) -> bool {
        self.sections.contains_key(section)
    }

    pub fn title_of(&self, section: &SectionId) -> Option<&SectionTitle> {
        self.sections.get(section).map(|held| &held.title)
    }

    pub fn parent_of(&self, section: &SectionId) -> Option<SectionId> {
        self.sections.get(section).and_then(|held| held.parent)
    }

    pub fn children_of(&self, section: &SectionId) -> Vec<SectionId> {
        self.sections
            .get(section)
            .map(|held| held.children.clone())
            .unwrap_or_default()
    }

    pub fn children(&self) -> &[SectionId] {
        &self.children
    }

    pub fn pieces_in(&self, section: &SectionId) -> Vec<PieceLink> {
        self.sections
            .get(section)
            .map(|held| held.pieces.clone())
            .unwrap_or_default()
    }

    pub fn section_holding(&self, piece: &PieceLink) -> Option<SectionId> {
        self.sections
            .iter()
            .find(|(_, held)| held.pieces.contains(piece))
            .map(|(section, _)| *section)
    }

    pub fn sections(&self) -> Vec<PlacedSection> {
        let mut walked = Vec::with_capacity(self.sections.len());

        for child in &self.children {
            self.walk(child, &mut walked);
        }

        walked
    }

    pub fn reading_order(&self) -> Vec<PieceLink> {
        self.sections()
            .into_iter()
            .flat_map(|placed| placed.pieces)
            .collect()
    }

    fn walk(&self, section: &SectionId, into: &mut Vec<PlacedSection>) {
        let Some(held) = self.sections.get(section) else {
            return;
        };

        into.push(PlacedSection {
            section: *section,
            parent: held.parent,
            title: held.title.clone(),
            pieces: held.pieces.clone(),
        });

        for child in &held.children {
            self.walk(child, into);
        }
    }

    fn siblings_of(&self, parent: Option<&SectionId>) -> &[SectionId] {
        match parent {
            None => &self.children,
            Some(parent) => self
                .sections
                .get(parent)
                .map(|held| held.children.as_slice())
                .unwrap_or_default(),
        }
    }

    fn rank_of(&self, section: &SectionId) -> Option<usize> {
        let among = self.siblings_of(self.parent_of(section).as_ref());

        among.iter().position(|held| held == section)
    }

    fn descends_from(&self, section: &SectionId, ancestor: &SectionId) -> bool {
        let mut walking = Some(*section);

        while let Some(here) = walking {
            if &here == ancestor {
                return true;
            }

            walking = self.parent_of(&here);
        }

        false
    }

    fn knows_where(
        &self,
        under: Option<&SectionId>,
        after: Option<&SectionId>,
    ) -> Result<(), OutlineError> {
        if under.is_some_and(|under| !self.holds(under)) {
            return Err(OutlineError::NoSuchSection);
        }

        match after {
            None => Ok(()),
            Some(after) => match self.siblings_of(under).contains(after) {
                true => Ok(()),
                false => Err(OutlineError::NoSuchNeighbour),
            },
        }
    }

    fn lift(&mut self, section: &SectionId) {
        let parent = self.parent_of(section);

        match parent {
            None => self.children.retain(|held| held != section),
            Some(parent) => {
                if let Some(held) = self.sections.get_mut(&parent) {
                    held.children.retain(|held| held != section);
                }
            }
        }
    }

    fn put(&mut self, section: &SectionId, under: Option<SectionId>, after: Option<&SectionId>) {
        self.lift(section);

        if let Some(held) = self.sections.get_mut(section) {
            held.parent = under;
        }

        let at = match after {
            None => 0,
            Some(after) => self
                .siblings_of(under.as_ref())
                .iter()
                .position(|held| held == after)
                .map_or(0, |nth| nth + 1),
        };

        match under {
            None => self.children.insert(at.min(self.children.len()), *section),
            Some(under) => {
                if let Some(held) = self.sections.get_mut(&under) {
                    let at = at.min(held.children.len());
                    held.children.insert(at, *section);
                }
            }
        }
    }

    fn last_child_of(&self, section: &SectionId) -> Option<SectionId> {
        self.sections
            .get(section)
            .and_then(|held| held.children.last().copied())
    }

    fn add(
        &mut self,
        section: &SectionId,
        under: Option<SectionId>,
        after: Option<&SectionId>,
        title: SectionTitle,
    ) {
        self.sections.insert(
            *section,
            HeldSection {
                title,
                parent: None,
                children: Vec::new(),
                pieces: Vec::new(),
            },
        );
        self.put(section, under, after);
    }

    fn retitle(&mut self, section: &SectionId, title: SectionTitle) {
        if let Some(held) = self.sections.get_mut(section) {
            held.title = title;
        }
    }

    fn promote(&mut self, section: &SectionId) {
        let Some(parent) = self.parent_of(section) else {
            return;
        };
        let Some(rank) = self.rank_of(section) else {
            return;
        };
        let adopted = self.siblings_of(Some(&parent))[rank + 1..].to_vec();
        let grandparent = self.parent_of(&parent);

        self.put(section, grandparent, Some(&parent));

        for each in &adopted {
            let last = self.last_child_of(section);
            self.put(each, Some(*section), last.as_ref());
        }
    }

    fn demote(&mut self, section: &SectionId) {
        let Some(rank) = self.rank_of(section) else {
            return;
        };

        if rank == 0 {
            return;
        }

        let elder = self.siblings_of(self.parent_of(section).as_ref())[rank - 1];
        let last = self.last_child_of(&elder);

        self.put(section, Some(elder), last.as_ref());
    }

    fn remove(&mut self, section: &SectionId) {
        let Some(rank) = self.rank_of(section) else {
            return;
        };
        let parent = self.parent_of(section);
        let orphans = self.children_of(section);

        self.lift(section);
        self.sections.shift_remove(section);

        for (at, orphan) in (rank..).zip(orphans) {
            if let Some(held) = self.sections.get_mut(&orphan) {
                held.parent = parent;
            }

            match &parent {
                None => {
                    let at = at.min(self.children.len());
                    self.children.insert(at, orphan);
                }
                Some(parent) => {
                    if let Some(held) = self.sections.get_mut(parent) {
                        let at = at.min(held.children.len());
                        held.children.insert(at, orphan);
                    }
                }
            }
        }
    }

    fn attach(&mut self, piece: &PieceLink, to: &SectionId, after: Option<&PieceLink>) {
        self.detach(piece);

        let Some(held) = self.sections.get_mut(to) else {
            return;
        };
        let at = match after {
            None => 0,
            Some(after) => held
                .pieces
                .iter()
                .position(|held| held == after)
                .map_or(0, |nth| nth + 1),
        };

        held.pieces.insert(at.min(held.pieces.len()), piece.clone());
    }

    fn detach(&mut self, piece: &PieceLink) {
        for held in self.sections.values_mut() {
            held.pieces.retain(|held| held != piece);
        }
    }

    fn holding(sections: &[PlacedSection]) -> (IndexMap<SectionId, HeldSection>, Vec<SectionId>) {
        let mut held = IndexMap::with_capacity(sections.len());
        let mut children = Vec::new();

        for placed in sections {
            held.insert(
                placed.section,
                HeldSection {
                    title: placed.title.clone(),
                    parent: placed.parent,
                    children: Vec::new(),
                    pieces: placed.pieces.clone(),
                },
            );
        }

        for placed in sections {
            match &placed.parent {
                None => children.push(placed.section),
                Some(parent) => {
                    if let Some(parent) = held.get_mut(parent) {
                        parent.children.push(placed.section);
                    }
                }
            }
        }

        (held, children)
    }
}

impl Event for OutlineEvent {
    fn name(&self) -> EventName {
        match self {
            Self::Started { .. } => STARTED,
            Self::SectionAdded { .. } => SECTION_ADDED,
            Self::SectionRetitled { .. } => SECTION_RETITLED,
            Self::SectionMoved { .. } => SECTION_MOVED,
            Self::SectionPromoted { .. } => SECTION_PROMOTED,
            Self::SectionDemoted { .. } => SECTION_DEMOTED,
            Self::SectionRemoved { .. } => SECTION_REMOVED,
            Self::PieceAttached { .. } => PIECE_ATTACHED,
            Self::PieceDetached { .. } => PIECE_DETACHED,
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

impl Aggregate for Outline {
    type Command = OutlineCommand;
    type Event = OutlineEvent;
    type Error = OutlineError;

    const KIND: AggregateType = KIND;

    fn begin(command: OutlineCommand, _agent: &Agent) -> Result<Vec<OutlineEvent>, OutlineError> {
        match command {
            OutlineCommand::Start { project } => Ok(vec![OutlineEvent::Started { project }]),
            _ => Err(OutlineError::NotStartedYet),
        }
    }

    fn from_first(event: &OutlineEvent, _metadata: &EventMetadata) -> Option<Self> {
        match event {
            OutlineEvent::Started { project } => Some(Self {
                project: project.clone(),
                sections: IndexMap::new(),
                children: Vec::new(),
            }),
            OutlineEvent::Snapshotted { project, sections } => {
                let (sections, children) = Self::holding(sections);

                Some(Self {
                    project: project.clone(),
                    sections,
                    children,
                })
            }
            _ => None,
        }
    }

    fn decide(
        &self,
        command: OutlineCommand,
        _agent: &Agent,
    ) -> Result<Vec<OutlineEvent>, OutlineError> {
        match command {
            OutlineCommand::Start { .. } => Err(OutlineError::AlreadyStarted),
            OutlineCommand::Add {
                section,
                under,
                after,
                title,
            } => {
                if self.holds(&section) {
                    return Err(OutlineError::AlreadyThere);
                }

                self.knows_where(under.as_ref(), after.as_ref())?;

                Ok(vec![OutlineEvent::SectionAdded {
                    section,
                    under,
                    after,
                    title,
                }])
            }
            OutlineCommand::Retitle { section, title } => {
                if !self.holds(&section) {
                    return Err(OutlineError::NoSuchSection);
                }

                if self.title_of(&section) == Some(&title) {
                    return Ok(Vec::new());
                }

                Ok(vec![OutlineEvent::SectionRetitled { section, title }])
            }
            OutlineCommand::Move {
                section,
                under,
                after,
            } => {
                if !self.holds(&section) {
                    return Err(OutlineError::NoSuchSection);
                }

                if under
                    .as_ref()
                    .is_some_and(|under| self.descends_from(under, &section))
                {
                    return Err(OutlineError::WouldContainItself);
                }

                self.knows_where(under.as_ref(), after.as_ref())?;

                if after.as_ref() == Some(&section) {
                    return Err(OutlineError::NoSuchNeighbour);
                }

                Ok(vec![OutlineEvent::SectionMoved {
                    section,
                    under,
                    after,
                }])
            }
            OutlineCommand::Promote { section } => {
                if !self.holds(&section) {
                    return Err(OutlineError::NoSuchSection);
                }

                match self.parent_of(&section) {
                    None => Ok(Vec::new()),
                    Some(_) => Ok(vec![OutlineEvent::SectionPromoted { section }]),
                }
            }
            OutlineCommand::Demote { section } => {
                if !self.holds(&section) {
                    return Err(OutlineError::NoSuchSection);
                }

                match self.rank_of(&section) {
                    Some(0) | None => Ok(Vec::new()),
                    Some(_) => Ok(vec![OutlineEvent::SectionDemoted { section }]),
                }
            }
            OutlineCommand::Remove { section } => {
                if !self.holds(&section) {
                    return Err(OutlineError::NoSuchSection);
                }

                Ok(vec![OutlineEvent::SectionRemoved { section }])
            }
            OutlineCommand::AttachPiece { piece, to, after } => {
                if !self.holds(&to) {
                    return Err(OutlineError::NoSuchSection);
                }

                if let Some(after) = &after
                    && (after == &piece || !self.pieces_in(&to).contains(after))
                {
                    return Err(OutlineError::NoSuchNeighbour);
                }

                Ok(vec![OutlineEvent::PieceAttached { piece, to, after }])
            }
            OutlineCommand::DetachPiece { piece } => {
                if self.section_holding(&piece).is_none() {
                    return Err(OutlineError::NotAttached);
                }

                Ok(vec![OutlineEvent::PieceDetached { piece }])
            }
        }
    }

    fn apply(&mut self, event: &OutlineEvent, _metadata: &EventMetadata) {
        match event {
            OutlineEvent::Started { .. } => {}
            OutlineEvent::SectionAdded {
                section,
                under,
                after,
                title,
            } => self.add(section, *under, after.as_ref(), title.clone()),
            OutlineEvent::SectionRetitled { section, title } => {
                self.retitle(section, title.clone())
            }
            OutlineEvent::SectionMoved {
                section,
                under,
                after,
            } => self.put(section, *under, after.as_ref()),
            OutlineEvent::SectionPromoted { section } => self.promote(section),
            OutlineEvent::SectionDemoted { section } => self.demote(section),
            OutlineEvent::SectionRemoved { section } => self.remove(section),
            OutlineEvent::PieceAttached { piece, to, after } => {
                self.attach(piece, to, after.as_ref())
            }
            OutlineEvent::PieceDetached { piece } => self.detach(piece),
            OutlineEvent::Snapshotted { project, sections } => {
                let (held, children) = Self::holding(sections);

                self.project = project.clone();
                self.sections = held;
                self.children = children;
            }
        }
    }

    fn snapshot(&self) -> OutlineEvent {
        OutlineEvent::Snapshotted {
            project: self.project.clone(),
            sections: self.sections(),
        }
    }
}
