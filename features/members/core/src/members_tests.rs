use eventsourcing::{
    AccountId, Agent, Aggregate, AggregateContext, AggregateId, Event, EventMetadata, Version,
};
use time::{Duration, OffsetDateTime};

use crate::members::*;

fn an_owner() -> Agent {
    Agent::Account(ada())
}

fn stamped(version: u64) -> EventMetadata {
    EventMetadata {
        aggregate: AggregateId::from("project_1"),
        kind: KIND,
        version: Version::of(version),
        agent: an_owner(),
        occurred_at: OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_000),
        is_snapshot: false,
        context: AggregateContext::default(),
    }
}

fn ada() -> AccountId {
    AccountId::from("account_ada")
}

fn grace() -> AccountId {
    AccountId::from("account_grace")
}

fn hedy() -> AccountId {
    AccountId::from("account_hedy")
}

fn ken() -> AccountId {
    AccountId::from("account_ken")
}

fn founded_by_ada() -> Members {
    Members::from_first(&MembersEvent::Founded { owner: ada() }, &stamped(1))
        .expect("founding gives birth")
}

fn after(members: Members, events: &[MembersEvent]) -> Members {
    events.iter().fold(members, |mut members, event| {
        members.apply(event, &stamped(2));
        members
    })
}

fn with(joining: &[(AccountId, Role)]) -> Members {
    let joined: Vec<MembersEvent> = joining
        .iter()
        .map(|(account, role)| MembersEvent::Joined {
            account: account.clone(),
            role: *role,
        })
        .collect();
    after(founded_by_ada(), &joined)
}

fn told(members: &Members, command: MembersCommand) -> Result<Vec<MembersEvent>, MembersError> {
    members.decide(command, &an_owner())
}

#[test]
fn whoever_founds_a_project_s_members_is_its_only_owner() {
    assert_eq!(
        Members::begin(MembersCommand::Found { owner: ada() }, &an_owner()),
        Ok(vec![MembersEvent::Founded { owner: ada() }])
    );
    assert_eq!(
        founded_by_ada().members(),
        vec![Member {
            account: ada(),
            role: Role::Owner
        }]
    );
}

#[test]
fn nothing_happens_to_members_never_founded() {
    for command in [
        MembersCommand::Join {
            account: grace(),
            role: Role::Author,
        },
        MembersCommand::ChangeRole {
            account: ada(),
            to: Role::Reader,
        },
        MembersCommand::Leave { account: ada() },
        MembersCommand::Remove { account: ada() },
    ] {
        assert_eq!(
            Members::begin(command, &an_owner()),
            Err(MembersError::NotFoundedYet)
        );
    }
    assert!(
        Members::from_first(
            &MembersEvent::Joined {
                account: grace(),
                role: Role::Author
            },
            &stamped(1)
        )
        .is_none()
    );
}

#[test]
fn members_are_founded_once() {
    assert_eq!(
        told(&founded_by_ada(), MembersCommand::Found { owner: grace() }),
        Err(MembersError::AlreadyFounded)
    );
}

#[test]
fn an_account_joins_in_a_role_and_joining_again_in_it_changes_nothing() {
    let joining = MembersCommand::Join {
        account: grace(),
        role: Role::Author,
    };
    let joined = told(&founded_by_ada(), joining.clone()).expect("a newcomer joins");

    assert_eq!(
        joined,
        vec![MembersEvent::Joined {
            account: grace(),
            role: Role::Author
        }]
    );

    let members = after(founded_by_ada(), &joined);

    assert_eq!(members.role_of(&grace()), Some(Role::Author));
    assert_eq!(told(&members, joining), Ok(vec![]));
    assert_eq!(
        told(
            &members,
            MembersCommand::Join {
                account: grace(),
                role: Role::Reader
            }
        ),
        Err(MembersError::AlreadyMember)
    );
}

#[test]
fn members_are_listed_in_the_order_they_joined_whoever_leaves_between() {
    let members = with(&[
        (grace(), Role::Author),
        (hedy(), Role::Reader),
        (ken(), Role::Author),
    ]);
    let members = after(members, &[MembersEvent::Left { account: grace() }]);
    let members = after(
        members,
        &[MembersEvent::Joined {
            account: grace(),
            role: Role::Reader,
        }],
    );

    assert_eq!(
        members.members(),
        vec![
            Member {
                account: ada(),
                role: Role::Owner
            },
            Member {
                account: hedy(),
                role: Role::Reader
            },
            Member {
                account: ken(),
                role: Role::Author
            },
            Member {
                account: grace(),
                role: Role::Reader
            },
        ]
    );
}

#[test]
fn a_member_s_role_changes_and_changing_it_to_what_it_is_changes_nothing() {
    let members = with(&[(grace(), Role::Author)]);
    let changed = told(
        &members,
        MembersCommand::ChangeRole {
            account: grace(),
            to: Role::Reader,
        },
    )
    .expect("an author may become a reader");

    assert_eq!(
        changed,
        vec![MembersEvent::RoleChanged {
            account: grace(),
            to: Role::Reader
        }]
    );
    assert_eq!(
        after(members.clone(), &changed).role_of(&grace()),
        Some(Role::Reader)
    );
    assert_eq!(
        told(
            &members,
            MembersCommand::ChangeRole {
                account: grace(),
                to: Role::Author
            }
        ),
        Ok(vec![])
    );
}

#[test]
fn only_a_member_s_role_changes() {
    assert_eq!(
        told(
            &founded_by_ada(),
            MembersCommand::ChangeRole {
                account: grace(),
                to: Role::Author
            }
        ),
        Err(MembersError::NotMember)
    );
}

#[test]
fn a_member_leaves_or_is_removed_and_one_gone_already_changes_nothing() {
    let members = with(&[(grace(), Role::Author), (hedy(), Role::Reader)]);

    assert_eq!(
        told(&members, MembersCommand::Leave { account: grace() }),
        Ok(vec![MembersEvent::Left { account: grace() }])
    );
    assert_eq!(
        told(&members, MembersCommand::Remove { account: hedy() }),
        Ok(vec![MembersEvent::Removed { account: hedy() }])
    );

    let gone = after(
        members,
        &[
            MembersEvent::Left { account: grace() },
            MembersEvent::Removed { account: hedy() },
        ],
    );

    assert_eq!(gone.role_of(&grace()), None);
    assert_eq!(gone.role_of(&hedy()), None);
    assert_eq!(
        told(&gone, MembersCommand::Leave { account: grace() }),
        Ok(vec![])
    );
    assert_eq!(
        told(&gone, MembersCommand::Remove { account: hedy() }),
        Ok(vec![])
    );
}

#[test]
fn the_last_owner_neither_leaves_nor_is_removed_nor_steps_down() {
    let members = with(&[(grace(), Role::Author)]);

    for command in [
        MembersCommand::Leave { account: ada() },
        MembersCommand::Remove { account: ada() },
        MembersCommand::ChangeRole {
            account: ada(),
            to: Role::Author,
        },
    ] {
        assert_eq!(told(&members, command), Err(MembersError::LastOwner));
    }
    assert_eq!(
        told(
            &members,
            MembersCommand::ChangeRole {
                account: ada(),
                to: Role::Owner
            }
        ),
        Ok(vec![])
    );
}

#[test]
fn an_owner_beside_another_may_leave_be_removed_or_step_down() {
    let members = with(&[(grace(), Role::Owner)]);

    assert_eq!(
        told(&members, MembersCommand::Leave { account: ada() }),
        Ok(vec![MembersEvent::Left { account: ada() }])
    );
    assert_eq!(
        told(&members, MembersCommand::Remove { account: ada() }),
        Ok(vec![MembersEvent::Removed { account: ada() }])
    );
    assert_eq!(
        told(
            &members,
            MembersCommand::ChangeRole {
                account: ada(),
                to: Role::Reader
            }
        ),
        Ok(vec![MembersEvent::RoleChanged {
            account: ada(),
            to: Role::Reader
        }])
    );
}

#[test]
fn a_snapshot_holds_every_member_in_order_with_their_role() {
    let members = with(&[(grace(), Role::Author), (hedy(), Role::Reader)]);
    let snapshot = members.snapshot();

    assert!(snapshot.is_snapshot());
    assert_eq!(
        Members::from_first(&snapshot, &stamped(4)).expect("a snapshot gives birth"),
        members
    );
    assert_eq!(after(founded_by_ada(), &[snapshot]), members);
}

#[test]
fn every_event_is_named_and_only_the_snapshot_is_one() {
    let named: Vec<(String, bool)> = [
        MembersEvent::Founded { owner: ada() },
        MembersEvent::Joined {
            account: grace(),
            role: Role::Author,
        },
        MembersEvent::RoleChanged {
            account: grace(),
            to: Role::Reader,
        },
        MembersEvent::Left { account: grace() },
        MembersEvent::Removed { account: grace() },
        MembersEvent::Snapshotted { members: vec![] },
    ]
    .iter()
    .map(|event| (event.name().to_string(), event.is_snapshot()))
    .collect();

    assert_eq!(
        named,
        vec![
            ("FOUNDED".to_owned(), false),
            ("JOINED".to_owned(), false),
            ("ROLE_CHANGED".to_owned(), false),
            ("LEFT".to_owned(), false),
            ("REMOVED".to_owned(), false),
            ("SNAPSHOTTED".to_owned(), true),
        ]
    );
}

#[test]
fn a_project_s_members_are_kept_under_the_project_s_own_id() {
    assert_eq!(
        AggregateId::from(&ProjectLink::from("project_1")),
        AggregateId::from("project_1")
    );
}
