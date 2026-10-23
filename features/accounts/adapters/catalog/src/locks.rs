use std::collections::BTreeMap;

use accounts_core::{CatalogError, Lock, LockKind, LockReason};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum StoredLockKind {
    Operator,
}

#[derive(Serialize, Deserialize)]
struct StoredLock {
    kind: StoredLockKind,
    reason: String,
    #[serde(with = "time::serde::rfc3339")]
    since: OffsetDateTime,
}

pub fn written(locks: &BTreeMap<LockKind, Lock>) -> String {
    let stored: Vec<StoredLock> = locks
        .iter()
        .map(|(kind, lock)| StoredLock {
            kind: match kind {
                LockKind::Operator => StoredLockKind::Operator,
            },
            reason: lock.reason.to_string(),
            since: lock.since,
        })
        .collect();

    serde_json::to_string(&stored).expect("locks are plain data and cannot fail to serialize")
}

pub fn read(written: &str) -> Result<BTreeMap<LockKind, Lock>, CatalogError> {
    let stored: Vec<StoredLock> =
        serde_json::from_str(written).map_err(|why| CatalogError::Backend(Box::new(why)))?;

    stored
        .into_iter()
        .map(
            |StoredLock {
                 kind,
                 reason,
                 since,
             }| {
                Ok((
                    match kind {
                        StoredLockKind::Operator => LockKind::Operator,
                    },
                    Lock {
                        reason: LockReason::new(&reason)
                            .map_err(|why| CatalogError::Backend(Box::new(why)))?,
                        since,
                    },
                ))
            },
        )
        .collect()
}
