use std::collections::BTreeSet;

use eventsourcing::{AccountId, AggregateId, ServiceError, Version};
use time::OffsetDateTime;
use transacting::Transaction;

use crate::asset::{Owner, ProjectLink};
use crate::assets::AssetsCore;
use crate::error::AssetsError;
use crate::id::AssetId;
use crate::uses::{Holder, LAST_VERSION, Purpose, SceneLink};

impl AssetsCore {
    /// Records an account's new picture, as the accounts feature reported it at a version.
    ///
    /// Only an image of the account's own can be its picture. Any other id is not recorded,
    /// and the account then shows no picture as far as the assets feature is concerned.
    /// `dropping` is the picture shown before, given only when the account asked for it to be deleted.
    /// It is deleted if it is the account's own and nothing uses it any more.
    /// Its stored bytes are removed later, by the chore that removes stray contents.
    pub async fn picture_changed(
        &self,
        within: Option<&mut Transaction>,
        account: &AccountId,
        version: Version,
        picture: &str,
        dropping: Option<&str>,
    ) -> Result<(), AssetsError> {
        let own = Owner::Account(account.clone());
        let shown: BTreeSet<AssetId> = self.image_of(&own, picture).await?.into_iter().collect();
        let dropped = match dropping {
            Some(previous) => self.image_of(&own, previous).await?,
            None => None,
        };
        let holder = Holder::Account(account.clone());

        transacting::joining!(&*self.transactions, within, |transaction| {
            self.uses
                .used_for(
                    transaction,
                    Purpose::AccountPicture,
                    &holder,
                    &shown,
                    version,
                    self.clock.now(),
                )
                .await?;
            if let Some(previous) = dropped {
                self.dropped_if_unused(transaction, &previous).await?;
            }

            Ok::<_, AssetsError>(())
        })
    }

    /// Records that an account shows no picture any more, as the accounts feature reported it at a version.
    /// `dropping` is the picture shown until now, given only when the account asked for it to be deleted.
    /// It is deleted if it is the account's own and nothing uses it any more.
    pub async fn picture_removed(
        &self,
        within: Option<&mut Transaction>,
        account: &AccountId,
        version: Version,
        dropping: Option<&str>,
    ) -> Result<(), AssetsError> {
        let dropped = match dropping {
            Some(picture) => {
                self.image_of(&Owner::Account(account.clone()), picture)
                    .await?
            }
            None => None,
        };
        let holder = Holder::Account(account.clone());

        transacting::joining!(&*self.transactions, within, |transaction| {
            self.uses
                .used_for(
                    transaction,
                    Purpose::AccountPicture,
                    &holder,
                    &BTreeSet::new(),
                    version,
                    self.clock.now(),
                )
                .await?;
            if let Some(picture) = dropped {
                self.dropped_if_unused(transaction, &picture).await?;
            }

            Ok::<_, AssetsError>(())
        })
    }

    /// Records which assets a scene holds, as the scenes feature reported it at a version of the scene's prose.
    ///
    /// Only the images of the scene's own project are kept as its uses. Any other id is not recorded,
    /// so a scene cannot keep an image of another project, or an account's own image, from being deleted.
    pub async fn scene_assets_changed(
        &self,
        within: Option<&mut Transaction>,
        scene: &SceneLink,
        project: &ProjectLink,
        assets: &[String],
        version: Version,
    ) -> Result<(), AssetsError> {
        let owner = Owner::Project(project.clone());
        let mut held = BTreeSet::new();
        for asset in assets {
            if let Some(image) = self.image_of(&owner, asset).await? {
                held.insert(image);
            }
        }

        transacting::joining!(&*self.transactions, within, |transaction| {
            self.uses
                .used_for(
                    transaction,
                    Purpose::SceneImage,
                    &Holder::Scene(scene.clone()),
                    &held,
                    version,
                    self.clock.now(),
                )
                .await?;

            Ok::<_, AssetsError>(())
        })
    }

    /// Ends the uses of a deleted scene's images for good.
    /// The tombstone laid at `at` keeps any later report about the scene from bringing them back.
    pub async fn scene_deleted(
        &self,
        within: Option<&mut Transaction>,
        scene: &SceneLink,
        at: OffsetDateTime,
    ) -> Result<(), AssetsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.uses
                .used_for(
                    transaction,
                    Purpose::SceneImage,
                    &Holder::Scene(scene.clone()),
                    &BTreeSet::new(),
                    LAST_VERSION,
                    at,
                )
                .await?;

            Ok::<_, AssetsError>(())
        })
    }

    /// Returns the asset with this id if it is an image of this owner that is not deleted.
    async fn image_of(&self, owner: &Owner, id: &str) -> Result<Option<AssetId>, AssetsError> {
        let Ok(id) = id.parse::<AssetId>() else {
            return Ok(None);
        };
        match self.events.latest(&AggregateId::from(&id)).await {
            Ok(standing) => {
                Ok((!standing.state.is_deleted() && standing.state.owner() == owner).then_some(id))
            }
            Err(ServiceError::NotFound { .. }) => Ok(None),
            Err(failed) => Err(failed.into()),
        }
    }
}
