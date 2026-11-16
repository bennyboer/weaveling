use eventsourcing::Agent;
use time::OffsetDateTime;
use transacting::Transaction;

use crate::asset::Owner;
use crate::assets::AssetsCore;
use crate::error::AssetsError;
use crate::id::AssetId;

/// How many assets a sweep deletes in one transaction.
pub const SWEPT_AT_ONCE: u64 = 50;

/// How many assets with stored pieces are checked against the catalog at once.
const STRAYS_CHECKED_AT_ONCE: u64 = 200;

impl AssetsCore {
    /// Deletes an asset, if the agent may write where it is.
    /// An asset still in use is not deleted, whoever asks: the places using it would show a hole.
    /// Its stored pieces are removed afterwards.
    /// Deleting it again changes nothing.
    pub async fn delete(&self, id: &str, agent: &Agent) -> Result<(), AssetsError> {
        let id: AssetId = id.parse()?;
        let asset = self.standing(&id).await?.state;
        self.may_change(agent, asset.owner(), || crate::assets::not_found(&id))
            .await?;

        transacting::retrying!(&*self.transactions, |transaction| {
            if !self.uses.uses_of(transaction, &[id]).await?.is_empty() {
                return Err(AssetsError::InUse);
            }
            self.record_deleted(transaction, &id, agent).await?;

            Ok::<_, AssetsError>(())
        })?;

        Ok(self.contents.forget(&id).await?)
    }

    /// Deletes every asset of an owner, used when a project or an account is deleted.
    ///
    /// The deletions are made by the system, so no permission is checked.
    /// They run in batches of `SWEPT_AT_ONCE`, each batch in its own transaction.
    /// A batch's stored bytes are removed only after its transaction has committed.
    /// Returns how many assets were deleted.
    pub async fn sweep(&self, owner: &Owner) -> Result<u64, AssetsError> {
        let mut swept = 0;
        loop {
            let deleted = transacting::retrying!(&*self.transactions, |transaction| {
                let owned = self
                    .catalog
                    .owned_by(transaction, owner, SWEPT_AT_ONCE)
                    .await?;
                for id in &owned {
                    self.record_deleted(transaction, id, &Agent::System).await?;
                }

                Ok::<_, AssetsError>(owned)
            })?;
            for id in &deleted {
                self.contents.forget(id).await?;
            }
            swept += deleted.len() as u64;
            if (deleted.len() as u64) < SWEPT_AT_ONCE {
                return Ok(swept);
            }
        }
    }

    /// Removes stored pieces that belong to no asset any more.
    ///
    /// Such pieces are left behind when the process stops between deleting an asset and removing its bytes.
    /// They are also left by an upload that failed half way.
    /// Only pieces of assets whose id was made before `made_before` are removed.
    /// That keeps an upload in progress safe, because its pieces are stored before the asset is recorded.
    /// Returns how many assets' pieces were removed.
    pub async fn forget_stray_contents(
        &self,
        made_before: OffsetDateTime,
    ) -> Result<u64, AssetsError> {
        let mut forgotten = 0;
        let mut after: Option<AssetId> = None;
        loop {
            let stored = self
                .contents
                .stored(after.as_ref(), STRAYS_CHECKED_AT_ONCE)
                .await?;
            let old: Vec<AssetId> = stored
                .iter()
                .filter(|id| made_at(id).is_some_and(|made| made < made_before))
                .copied()
                .collect();
            let held = transacting::retrying!(&*self.transactions, |transaction| {
                Ok::<_, AssetsError>(self.catalog.held_of(transaction, &old).await?)
            })?;
            for stray in old.iter().filter(|id| !held.contains(id)) {
                self.contents.forget(stray).await?;
                forgotten += 1;
            }
            if (stored.len() as u64) < STRAYS_CHECKED_AT_ONCE {
                return Ok(forgotten);
            }
            after = stored.last().copied();
        }
    }

    /// Deletes an asset as the system, in the given transaction, unless something still uses it.
    /// Its stored bytes are left to the chore that removes stray contents.
    pub(super) async fn dropped_if_unused(
        &self,
        transaction: &mut Transaction,
        id: &AssetId,
    ) -> Result<(), AssetsError> {
        if !self.uses.uses_of(transaction, &[*id]).await?.is_empty() {
            return Ok(());
        }

        self.record_deleted(transaction, id, &Agent::System).await
    }
}

/// Returns when an asset's id was made, which is just before its pieces were stored.
fn made_at(id: &AssetId) -> Option<OffsetDateTime> {
    let (seconds, nanoseconds) = id.as_uuid().get_timestamp()?.to_unix();
    let made = OffsetDateTime::from_unix_timestamp(i64::try_from(seconds).ok()?).ok()?;

    Some(made + time::Duration::nanoseconds(i64::from(nanoseconds)))
}
