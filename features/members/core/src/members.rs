use std::convert::Infallible;
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

use indexmap::IndexMap;

use eventsourcing::{
    AccountId, Agent, Aggregate, AggregateId, AggregateType, Event, EventMetadata, EventName,
    Version,
};
use thiserror::Error;

pub const KIND: AggregateType = AggregateType::of("members");

const FOUNDED: EventName = EventName::of("FOUNDED");
const JOINED: EventName = EventName::of("JOINED");
const ROLE_CHANGED: EventName = EventName::of("ROLE_CHANGED");
const LEFT: EventName = EventName::of("LEFT");
const REMOVED: EventName = EventName::of("REMOVED");
const SNAPSHOTTED: EventName = EventName::of("SNAPSHOTTED");

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectLink(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    Owner,
    Author,
    Reader,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub account: AccountId,
    pub role: Role,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MembersCommand {
    Found { owner: AccountId },
    Join { account: AccountId, role: Role },
    ChangeRole { account: AccountId, to: Role },
    Leave { account: AccountId },
    Remove { account: AccountId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MembersEvent {
    Founded { owner: AccountId },
    Joined { account: AccountId, role: Role },
    RoleChanged { account: AccountId, to: Role },
    Left { account: AccountId },
    Removed { account: AccountId },
    Snapshotted { members: Vec<Member> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Members {
    roles: IndexMap<AccountId, Role>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MembersError {
    #[error("a project's members must be founded before anyone can join")]
    NotFoundedYet,
    #[error("a project's members are founded once")]
    AlreadyFounded,
    #[error("this account is a member already, in another role")]
    AlreadyMember,
    #[error("this account is not a member of the project")]
    NotMember,
    #[error("a project keeps at least one owner; make someone else an owner first")]
    LastOwner,
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

impl FromStr for ProjectLink {
    type Err = Infallible;

    fn from_str(given: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(given))
    }
}

impl Display for ProjectLink {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl From<&ProjectLink> for AggregateId {
    fn from(project: &ProjectLink) -> Self {
        AggregateId::from(project.to_string())
    }
}

impl Members {
    pub fn members(&self) -> Vec<Member> {
        self.roles
            .iter()
            .map(|(account, role)| Member {
                account: account.clone(),
                role: *role,
            })
            .collect()
    }

    pub fn role_of(&self, account: &AccountId) -> Option<Role> {
        self.roles.get(account).copied()
    }

    fn is_last_owner(&self, account: &AccountId) -> bool {
        self.role_of(account) == Some(Role::Owner)
            && self
                .roles
                .values()
                .filter(|role| **role == Role::Owner)
                .count()
                == 1
    }

    fn of(members: &[Member]) -> Self {
        Self {
            roles: members
                .iter()
                .map(|member| (member.account.clone(), member.role))
                .collect(),
        }
    }
}

impl Event for MembersEvent {
    fn name(&self) -> EventName {
        match self {
            Self::Founded { .. } => FOUNDED,
            Self::Joined { .. } => JOINED,
            Self::RoleChanged { .. } => ROLE_CHANGED,
            Self::Left { .. } => LEFT,
            Self::Removed { .. } => REMOVED,
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

impl Aggregate for Members {
    type Command = MembersCommand;
    type Event = MembersEvent;
    type Error = MembersError;

    const KIND: AggregateType = KIND;

    fn begin(command: MembersCommand, _agent: &Agent) -> Result<Vec<MembersEvent>, MembersError> {
        match command {
            MembersCommand::Found { owner } => Ok(vec![MembersEvent::Founded { owner }]),
            MembersCommand::Join { .. }
            | MembersCommand::ChangeRole { .. }
            | MembersCommand::Leave { .. }
            | MembersCommand::Remove { .. } => Err(MembersError::NotFoundedYet),
        }
    }

    fn from_first(event: &MembersEvent, _metadata: &EventMetadata) -> Option<Self> {
        match event {
            MembersEvent::Founded { owner } => Some(Self::of(&[Member {
                account: owner.clone(),
                role: Role::Owner,
            }])),
            MembersEvent::Snapshotted { members } => Some(Self::of(members)),
            MembersEvent::Joined { .. }
            | MembersEvent::RoleChanged { .. }
            | MembersEvent::Left { .. }
            | MembersEvent::Removed { .. } => None,
        }
    }

    fn decide(
        &self,
        command: MembersCommand,
        _agent: &Agent,
    ) -> Result<Vec<MembersEvent>, MembersError> {
        match command {
            MembersCommand::Found { .. } => Err(MembersError::AlreadyFounded),
            MembersCommand::Join { account, role } => match self.role_of(&account) {
                Some(already) if already == role => Ok(vec![]),
                Some(_) => Err(MembersError::AlreadyMember),
                None => Ok(vec![MembersEvent::Joined { account, role }]),
            },
            MembersCommand::ChangeRole { account, to } => match self.role_of(&account) {
                None => Err(MembersError::NotMember),
                Some(already) if already == to => Ok(vec![]),
                Some(_) if self.is_last_owner(&account) => Err(MembersError::LastOwner),
                Some(_) => Ok(vec![MembersEvent::RoleChanged { account, to }]),
            },
            MembersCommand::Leave { account } => match self.role_of(&account) {
                None => Ok(vec![]),
                Some(_) if self.is_last_owner(&account) => Err(MembersError::LastOwner),
                Some(_) => Ok(vec![MembersEvent::Left { account }]),
            },
            MembersCommand::Remove { account } => match self.role_of(&account) {
                None => Ok(vec![]),
                Some(_) if self.is_last_owner(&account) => Err(MembersError::LastOwner),
                Some(_) => Ok(vec![MembersEvent::Removed { account }]),
            },
        }
    }

    fn apply(&mut self, event: &MembersEvent, _metadata: &EventMetadata) {
        match event {
            MembersEvent::Founded { .. } => {}
            MembersEvent::Joined { account, role } => {
                self.roles.insert(account.clone(), *role);
            }
            MembersEvent::RoleChanged { account, to } => {
                if let Some(role) = self.roles.get_mut(account) {
                    *role = *to;
                }
            }
            MembersEvent::Left { account } | MembersEvent::Removed { account } => {
                self.roles.shift_remove(account);
            }
            MembersEvent::Snapshotted { members } => *self = Self::of(members),
        }
    }

    fn snapshot(&self) -> MembersEvent {
        MembersEvent::Snapshotted {
            members: self.members(),
        }
    }
}
