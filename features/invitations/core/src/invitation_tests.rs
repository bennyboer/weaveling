use email::EmailAddress;
use eventsourcing::{Agent, Aggregate, AggregateContext, AggregateId, EventMetadata, Version};
use time::{Duration, OffsetDateTime};

use crate::invitation::*;
use crate::scope::{ProjectLink, Role, Scope};
use crate::token::InvitationTokenDigest;

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn an_operator() -> Agent {
    Agent::Account("account_1".into())
}

fn stamped(version: u64, occurred_at: OffsetDateTime) -> EventMetadata {
    EventMetadata {
        aggregate: AggregateId::from("invitation_1"),
        kind: KIND,
        version: Version::of(version),
        agent: an_operator(),
        occurred_at,
        is_snapshot: false,
        context: AggregateContext::default(),
    }
}

fn ada() -> EmailAddress {
    EmailAddress::new("ada@example.com").expect("a plain address")
}

fn an_issue(seconds: i64) -> InvitationEvent {
    InvitationEvent::Issued {
        to: ada(),
        digest: InvitationTokenDigest::kept("ab12".to_owned()),
        until: at(seconds) + Invitation::LASTS,
        scope: Scope::Instance,
        by_an_operator: true,
    }
}

fn issued_at(seconds: i64) -> Invitation {
    Invitation::from_first(&an_issue(seconds), &stamped(1, at(seconds)))
        .expect("issuing gives birth")
}

fn accepting(seconds: i64) -> InvitationCommand {
    InvitationCommand::Accept { at: at(seconds) }
}

#[test]
fn an_invitation_is_issued_to_an_address_and_stands_open_for_seven_days() {
    assert_eq!(
        Invitation::begin(
            InvitationCommand::Issue {
                to: ada(),
                digest: InvitationTokenDigest::kept("ab12".to_owned()),
                until: at(1_000) + Invitation::LASTS,
                scope: Scope::Instance,
                by_an_operator: true,
            },
            &an_operator()
        ),
        Ok(vec![an_issue(1_000)])
    );

    let invitation = issued_at(1_000);

    assert_eq!(invitation.to(), &ada());
    assert_eq!(invitation.outcome(), Outcome::Open);
    assert_eq!(invitation.until(), at(1_000) + Duration::days(7));
}

#[test]
fn an_invitation_holds_until_its_issue_says_however_late_it_was_told() {
    let issue = InvitationEvent::Issued {
        to: ada(),
        digest: InvitationTokenDigest::kept("ab12".to_owned()),
        until: at(5_000),
        scope: Scope::Instance,
        by_an_operator: true,
    };

    let invitation =
        Invitation::from_first(&issue, &stamped(1, at(1_000))).expect("issuing gives birth");

    assert_eq!(invitation.until(), at(5_000));
}

#[test]
fn an_open_invitation_is_taken_up_once() {
    let mut invitation = issued_at(1_000);

    assert_eq!(
        invitation.decide(accepting(2_000), &an_operator()),
        Ok(vec![InvitationEvent::Accepted])
    );
    invitation.apply(&InvitationEvent::Accepted, &stamped(2, at(2_000)));

    assert_eq!(invitation.outcome(), Outcome::Accepted);
    assert_eq!(
        invitation.decide(accepting(3_000), &an_operator()),
        Err(InvitationError::AlreadyAccepted),
        "one link signs one account up"
    );
}

#[test]
fn an_invitation_holds_to_its_last_moment_and_no_longer() {
    let last = 1_000 + Duration::days(7).whole_seconds();

    assert_eq!(
        issued_at(1_000).decide(accepting(last), &an_operator()),
        Ok(vec![InvitationEvent::Accepted])
    );
    assert_eq!(
        issued_at(1_000).decide(accepting(last + 1), &an_operator()),
        Err(InvitationError::Expired)
    );
}

#[test]
fn a_withdrawn_invitation_is_not_taken_up_and_withdrawing_twice_changes_nothing() {
    let mut invitation = issued_at(1_000);

    assert_eq!(
        invitation.decide(InvitationCommand::Withdraw, &an_operator()),
        Ok(vec![InvitationEvent::Withdrawn])
    );
    invitation.apply(&InvitationEvent::Withdrawn, &stamped(2, at(2_000)));

    assert_eq!(invitation.outcome(), Outcome::Withdrawn);
    assert_eq!(
        invitation.decide(accepting(3_000), &an_operator()),
        Err(InvitationError::Withdrawn)
    );
    assert_eq!(
        invitation.decide(InvitationCommand::Withdraw, &an_operator()),
        Ok(vec![])
    );
}

#[test]
fn an_invitation_taken_up_is_not_withdrawn() {
    let mut invitation = issued_at(1_000);
    invitation.apply(&InvitationEvent::Accepted, &stamped(2, at(2_000)));

    assert_eq!(
        invitation.decide(InvitationCommand::Withdraw, &an_operator()),
        Err(InvitationError::AlreadyAccepted),
        "the account it made stays; it is locked or deleted as any other"
    );
}

#[test]
fn an_expired_invitation_may_still_be_withdrawn() {
    let late = 1_000 + Duration::days(30).whole_seconds();
    let invitation = issued_at(1_000);

    assert!(invitation.is_expired(at(late)));
    assert_eq!(
        invitation.decide(InvitationCommand::Withdraw, &an_operator()),
        Ok(vec![InvitationEvent::Withdrawn])
    );
}

#[test]
fn nothing_but_issuing_comes_first_and_issuing_comes_once() {
    for command in [accepting(1_000), InvitationCommand::Withdraw] {
        assert_eq!(
            Invitation::begin(command, &an_operator()),
            Err(InvitationError::NotIssuedYet)
        );
    }
    assert_eq!(
        issued_at(1_000).decide(
            InvitationCommand::Issue {
                to: ada(),
                digest: InvitationTokenDigest::kept("cd34".to_owned()),
                until: at(2_000),
                scope: Scope::Instance,
                by_an_operator: true,
            },
            &an_operator()
        ),
        Err(InvitationError::AlreadyIssued)
    );
    for event in [
        InvitationEvent::Retired,
        InvitationEvent::Accepted,
        InvitationEvent::Withdrawn,
        InvitationEvent::Declined {
            reason: DeclineReason::AddressHasAccount,
        },
    ] {
        assert_eq!(Invitation::from_first(&event, &stamped(1, at(1_000))), None);
    }
}

#[test]
fn a_snapshot_holds_everything_the_invitation_is() {
    let mut invitation = issued_at(1_000);
    invitation.apply(&InvitationEvent::Withdrawn, &stamped(2, at(2_000)));

    let restored = Invitation::from_first(&invitation.snapshot(), &stamped(2, at(3_000)))
        .expect("a snapshot gives birth");
    let mut behind = issued_at(500);
    behind.apply(&invitation.snapshot(), &stamped(3, at(3_000)));

    assert_eq!(restored, invitation);
    assert_eq!(behind, invitation);
}

#[test]
fn a_snapshot_keeps_its_scope_and_whether_an_operator_issued_it() {
    let into_a_project = InvitationEvent::Issued {
        to: ada(),
        digest: InvitationTokenDigest::kept("ab12".to_owned()),
        until: at(1_000) + Invitation::LASTS,
        scope: Scope::Project {
            project: ProjectLink::from("project_1"),
            role: Role::Reader,
        },
        by_an_operator: false,
    };
    let invitation = Invitation::from_first(&into_a_project, &stamped(1, at(1_000)))
        .expect("issuing gives birth");
    assert!(!invitation.is_by_an_operator());

    let restored = Invitation::from_first(&invitation.snapshot(), &stamped(2, at(2_000)))
        .expect("a snapshot gives birth");
    let mut issued_otherwise = issued_at(500);
    assert!(issued_otherwise.is_by_an_operator());
    issued_otherwise.apply(&invitation.snapshot(), &stamped(2, at(2_000)));

    for told in [restored, issued_otherwise] {
        assert!(!told.is_by_an_operator());
        assert_eq!(told.scope(), invitation.scope());
    }
}

fn declined_at(seconds: i64) -> Invitation {
    let mut invitation = issued_at(seconds);
    let declined = invitation
        .decide(
            InvitationCommand::Decline {
                reason: DeclineReason::AddressHasAccount,
            },
            &Agent::System,
        )
        .expect("an open invitation is declined");
    assert_eq!(
        declined,
        vec![InvitationEvent::Declined {
            reason: DeclineReason::AddressHasAccount,
        }]
    );
    invitation.apply(&declined[0], &stamped(2, at(seconds + 10)));

    invitation
}

#[test]
fn an_invitation_to_an_address_with_an_account_is_declined_and_closed() {
    let invitation = declined_at(1_000);

    assert_eq!(
        invitation.outcome(),
        Outcome::Declined(DeclineReason::AddressHasAccount)
    );
    assert_eq!(
        invitation.decide(
            InvitationCommand::Decline {
                reason: DeclineReason::AddressHasAccount,
            },
            &Agent::System
        ),
        Ok(vec![]),
        "declining twice changes nothing"
    );
    assert_eq!(
        invitation.decide(InvitationCommand::Withdraw, &an_operator()),
        Ok(vec![]),
        "an operator withdrawing what was declined is told nothing is left to do"
    );
    assert_eq!(
        invitation.decide(InvitationCommand::Accept { at: at(1_100) }, &Agent::System),
        Err(InvitationError::Declined)
    );
}

#[test]
fn an_invitation_withdrawn_or_taken_up_is_not_declined() {
    let mut withdrawn = issued_at(1_000);
    withdrawn.apply(&InvitationEvent::Withdrawn, &stamped(2, at(1_010)));
    let mut accepted = issued_at(1_000);
    accepted.apply(&InvitationEvent::Accepted, &stamped(2, at(1_010)));

    assert_eq!(
        withdrawn.decide(
            InvitationCommand::Decline {
                reason: DeclineReason::AddressHasAccount,
            },
            &Agent::System
        ),
        Err(InvitationError::Withdrawn)
    );
    assert_eq!(
        accepted.decide(
            InvitationCommand::Decline {
                reason: DeclineReason::AddressHasAccount,
            },
            &Agent::System
        ),
        Err(InvitationError::AlreadyAccepted)
    );
    assert_eq!(
        Invitation::begin(
            InvitationCommand::Decline {
                reason: DeclineReason::AddressHasAccount,
            },
            &Agent::System
        ),
        Err(InvitationError::NotIssuedYet)
    );
}

#[test]
fn an_invitation_remembers_when_it_was_settled_and_an_open_one_was_not() {
    let open = issued_at(1_000);
    let mut withdrawn = issued_at(1_000);
    withdrawn.apply(&InvitationEvent::Withdrawn, &stamped(2, at(4_000)));

    assert_eq!(open.settled_at(), None);
    assert_eq!(withdrawn.settled_at(), Some(at(4_000)));
}

#[test]
fn an_invitation_retires_once_however_it_stands() {
    let mut withdrawn = issued_at(1_000);
    withdrawn.apply(&InvitationEvent::Withdrawn, &stamped(2, at(2_000)));

    for invitation in [issued_at(1_000), withdrawn, declined_at(1_000)] {
        let retiring = invitation
            .decide(InvitationCommand::Retire, &Agent::System)
            .expect("any invitation retires");
        assert_eq!(retiring, vec![InvitationEvent::Retired]);

        let mut retired = invitation.clone();
        retired.apply(&retiring[0], &stamped(3, at(3_000)));
        assert!(retired.is_retired());
        assert_eq!(
            retired.outcome(),
            invitation.outcome(),
            "retiring changes how it stood not at all"
        );
        assert_eq!(
            retired.decide(InvitationCommand::Retire, &Agent::System),
            Ok(vec![]),
            "retiring twice changes nothing"
        );
        let restored = Invitation::from_first(&retired.snapshot(), &stamped(3, at(4_000)))
            .expect("a snapshot gives birth");
        assert!(restored.is_retired(), "a snapshot keeps that it retired");
    }
    assert_eq!(
        Invitation::begin(InvitationCommand::Retire, &Agent::System),
        Err(InvitationError::NotIssuedYet)
    );
}
