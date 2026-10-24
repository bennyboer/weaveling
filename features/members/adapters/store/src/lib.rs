use eventsourcing::{AccountId, Codec};
use members_core::{Member, MembersEvent, Role};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
enum StoredMembersEvent {
    Founded { owner: String },
    Joined { account: String, role: StoredRole },
    RoleChanged { account: String, to: StoredRole },
    Left { account: String },
    Removed { account: String },
    Snapshotted { members: Vec<StoredMember> },
}

#[derive(Serialize, Deserialize)]
struct StoredMember {
    account: String,
    role: StoredRole,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
enum StoredRole {
    Owner,
    Author,
    Reader,
}

pub fn codec() -> Codec<MembersEvent> {
    Codec {
        body: |event| {
            serde_json::to_value(StoredMembersEvent::from(event))
                .expect("a stored members event is plain data and cannot fail to serialize")
        },
        event: |body| {
            serde_json::from_value::<StoredMembersEvent>(body)
                .ok()
                .map(MembersEvent::from)
        },
    }
}

impl From<Role> for StoredRole {
    fn from(role: Role) -> Self {
        match role {
            Role::Owner => Self::Owner,
            Role::Author => Self::Author,
            Role::Reader => Self::Reader,
        }
    }
}

impl From<StoredRole> for Role {
    fn from(role: StoredRole) -> Self {
        match role {
            StoredRole::Owner => Self::Owner,
            StoredRole::Author => Self::Author,
            StoredRole::Reader => Self::Reader,
        }
    }
}

impl From<&MembersEvent> for StoredMembersEvent {
    fn from(event: &MembersEvent) -> Self {
        match event {
            MembersEvent::Founded { owner } => Self::Founded {
                owner: owner.to_string(),
            },
            MembersEvent::Joined { account, role } => Self::Joined {
                account: account.to_string(),
                role: (*role).into(),
            },
            MembersEvent::RoleChanged { account, to } => Self::RoleChanged {
                account: account.to_string(),
                to: (*to).into(),
            },
            MembersEvent::Left { account } => Self::Left {
                account: account.to_string(),
            },
            MembersEvent::Removed { account } => Self::Removed {
                account: account.to_string(),
            },
            MembersEvent::Snapshotted { members } => Self::Snapshotted {
                members: members
                    .iter()
                    .map(|member| StoredMember {
                        account: member.account.to_string(),
                        role: member.role.into(),
                    })
                    .collect(),
            },
        }
    }
}

impl From<StoredMembersEvent> for MembersEvent {
    fn from(stored: StoredMembersEvent) -> Self {
        match stored {
            StoredMembersEvent::Founded { owner } => Self::Founded {
                owner: AccountId::from(owner),
            },
            StoredMembersEvent::Joined { account, role } => Self::Joined {
                account: AccountId::from(account),
                role: role.into(),
            },
            StoredMembersEvent::RoleChanged { account, to } => Self::RoleChanged {
                account: AccountId::from(account),
                to: to.into(),
            },
            StoredMembersEvent::Left { account } => Self::Left {
                account: AccountId::from(account),
            },
            StoredMembersEvent::Removed { account } => Self::Removed {
                account: AccountId::from(account),
            },
            StoredMembersEvent::Snapshotted { members } => Self::Snapshotted {
                members: members
                    .into_iter()
                    .map(|member| Member {
                        account: AccountId::from(member.account),
                        role: member.role.into(),
                    })
                    .collect(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn grace() -> AccountId {
        AccountId::from("account_grace")
    }

    fn round_trip(event: MembersEvent) -> MembersEvent {
        let codec = codec();

        (codec.event)((codec.body)(&event)).expect("what we just wrote should be readable")
    }

    #[test]
    fn every_event_in_every_role_survives_the_journey_through_the_column() {
        for role in [Role::Owner, Role::Author, Role::Reader] {
            for event in [
                MembersEvent::Founded { owner: grace() },
                MembersEvent::Joined {
                    account: grace(),
                    role,
                },
                MembersEvent::RoleChanged {
                    account: grace(),
                    to: role,
                },
                MembersEvent::Left { account: grace() },
                MembersEvent::Removed { account: grace() },
                MembersEvent::Snapshotted {
                    members: vec![
                        Member {
                            account: AccountId::from("account_ada"),
                            role: Role::Owner,
                        },
                        Member {
                            account: grace(),
                            role,
                        },
                    ],
                },
            ] {
                assert_eq!(round_trip(event.clone()), event);
            }
        }
    }

    #[test]
    fn a_role_is_stored_by_its_name() {
        assert_eq!(
            (codec().body)(&MembersEvent::Joined {
                account: grace(),
                role: Role::Reader,
            }),
            json!({ "Joined": { "account": "account_grace", "role": "reader" } })
        );
    }

    #[test]
    fn a_role_nobody_declared_reads_as_nothing() {
        assert!(
            (codec().event)(json!({ "Joined": { "account": "account_grace", "role": "editor" } }))
                .is_none()
        );
    }
}
