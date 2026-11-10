use email::EmailAddress;
use eventsourcing::{
    AccountId, Agent, Aggregate, AggregateContext, AggregateId, EventMetadata, Version,
};
use time::{Duration, OffsetDateTime};

use crate::invitation::*;
use crate::scope::{ProjectLink, Role, Scope};
use crate::token::InvitationTokenDigest;

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
        until: clock::at(seconds) + Invitation::LASTS,
        scope: Scope::Instance,
        by_an_operator: true,
        origin: "https://weaveling.example.com".to_owned(),
    }
}

fn an_invitation_issued_at(seconds: i64) -> Invitation {
    Invitation::from_first(&an_issue(seconds), &stamped(1, clock::at(seconds)))
        .expect("issuing gives birth")
}

#[test]
fn an_invitation_is_issued_to_an_address_and_stands_open_for_seven_days() {
    // expect: issuing to an address is decided as an issue
    assert_eq!(
        Invitation::begin(
            InvitationCommand::Issue {
                to: ada(),
                digest: InvitationTokenDigest::kept("ab12".to_owned()),
                until: clock::at(1_000) + Invitation::LASTS,
                scope: Scope::Instance,
                by_an_operator: true,
                origin: "https://weaveling.example.com".to_owned(),
            },
            &an_operator()
        ),
        Ok(vec![an_issue(1_000)])
    );

    // when: an invitation is issued
    let invitation = an_invitation_issued_at(1_000);

    // then: it stands open to the address for seven days
    assert_eq!(invitation.to(), &ada());
    assert_eq!(invitation.outcome(), Outcome::Open);
    assert_eq!(invitation.until(), clock::at(1_000) + Duration::days(7));
}

#[test]
fn an_invitation_holds_until_its_issue_says_however_late_it_was_told() {
    // given: an issue told late
    let issue = InvitationEvent::Issued {
        to: ada(),
        digest: InvitationTokenDigest::kept("ab12".to_owned()),
        until: clock::at(5_000),
        scope: Scope::Instance,
        by_an_operator: true,
        origin: "https://weaveling.example.com".to_owned(),
    };

    // when: an invitation is born of it
    let invitation =
        Invitation::from_first(&issue, &stamped(1, clock::at(1_000))).expect("issuing gives birth");

    // then: it holds until the issue says
    assert_eq!(invitation.until(), clock::at(5_000));
}

#[test]
fn an_open_invitation_is_taken_up_once() {
    // given: an open invitation
    let mut invitation = an_invitation_issued_at(1_000);

    // then: taking it up is decided as accepted
    assert_eq!(
        invitation.decide(InvitationCommand::Accept, &an_operator()),
        Ok(vec![InvitationEvent::Accepted {
            scope: Scope::Instance,
        }])
    );
    // when: it is accepted
    invitation.apply(
        &InvitationEvent::Accepted {
            scope: Scope::Instance,
        },
        &stamped(2, clock::at(2_000)),
    );

    // then: it is accepted
    assert_eq!(invitation.outcome(), Outcome::Accepted);
    // and: taking it up again is refused
    assert_eq!(
        invitation.decide(InvitationCommand::Accept, &an_operator()),
        Err(InvitationError::AlreadyAccepted),
        "one link signs one account up"
    );
}

#[test]
fn an_invitation_taken_up_is_accepted_however_late_it_is_heard() {
    // given: an open invitation past its last moment
    let after_its_last_moment = 1_000 + Duration::days(7).whole_seconds() + 1;
    let invitation = an_invitation_issued_at(1_000);

    // then: it is expired
    assert!(invitation.is_expired(clock::at(after_its_last_moment)));
    // and: taking it up is still decided as accepted
    assert_eq!(
        invitation.decide(InvitationCommand::Accept, &an_operator()),
        Ok(vec![InvitationEvent::Accepted {
            scope: Scope::Instance,
        }]),
        "accounts decided it came in time as it took the ticket, so its last moment is not judged a second time"
    );
}

#[test]
fn a_withdrawn_invitation_is_not_taken_up_and_withdrawing_twice_changes_nothing() {
    // given: an open invitation
    let mut invitation = an_invitation_issued_at(1_000);

    // then: withdrawing it is decided as withdrawn
    assert_eq!(
        invitation.decide(InvitationCommand::Withdraw, &an_operator()),
        Ok(vec![InvitationEvent::Withdrawn])
    );
    // when: it is withdrawn
    invitation.apply(&InvitationEvent::Withdrawn, &stamped(2, clock::at(2_000)));

    // then: it is withdrawn
    assert_eq!(invitation.outcome(), Outcome::Withdrawn);
    // and: it is not taken up
    assert_eq!(
        invitation.decide(InvitationCommand::Accept, &an_operator()),
        Err(InvitationError::Withdrawn)
    );
    // and: withdrawing it again changes nothing
    assert_eq!(
        invitation.decide(InvitationCommand::Withdraw, &an_operator()),
        Ok(vec![])
    );
}

#[test]
fn an_invitation_taken_up_is_not_withdrawn() {
    // given: an invitation taken up
    let mut invitation = an_invitation_issued_at(1_000);
    invitation.apply(
        &InvitationEvent::Accepted {
            scope: Scope::Instance,
        },
        &stamped(2, clock::at(2_000)),
    );

    // then: withdrawing it is refused
    assert_eq!(
        invitation.decide(InvitationCommand::Withdraw, &an_operator()),
        Err(InvitationError::AlreadyAccepted),
        "the account it made stays; it is locked or deleted as any other"
    );
}

#[test]
fn an_expired_invitation_may_still_be_withdrawn() {
    // given: an invitation long expired
    let late = 1_000 + Duration::days(30).whole_seconds();
    let invitation = an_invitation_issued_at(1_000);

    // then: it is expired
    assert!(invitation.is_expired(clock::at(late)));
    // and: withdrawing it is still decided as withdrawn
    assert_eq!(
        invitation.decide(InvitationCommand::Withdraw, &an_operator()),
        Ok(vec![InvitationEvent::Withdrawn])
    );
}

#[test]
fn nothing_but_issuing_comes_first_and_issuing_comes_once() {
    // then: nothing but issuing begins an invitation
    for command in [InvitationCommand::Accept, InvitationCommand::Withdraw] {
        assert_eq!(
            Invitation::begin(command, &an_operator()),
            Err(InvitationError::NotIssuedYet)
        );
    }
    // and: an issued one is not issued again
    assert_eq!(
        an_invitation_issued_at(1_000).decide(
            InvitationCommand::Issue {
                to: ada(),
                digest: InvitationTokenDigest::kept("cd34".to_owned()),
                until: clock::at(2_000),
                scope: Scope::Instance,
                by_an_operator: true,
                origin: "https://weaveling.example.com".to_owned(),
            },
            &an_operator()
        ),
        Err(InvitationError::AlreadyIssued)
    );
    // and: no event but an issue gives birth to one
    for event in [
        InvitationEvent::Retired,
        InvitationEvent::Accepted {
            scope: Scope::Instance,
        },
        InvitationEvent::Withdrawn,
        InvitationEvent::Declined {
            reason: DeclineReason::AddressHasAccount,
        },
    ] {
        assert_eq!(
            Invitation::from_first(&event, &stamped(1, clock::at(1_000))),
            None
        );
    }
}

#[test]
fn a_snapshot_holds_everything_the_invitation_is() {
    // given: a withdrawn invitation
    let mut invitation = an_invitation_issued_at(1_000);
    invitation.apply(&InvitationEvent::Withdrawn, &stamped(2, clock::at(2_000)));

    // when: one is born of its snapshot and one behind has it applied
    let restored = Invitation::from_first(&invitation.snapshot(), &stamped(2, clock::at(3_000)))
        .expect("a snapshot gives birth");
    let mut behind = an_invitation_issued_at(500);
    behind.apply(&invitation.snapshot(), &stamped(3, clock::at(3_000)));

    // then: both are the invitation
    assert_eq!(restored, invitation);
    assert_eq!(behind, invitation);
}

#[test]
fn a_snapshot_keeps_its_scope_and_whether_an_operator_issued_it() {
    // given: an invitation into a project issued by no operator
    let into_a_project = InvitationEvent::Issued {
        to: ada(),
        digest: InvitationTokenDigest::kept("ab12".to_owned()),
        until: clock::at(1_000) + Invitation::LASTS,
        scope: Scope::Project {
            project: ProjectLink::from("project_1"),
            role: Role::Reader,
        },
        by_an_operator: false,
        origin: "https://weaveling.example.com".to_owned(),
    };
    let invitation = Invitation::from_first(&into_a_project, &stamped(1, clock::at(1_000)))
        .expect("issuing gives birth");
    // then: it is by no operator
    assert!(!invitation.is_by_an_operator());

    // when: one is born of its snapshot and one issued by an operator has it applied
    let restored = Invitation::from_first(&invitation.snapshot(), &stamped(2, clock::at(2_000)))
        .expect("a snapshot gives birth");
    let mut issued_otherwise = an_invitation_issued_at(500);
    assert!(issued_otherwise.is_by_an_operator());
    issued_otherwise.apply(&invitation.snapshot(), &stamped(2, clock::at(2_000)));

    // then: both keep its scope and that no operator issued it
    for told in [restored, issued_otherwise] {
        assert!(!told.is_by_an_operator());
        assert_eq!(told.scope(), invitation.scope());
    }
}

fn an_invitation_declined_at(seconds: i64) -> Invitation {
    let mut invitation = an_invitation_issued_at(seconds);
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
    invitation.apply(&declined[0], &stamped(2, clock::at(seconds + 10)));

    invitation
}

#[test]
fn an_invitation_to_an_address_with_an_account_is_declined_and_closed() {
    // given: an invitation declined for its address having an account
    let invitation = an_invitation_declined_at(1_000);

    // then: it is declined
    assert_eq!(
        invitation.outcome(),
        Outcome::Declined(DeclineReason::AddressHasAccount)
    );
    // and: declining it again changes nothing
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
    // and: withdrawing it changes nothing
    assert_eq!(
        invitation.decide(InvitationCommand::Withdraw, &an_operator()),
        Ok(vec![]),
        "an operator withdrawing what was declined is told nothing is left to do"
    );
    // and: it is not taken up
    assert_eq!(
        invitation.decide(InvitationCommand::Accept, &Agent::System),
        Err(InvitationError::Declined)
    );
}

#[test]
fn an_invitation_withdrawn_or_taken_up_is_not_declined() {
    // given: a withdrawn invitation
    let mut withdrawn = an_invitation_issued_at(1_000);
    withdrawn.apply(&InvitationEvent::Withdrawn, &stamped(2, clock::at(1_010)));
    // and: an accepted one
    let mut accepted = an_invitation_issued_at(1_000);
    accepted.apply(
        &InvitationEvent::Accepted {
            scope: Scope::Instance,
        },
        &stamped(2, clock::at(1_010)),
    );

    // then: the withdrawn one is not declined
    assert_eq!(
        withdrawn.decide(
            InvitationCommand::Decline {
                reason: DeclineReason::AddressHasAccount,
            },
            &Agent::System
        ),
        Err(InvitationError::Withdrawn)
    );
    // and: the accepted one is not declined
    assert_eq!(
        accepted.decide(
            InvitationCommand::Decline {
                reason: DeclineReason::AddressHasAccount,
            },
            &Agent::System
        ),
        Err(InvitationError::AlreadyAccepted)
    );
    // and: nothing not issued is declined
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
    // given: an open invitation
    let open = an_invitation_issued_at(1_000);
    // and: a withdrawn one
    let mut withdrawn = an_invitation_issued_at(1_000);
    withdrawn.apply(&InvitationEvent::Withdrawn, &stamped(2, clock::at(4_000)));

    // then: the open one was never settled
    assert_eq!(open.settled_at(), None);
    // and: the withdrawn one remembers when it was
    assert_eq!(withdrawn.settled_at(), Some(clock::at(4_000)));
}

#[test]
fn an_invitation_retires_once_however_it_stands() {
    // given: a withdrawn invitation
    let mut withdrawn = an_invitation_issued_at(1_000);
    withdrawn.apply(&InvitationEvent::Withdrawn, &stamped(2, clock::at(2_000)));

    // when: it, an open one and a declined one each retire
    for invitation in [
        an_invitation_issued_at(1_000),
        withdrawn,
        an_invitation_declined_at(1_000),
    ] {
        let retiring = invitation
            .decide(InvitationCommand::Retire, &Agent::System)
            .expect("any invitation retires");
        // then: retiring is decided
        assert_eq!(retiring, vec![InvitationEvent::Retired]);

        // and: once retired, it stands as it stood
        let mut retired = invitation.clone();
        retired.apply(&retiring[0], &stamped(3, clock::at(3_000)));
        assert!(retired.is_retired());
        // and: it keeps no address
        assert_eq!(retired.to().as_str(), "deleted@invitation.invalid");
        assert_eq!(
            Invitation::from_first(&retired.snapshot(), &stamped(4, clock::at(4_000)))
                .expect("a snapshot gives birth")
                .to()
                .as_str(),
            "deleted@invitation.invalid",
            "nor does its snapshot"
        );
        assert_eq!(
            retired.outcome(),
            invitation.outcome(),
            "retiring changes how it stood not at all"
        );
        // and: retiring again changes nothing
        assert_eq!(
            retired.decide(InvitationCommand::Retire, &Agent::System),
            Ok(vec![]),
            "retiring twice changes nothing"
        );
        // and: its snapshot keeps that it retired
        let restored = Invitation::from_first(&retired.snapshot(), &stamped(3, clock::at(4_000)))
            .expect("a snapshot gives birth");
        assert!(restored.is_retired(), "a snapshot keeps that it retired");
    }
    // and: nothing not issued retires
    assert_eq!(
        Invitation::begin(InvitationCommand::Retire, &Agent::System),
        Err(InvitationError::NotIssuedYet)
    );
}

#[test]
fn an_acceptance_tells_the_scope_it_was_into() {
    // given: an invitation into a project
    let into_a_project = Scope::Project {
        project: ProjectLink::from("project_1"),
        role: Role::Author,
    };
    let invitation = Invitation::from_first(
        &InvitationEvent::Issued {
            to: ada(),
            digest: InvitationTokenDigest::kept("ab12".to_owned()),
            until: clock::at(1_000) + Invitation::LASTS,
            scope: into_a_project.clone(),
            by_an_operator: false,
            origin: "https://weaveling.example.com".to_owned(),
        },
        &stamped(1, clock::at(1_000)),
    )
    .expect("issuing gives birth");

    // then: its acceptance tells the project and the role
    assert_eq!(
        invitation.decide(InvitationCommand::Accept, &Agent::System),
        Ok(vec![InvitationEvent::Accepted {
            scope: into_a_project
        }]),
        "members hear from the acceptance alone where to let the account join"
    );
}

#[test]
fn an_invitation_knows_which_account_issued_it_and_keeps_it_through_a_snapshot() {
    // given: an invitation issued by an account
    let by_an_account = an_invitation_issued_at(1_000);
    // and: one issued by the system
    let by_the_system = Invitation::from_first(
        &an_issue(1_000),
        &EventMetadata {
            agent: Agent::System,
            ..stamped(1, clock::at(1_000))
        },
    )
    .expect("issuing gives birth");

    // then: the first knows the account
    assert_eq!(
        by_an_account.issued_by(),
        Some(&AccountId::from("account_1"))
    );
    // and: the other none
    assert_eq!(by_the_system.issued_by(), None);
    // and: a snapshot keeps the account, born of it or applied
    let restored = Invitation::from_first(&by_an_account.snapshot(), &stamped(2, clock::at(2_000)))
        .expect("a snapshot gives birth");
    assert_eq!(restored.issued_by(), Some(&AccountId::from("account_1")));
    let mut behind = by_the_system;
    behind.apply(&by_an_account.snapshot(), &stamped(2, clock::at(2_000)));
    assert_eq!(behind.issued_by(), Some(&AccountId::from("account_1")));
}

#[test]
fn an_invitation_keeps_its_token_s_digest_and_through_a_snapshot() {
    // given: an invitation issued with its token's digest
    let invitation = Invitation::from_first(
        &InvitationEvent::Issued {
            to: ada(),
            digest: InvitationTokenDigest::kept("cd34".to_owned()),
            until: clock::at(1_000) + Invitation::LASTS,
            scope: Scope::Instance,
            by_an_operator: true,
            origin: "https://weaveling.example.com".to_owned(),
        },
        &stamped(1, clock::at(1_000)),
    )
    .expect("issuing gives birth");

    // then: it keeps the digest
    assert_eq!(invitation.digest().as_str(), "cd34");
    // and: so does a snapshot, born of it or applied
    let restored = Invitation::from_first(&invitation.snapshot(), &stamped(2, clock::at(2_000)))
        .expect("a snapshot gives birth");
    let mut behind = an_invitation_issued_at(500);
    behind.apply(&invitation.snapshot(), &stamped(2, clock::at(2_000)));
    for told in [restored, behind] {
        assert_eq!(told.digest().as_str(), "cd34");
    }
}
