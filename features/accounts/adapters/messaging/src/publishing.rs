use accounts_contract::{AccountEventDTO, decline_reason, lock_kind};
use accounts_core::{AccountEvent, DeclineReason, LockKind};
use eventsourcing::Recorded;
use messaging::Message;

pub fn message_for(happened: &Recorded<AccountEvent>) -> Option<Message> {
    eventpublishing::message_for(happened, body)
}

fn body(event: &AccountEvent) -> Option<AccountEventDTO> {
    Some(match event {
        AccountEvent::SignedUp { .. } => AccountEventDTO::SignedUp,
        AccountEvent::SignedUpByInvitation { invitation, .. } => {
            AccountEventDTO::SignedUpByInvitation {
                invitation: invitation.clone(),
            }
        }
        AccountEvent::EmailVerified => AccountEventDTO::EmailVerified,
        AccountEvent::PasswordReset => AccountEventDTO::PasswordReset,
        AccountEvent::BecameOperator => AccountEventDTO::BecameOperator,
        AccountEvent::StoppedBeingOperator => AccountEventDTO::StoppedBeingOperator,
        AccountEvent::Locked { kind, .. } => AccountEventDTO::Locked {
            kind: named(*kind).to_owned(),
        },
        AccountEvent::Unlocked { kind } => AccountEventDTO::Unlocked {
            kind: named(*kind).to_owned(),
        },
        AccountEvent::InvitationDeclined { invitation, reason } => {
            AccountEventDTO::InvitationDeclined {
                invitation: invitation.clone(),
                reason: declined_for(*reason).to_owned(),
            }
        }
        AccountEvent::Snapshotted { .. } => return None,
    })
}

fn declined_for(reason: DeclineReason) -> &'static str {
    match reason {
        DeclineReason::AddressHasAccount => decline_reason::ADDRESS_HAS_ACCOUNT,
    }
}

fn named(kind: LockKind) -> &'static str {
    match kind {
        LockKind::Operator => lock_kind::OPERATOR,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use accounts_contract::{
        BECAME_OPERATOR, EMAIL_VERIFIED, EVERY_ACCOUNT, INVITATION_DECLINED, LOCKED,
        PASSWORD_RESET, SIGNED_UP, SIGNED_UP_BY_INVITATION, STOPPED_BEING_OPERATOR, UNLOCKED,
    };
    use accounts_core::{AccountId, AccountName, EmailAddress, KIND, LockReason};
    use eventpublishing::{aggregate_in, published_in};
    use eventsourcing::{AggregateId, Event, EventMetadata, Version};
    use messaging::Subscription;
    use time::{Duration, OffsetDateTime};

    use super::*;

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
    }

    fn an_account() -> AccountId {
        AccountId::generate(at(1_000))
    }

    fn recorded(id: &AccountId, event: AccountEvent) -> Recorded<AccountEvent> {
        Recorded {
            metadata: EventMetadata {
                aggregate: AggregateId::from(id),
                kind: KIND,
                version: Version::of(1),
                agent: id.acting(),
                occurred_at: at(1_000),
                is_snapshot: event.is_snapshot(),
            },
            event,
        }
    }

    fn ada() -> (EmailAddress, AccountName) {
        (
            EmailAddress::new("ada@example.com").expect("a plain address"),
            AccountName::new("Ada Lovelace").expect("a plain name"),
        )
    }

    fn a_sign_up() -> AccountEvent {
        let (email, name) = ada();

        AccountEvent::SignedUp { email, name }
    }

    fn a_sign_up_by_invitation() -> AccountEvent {
        let (email, name) = ada();

        AccountEvent::SignedUpByInvitation {
            email,
            name,
            invitation: "invitation_1".to_owned(),
        }
    }

    fn a_lock() -> AccountEvent {
        AccountEvent::Locked {
            kind: LockKind::Operator,
            reason: LockReason::new("Sent spam to every member.").expect("a plain reason"),
        }
    }

    fn everything_worth_publishing() -> Vec<(AccountEvent, &'static str)> {
        vec![
            (a_sign_up(), SIGNED_UP),
            (a_sign_up_by_invitation(), SIGNED_UP_BY_INVITATION),
            (AccountEvent::EmailVerified, EMAIL_VERIFIED),
            (AccountEvent::PasswordReset, PASSWORD_RESET),
            (AccountEvent::BecameOperator, BECAME_OPERATOR),
            (AccountEvent::StoppedBeingOperator, STOPPED_BEING_OPERATOR),
            (a_lock(), LOCKED),
            (
                AccountEvent::Unlocked {
                    kind: LockKind::Operator,
                },
                UNLOCKED,
            ),
            (
                AccountEvent::InvitationDeclined {
                    invitation: "invitation_1".to_owned(),
                    reason: DeclineReason::AddressHasAccount,
                },
                INVITATION_DECLINED,
            ),
        ]
    }

    fn published(id: &AccountId, event: AccountEvent) -> Message {
        message_for(&recorded(id, event)).expect("this event should be published")
    }

    #[test]
    fn every_event_lands_on_the_routing_key_the_contract_declares() {
        let id = an_account();

        for (event, declared) in everything_worth_publishing() {
            assert_eq!(
                eventpublishing::routing_for(KIND, event.name()).to_string(),
                declared,
                "the key the library derives and the key subscribers bind to must not drift"
            );
            assert_eq!(published(&id, event).routing.to_string(), declared);
        }
    }

    #[test]
    fn the_subscription_the_contract_declares_is_the_one_the_library_derives() {
        assert_eq!(eventpublishing::everything_from(KIND), EVERY_ACCOUNT);
        assert!(
            everything_worth_publishing().into_iter().all(|(event, _)| {
                Subscription::parse(EVERY_ACCOUNT)
                    .expect("a declared pattern")
                    .covers(&eventpublishing::routing_for(KIND, event.name()))
            }),
            "a listener asking for every account must hear all of them"
        );
    }

    #[test]
    fn the_name_on_the_wire_is_the_name_of_the_event() {
        let id = an_account();

        for (event, _) in everything_worth_publishing() {
            let expected = event.name().as_str().to_owned();

            assert_eq!(published(&id, event).payload["event"]["name"], expected);
        }
    }

    #[test]
    fn nothing_personal_travels_with_a_message() {
        let id = an_account();

        for (event, _) in everything_worth_publishing() {
            let told = published(&id, event).payload.to_string();

            for personal in ["ada@example.com", "Ada Lovelace", "Sent spam"] {
                assert!(
                    !told.contains(personal),
                    "messages are kept and refused ones shown, so {personal} must not be in {told}"
                );
            }
        }
    }

    #[test]
    fn a_lock_and_an_unlock_say_whose_lock_it_is() {
        let id = an_account();

        let locked = published_in::<AccountEventDTO>(&published(&id, a_lock()))
            .expect("what we wrote must be readable");
        let unlocked = published_in::<AccountEventDTO>(&published(
            &id,
            AccountEvent::Unlocked {
                kind: LockKind::Operator,
            },
        ))
        .expect("what we wrote must be readable");

        assert_eq!(
            locked.event.body,
            AccountEventDTO::Locked {
                kind: "operator".to_owned()
            }
        );
        assert_eq!(
            unlocked.event.body,
            AccountEventDTO::Unlocked {
                kind: "operator".to_owned()
            }
        );
    }

    #[test]
    fn a_sign_up_by_invitation_says_which_invitation_was_taken_up() {
        let told = eventpublishing::published_in::<AccountEventDTO>(&published(
            &an_account(),
            a_sign_up_by_invitation(),
        ))
        .expect("what we wrote must be readable");

        assert_eq!(
            told.event.body,
            AccountEventDTO::SignedUpByInvitation {
                invitation: "invitation_1".to_owned()
            },
            "invitations hear from it that the invitation was accepted"
        );
    }

    #[test]
    fn a_declined_invitation_says_which_invitation_it_was_and_why() {
        let told = eventpublishing::published_in::<AccountEventDTO>(&published(
            &an_account(),
            AccountEvent::InvitationDeclined {
                invitation: "invitation_1".to_owned(),
                reason: DeclineReason::AddressHasAccount,
            },
        ))
        .expect("what we wrote must be readable");

        assert_eq!(
            told.event.body,
            AccountEventDTO::InvitationDeclined {
                invitation: "invitation_1".to_owned(),
                reason: "address-has-account".to_owned(),
            },
            "invitations hear from it whom to tell the operator about and why, and nothing personal"
        );
    }

    #[test]
    fn a_snapshot_is_not_published_at_all() {
        let id = an_account();
        let (email, name) = ada();
        let snapshot = AccountEvent::Snapshotted {
            email,
            name,
            verified: true,
            signed_up_at: at(1_000),
            sessions_since: None,
            operator: false,
            locks: BTreeMap::new(),
        };

        assert!(message_for(&recorded(&id, snapshot)).is_none());
    }

    #[test]
    fn the_account_can_be_read_back_out_of_a_message() {
        let id = an_account();

        assert_eq!(
            aggregate_in::<AccountId>(&published(&id, a_sign_up()))
                .expect("what we wrote must be readable"),
            id
        );
    }
}
