use accounts_contract::{
    AccountSummaryDTO, DeletionDTO, KeyDTO, LockDTO, deletion_asked_by, key_role, lock_kind,
};
use accounts_core::{
    AccountSummary, DeletionAskedBy, Key, KeyId, KeyRole, LockKind, PendingDeletion,
};
use clock::text;

pub(crate) fn to_key_dto(key: &KeyId, held: &Key) -> KeyDTO {
    KeyDTO {
        key: key.to_string(),
        name: held.name.to_string(),
        role: match held.role {
            KeyRole::Primary => key_role::PRIMARY,
            KeyRole::SecondFactor => key_role::SECOND_FACTOR,
        }
        .to_owned(),
        since: text::serialize(held.since),
    }
}

pub(crate) fn to_summary_dto(summary: &AccountSummary) -> AccountSummaryDTO {
    AccountSummaryDTO {
        deletion: summary.deletion.map(to_deletion_dto),
        second_factor: summary.second_factor,
        id: summary.id.to_string(),
        version: summary.version.count(),
        name: summary.name.to_string(),
        email: summary.email.to_string(),
        verified: summary.verified,
        operator: summary.operator,
        signed_up_at: text::serialize(summary.signed_up_at),
        locks: summary
            .locks
            .iter()
            .map(|(kind, lock)| LockDTO {
                kind: match kind {
                    LockKind::Operator => lock_kind::OPERATOR,
                }
                .to_owned(),
                reason: lock.reason.to_string(),
                since: text::serialize(lock.since),
            })
            .collect(),
    }
}

pub(crate) fn to_deletion_dto(pending: PendingDeletion) -> DeletionDTO {
    DeletionDTO {
        until: text::serialize(pending.until),
        by: match pending.asked_by {
            DeletionAskedBy::Holder => deletion_asked_by::HOLDER,
            DeletionAskedBy::Operator => deletion_asked_by::OPERATOR,
        }
        .to_owned(),
    }
}
