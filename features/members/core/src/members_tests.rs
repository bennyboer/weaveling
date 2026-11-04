use eventsourcing::{
    AccountId, Agent, Aggregate, AggregateContext, AggregateId, Event, EventMetadata, Version,
};
use projects_contract::context::PROJECT;
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

fn members_with(joining: &[(AccountId, Role)]) -> Members {
    let joined: Vec<MembersEvent> = joining
        .iter()
        .map(|(account, role)| MembersEvent::Joined {
            account: account.clone(),
            role: *role,
        })
        .collect();
    after(founded_by_ada(), &joined)
}

fn deciding(members: &Members, command: MembersCommand) -> Result<Vec<MembersEvent>, MembersError> {
    members.decide(command, &an_owner())
}

#[test]
fn whoever_founds_a_project_s_members_is_its_only_owner() {
    // expect: founding names its owner
    assert_eq!(
        Members::begin(MembersCommand::Found { owner: ada() }, &an_owner()),
        Ok(vec![MembersEvent::Founded { owner: ada() }])
    );
    // and: members founded hold that owner alone
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
    // expect: every command before a founding is refused
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
    // and: a joining gives no birth
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
    // expect: founding members founded already is refused
    assert_eq!(
        deciding(&founded_by_ada(), MembersCommand::Found { owner: grace() }),
        Err(MembersError::AlreadyFounded)
    );
}

#[test]
fn an_account_joins_in_a_role_and_joining_again_in_it_changes_nothing() {
    // given: a joining as author
    let joining = MembersCommand::Join {
        account: grace(),
        role: Role::Author,
    };

    // when: a newcomer joins
    let joined = deciding(&founded_by_ada(), joining.clone()).expect("a newcomer joins");

    // then: they joined as author
    assert_eq!(
        joined,
        vec![MembersEvent::Joined {
            account: grace(),
            role: Role::Author
        }]
    );

    // when: the joining happens
    let members = after(founded_by_ada(), &joined);

    // then: they are an author
    assert_eq!(members.role_of(&grace()), Some(Role::Author));
    // and: joining again as author changes nothing
    assert_eq!(deciding(&members, joining), Ok(vec![]));
    // and: joining again in another role is refused
    assert_eq!(
        deciding(
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
    // given: three who joined one after another
    let members = members_with(&[
        (grace(), Role::Author),
        (hedy(), Role::Reader),
        (ken(), Role::Author),
    ]);

    // when: the first leaves and joins again
    let members = after(members, &[MembersEvent::Left { account: grace() }]);
    let members = after(
        members,
        &[MembersEvent::Joined {
            account: grace(),
            role: Role::Reader,
        }],
    );

    // then: the members are listed as they joined, the one who came back last
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
    // given: an author
    let members = members_with(&[(grace(), Role::Author)]);

    // when: their role is changed to reader
    let changed = deciding(
        &members,
        MembersCommand::ChangeRole {
            account: grace(),
            to: Role::Reader,
        },
    )
    .expect("an author may become a reader");

    // then: it says their role changed
    assert_eq!(
        changed,
        vec![MembersEvent::RoleChanged {
            account: grace(),
            to: Role::Reader
        }]
    );
    // and: once it happened they are a reader
    assert_eq!(
        after(members.clone(), &changed).role_of(&grace()),
        Some(Role::Reader)
    );
    // and: changing it to author, as it is, changes nothing
    assert_eq!(
        deciding(
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
    // expect: changing the role of someone not a member is refused
    assert_eq!(
        deciding(
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
    // given: two members
    let members = members_with(&[(grace(), Role::Author), (hedy(), Role::Reader)]);

    // expect: one may leave
    assert_eq!(
        deciding(&members, MembersCommand::Leave { account: grace() }),
        Ok(vec![MembersEvent::Left { account: grace() }])
    );
    // and: the other may be removed
    assert_eq!(
        deciding(&members, MembersCommand::Remove { account: hedy() }),
        Ok(vec![MembersEvent::Removed { account: hedy() }])
    );

    // when: both are gone
    let gone = after(
        members,
        &[
            MembersEvent::Left { account: grace() },
            MembersEvent::Removed { account: hedy() },
        ],
    );

    // then: neither has a role
    assert_eq!(gone.role_of(&grace()), None);
    assert_eq!(gone.role_of(&hedy()), None);
    // and: leaving or being removed again changes nothing
    assert_eq!(
        deciding(&gone, MembersCommand::Leave { account: grace() }),
        Ok(vec![])
    );
    assert_eq!(
        deciding(&gone, MembersCommand::Remove { account: hedy() }),
        Ok(vec![])
    );
}

#[test]
fn the_last_owner_neither_leaves_nor_is_removed_nor_steps_down() {
    // given: one owner and an author
    let members = members_with(&[(grace(), Role::Author)]);

    // expect: the owner may neither leave, nor be removed, nor step down
    for command in [
        MembersCommand::Leave { account: ada() },
        MembersCommand::Remove { account: ada() },
        MembersCommand::ChangeRole {
            account: ada(),
            to: Role::Author,
        },
    ] {
        assert_eq!(deciding(&members, command), Err(MembersError::LastOwner));
    }
    // and: staying owner changes nothing
    assert_eq!(
        deciding(
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
    // given: two owners
    let members = members_with(&[(grace(), Role::Owner)]);

    // expect: one may leave
    assert_eq!(
        deciding(&members, MembersCommand::Leave { account: ada() }),
        Ok(vec![MembersEvent::Left { account: ada() }])
    );
    // and: may be removed
    assert_eq!(
        deciding(&members, MembersCommand::Remove { account: ada() }),
        Ok(vec![MembersEvent::Removed { account: ada() }])
    );
    // and: may step down
    assert_eq!(
        deciding(
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
fn only_an_owner_as_the_members_stand_changes_a_role_or_removes() {
    // given: an author and a reader
    let members = members_with(&[(grace(), Role::Author), (hedy(), Role::Reader)]);
    let grace_acts = Agent::Account(grace());

    // expect: the author may not make themselves owner
    assert_eq!(
        members.decide(
            MembersCommand::ChangeRole {
                account: grace(),
                to: Role::Owner
            },
            &grace_acts
        ),
        Err(MembersError::NotAnOwner),
        "an author cannot make themselves owner, whatever a list read earlier said"
    );
    // and: may not remove the reader
    assert_eq!(
        members.decide(MembersCommand::Remove { account: hedy() }, &grace_acts),
        Err(MembersError::NotAnOwner)
    );
    // and: nor may someone not a member
    assert_eq!(
        members.decide(
            MembersCommand::Remove { account: hedy() },
            &Agent::Account(ken())
        ),
        Err(MembersError::NotAnOwner),
        "someone removed, or never a member, manages nothing"
    );
    // and: the system may
    assert_eq!(
        members.decide(MembersCommand::Remove { account: hedy() }, &Agent::System),
        Ok(vec![MembersEvent::Removed { account: hedy() }]),
        "the system acts for no member"
    );
}

#[test]
fn a_snapshot_holds_every_member_in_order_with_their_role() {
    // given: three members
    let members = members_with(&[(grace(), Role::Author), (hedy(), Role::Reader)]);

    // when: they are snapshotted
    let snapshot = members.snapshot();

    // then: the snapshot says it is one
    assert!(snapshot.is_snapshot());
    // and: members born of it are the same members
    assert_eq!(
        Members::from_first(&snapshot, &stamped(4)).expect("a snapshot gives birth"),
        members
    );
    // and: so are members it is applied to
    assert_eq!(after(founded_by_ada(), &[snapshot]), members);
}

#[test]
fn every_event_is_named_and_only_the_snapshot_is_one() {
    // when: every event is named and asked whether it is a snapshot
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
        MembersEvent::Disbanded,
        MembersEvent::Snapshotted {
            members: vec![],
            disbanded: false,
        },
    ]
    .iter()
    .map(|event| (event.name().to_string(), event.is_snapshot()))
    .collect();

    // then: each says its own name and only the snapshot is one
    assert_eq!(
        named,
        vec![
            ("FOUNDED".to_owned(), false),
            ("JOINED".to_owned(), false),
            ("ROLE_CHANGED".to_owned(), false),
            ("LEFT".to_owned(), false),
            ("REMOVED".to_owned(), false),
            ("DISBANDED".to_owned(), false),
            ("SNAPSHOTTED".to_owned(), true),
        ]
    );
}

#[test]
fn a_project_s_members_are_kept_under_the_project_s_own_id() {
    // expect: a project's members are kept under its id
    assert_eq!(
        AggregateId::from(&ProjectLink::from("project_1")),
        AggregateId::from("project_1")
    );
}

#[test]
fn a_project_s_members_are_in_its_context_and_stay_so_through_a_snapshot() {
    // given: a project's members
    let members = members_with(&[(grace(), Role::Author)]);

    // when: they are rebuilt from their snapshot and their snapshot is applied
    let rebuilt =
        Members::from_first(&members.snapshot(), &stamped(4)).expect("a snapshot gives birth");
    let applied = after(founded_by_ada(), &[members.snapshot()]);

    // then: each is in the project's context
    for held in [&members, &rebuilt, &applied] {
        assert_eq!(held.context().get(PROJECT), Some("project_1"));
    }
}

#[test]
fn a_deleted_project_s_members_are_disbanded_all_at_once_and_once() {
    // given: three members
    let members = members_with(&[(grace(), Role::Author), (hedy(), Role::Reader)]);

    // expect: disbanding them is one event
    assert_eq!(
        deciding(&members, MembersCommand::Disband),
        Ok(vec![MembersEvent::Disbanded])
    );

    // when: they are disbanded
    let disbanded = after(members, &[MembersEvent::Disbanded]);
    // then: nobody is left, the owner neither
    assert!(disbanded.members().is_empty());
    assert_eq!(disbanded.role_of(&ada()), None);
    // and: disbanding again changes nothing
    assert_eq!(deciding(&disbanded, MembersCommand::Disband), Ok(vec![]));
}

#[test]
fn nobody_joins_or_changes_among_members_disbanded() {
    // given: members disbanded
    let disbanded = after(
        members_with(&[(grace(), Role::Author)]),
        &[MembersEvent::Disbanded],
    );

    // expect: every command is refused
    for command in [
        MembersCommand::Found { owner: ken() },
        MembersCommand::Join {
            account: ken(),
            role: Role::Reader,
        },
        MembersCommand::ChangeRole {
            account: grace(),
            to: Role::Owner,
        },
        MembersCommand::Leave { account: grace() },
        MembersCommand::Remove { account: grace() },
    ] {
        assert_eq!(deciding(&disbanded, command), Err(MembersError::Disbanded));
    }
}

#[test]
fn members_disbanded_before_they_were_founded_are_founded_no_more() {
    // expect: members never founded may be disbanded
    assert_eq!(
        Members::begin(MembersCommand::Disband, &Agent::System),
        Ok(vec![MembersEvent::Disbanded])
    );

    // when: they are born disbanded
    let disbanded = Members::from_first(&MembersEvent::Disbanded, &stamped(1))
        .expect("a disbanding gives birth, so a founding heard late finds them gone");

    // then: nobody is in them
    assert!(disbanded.members().is_empty());
    // and: founding them is refused
    assert_eq!(
        deciding(&disbanded, MembersCommand::Found { owner: ada() }),
        Err(MembersError::Disbanded)
    );
}

#[test]
fn a_snapshot_of_members_disbanded_keeps_them_disbanded() {
    // given: members disbanded
    let disbanded = after(
        members_with(&[(grace(), Role::Author)]),
        &[MembersEvent::Disbanded],
    );

    // when: they are snapshotted
    let snapshot = disbanded.snapshot();

    // then: the snapshot holds nobody and says they are disbanded
    assert_eq!(
        snapshot,
        MembersEvent::Snapshotted {
            members: vec![],
            disbanded: true,
        }
    );
    // and: members born of it refuse a joining
    let read_back = Members::from_first(&snapshot, &stamped(4)).expect("a snapshot gives birth");
    assert_eq!(
        deciding(
            &read_back,
            MembersCommand::Join {
                account: ken(),
                role: Role::Reader,
            }
        ),
        Err(MembersError::Disbanded)
    );
    // and: members it is applied to are the disbanded ones
    assert_eq!(after(founded_by_ada(), &[snapshot]), disbanded);
}
