use authorizing::Act;
use eventsourcing::{Agent, Standing};

use crate::asset::{Asset, AssetType, ProjectLink};
use crate::assets::AssetsCore;
use crate::catalog::{Upload, UploadMark, UploadsOf, UploadsSorted};
use crate::contents::Piece;
use crate::error::AssetsError;
use crate::id::AssetId;
use crate::storage::Listed;
use crate::uses::AssetUse;

/// The part of an asset that is handed out: a variant, as WebP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Content {
    pub bytes: Vec<u8>,
    pub content_type: &'static str,
}

const VARIANTS_ARE: &str = "image/webp";

impl AssetsCore {
    /// Lists the assets a project holds, the newest first, one page at a time.
    /// Given a type, it lists only the assets of that type.
    /// Whoever may read the project may list them, whoever uploaded them.
    pub async fn assets_in(
        &self,
        project: &str,
        of_type: Option<AssetType>,
        after: Option<&UploadMark>,
        at_most: u64,
        agent: &Agent,
    ) -> Result<Listed<Upload, UploadMark>, AssetsError> {
        let project = ProjectLink::from(project);
        self.authorized(agent, Act::Read, &project, || AssetsError::ProjectNotThere)
            .await?;
        let whose = UploadsOf::Project(project);

        let assets = transacting::retrying!(&*self.transactions, |transaction| {
            Ok::<_, AssetsError>(
                self.catalog
                    .uploads_of(
                        transaction,
                        &whose,
                        of_type,
                        UploadsSorted::Newest,
                        after,
                        at_most,
                    )
                    .await?,
            )
        })?;
        let last = assets.last().map(UploadMark::from);

        Ok(Listed::paged(assets, last, at_most))
    }

    /// Returns a variant of an asset, if the agent may see the asset.
    /// It is the narrowest variant at least as wide as asked, or else the widest.
    /// The widest is also returned when no width is asked for.
    /// The original is never returned.
    pub async fn content(
        &self,
        id: &str,
        width: Option<u32>,
        agent: &Agent,
    ) -> Result<Content, AssetsError> {
        let id: AssetId = id.parse()?;
        let asset = self.seen_by(&id, agent).await?.state;

        let variants = &asset.image().variants;
        let chosen = width
            .and_then(|asked| variants.iter().copied().filter(|each| *each >= asked).min())
            .or_else(|| variants.iter().copied().max())
            .ok_or(AssetsError::ContentsMissing)?;

        let bytes = self
            .contents
            .read(&id, Piece::Width(chosen))
            .await?
            .ok_or(AssetsError::ContentsMissing)?;

        Ok(Content {
            bytes,
            content_type: VARIANTS_ARE,
        })
    }

    /// Returns an asset and what it is used for, if the agent may see it.
    /// An asset the agent may not see is reported as not found.
    pub async fn asset(
        &self,
        id: &str,
        agent: &Agent,
    ) -> Result<(Standing<Asset>, Vec<AssetUse>), AssetsError> {
        let id: AssetId = id.parse()?;
        let standing = self.seen_by(&id, agent).await?;
        let uses = transacting::retrying!(&*self.transactions, |transaction| {
            Ok::<_, AssetsError>(self.uses.uses_of(transaction, &[id]).await?)
        })?;

        Ok((standing, uses.into_iter().map(|(_, used)| used).collect()))
    }
}
