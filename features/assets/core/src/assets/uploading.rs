use authorizing::Act;
use eventsourcing::{AccountId, Agent, AggregateId, Standing};
use transacting::Transaction;

use crate::asset::{Asset, AssetCommand, Owner, ProjectLink};
use crate::assets::AssetsCore;
use crate::catalog::AssetSummary;
use crate::contents::Piece;
use crate::error::AssetsError;
use crate::id::AssetId;
use crate::images::Prepared;
use crate::limits::Counted;
use crate::preparing;

/// Whose bounds an upload counts against, besides the installation:
/// the uploading account's, if an account uploads, and the project's it goes into, if it goes into one.
struct CountedAgainst {
    uploader: Option<AccountId>,
    project: Option<ProjectLink>,
}

impl CountedAgainst {
    fn of(owner: &Owner, agent: &Agent) -> Self {
        Self {
            uploader: match agent {
                Agent::Account(account) => Some(account.clone()),
                _ => None,
            },
            project: match owner {
                Owner::Project(project) => Some(project.clone()),
                Owner::Account(_) => None,
            },
        }
    }

    fn counted(&self) -> Counted {
        Counted {
            by_an_account: self.uploader.is_some(),
            into_a_project: self.project.is_some(),
        }
    }
}

impl AssetsCore {
    /// Uploads an image into a project.
    /// The agent must be allowed to write in the project.
    /// An upload larger than the upload limit is refused before anything is made from it.
    /// The image is prepared outside the async runtime, and its pieces are stored.
    /// It is recorded as uploaded only if it fits every limit it counts against.
    /// Those limits are checked before any piece is stored, and again in the transaction that records the upload.
    pub async fn upload_into(
        &self,
        project: &str,
        uploaded: Vec<u8>,
        agent: &Agent,
    ) -> Result<(AssetId, Standing<Asset>), AssetsError> {
        let project = ProjectLink::from(project);
        self.authorized(agent, Act::Write, &project, || AssetsError::ProjectNotThere)
            .await?;

        self.uploaded(Owner::Project(project), uploaded, agent)
            .await
    }

    /// Uploads an image as the uploading account's own, like its picture.
    /// It counts only against the installation's and the account's limits.
    /// It is refused if nobody is signed in, which is always the case without accounts.
    pub async fn upload_as_own(
        &self,
        uploaded: Vec<u8>,
        agent: &Agent,
    ) -> Result<(AssetId, Standing<Asset>), AssetsError> {
        let Agent::Account(account) = agent else {
            return Err(AssetsError::NoAccount);
        };

        self.uploaded(Owner::Account(account.clone()), uploaded, agent)
            .await
    }

    /// Copies an image into a project, so the project holds it as its own.
    /// The agent must be allowed to see the image and to write in the project.
    /// The copy is made from the image's original as an upload would be, and counts against the same limits.
    pub async fn copy_into(
        &self,
        id: &str,
        project: &str,
        agent: &Agent,
    ) -> Result<(AssetId, Standing<Asset>), AssetsError> {
        let id: AssetId = id.parse()?;
        self.seen_by(&id, agent).await?;
        let project = ProjectLink::from(project);
        self.authorized(agent, Act::Write, &project, || AssetsError::ProjectNotThere)
            .await?;
        let original = self
            .contents
            .read(&id, Piece::Original)
            .await?
            .ok_or(AssetsError::ContentsMissing)?;

        self.uploaded(Owner::Project(project), original, agent)
            .await
    }

    /// Prepares an upload, stores its pieces, and records it as uploaded.
    ///
    /// The limits are checked once before any piece is stored, so an upload that cannot fit stores nothing at all.
    /// They are checked again in the transaction that records the upload,
    /// because another upload may have taken the room in between.
    async fn uploaded(
        &self,
        owner: Owner,
        uploaded: Vec<u8>,
        agent: &Agent,
    ) -> Result<(AssetId, Standing<Asset>), AssetsError> {
        if let Some(limit) = self.limits.on_the_upload()
            && !limit.takes(uploaded.len() as u64)
        {
            return Err(AssetsError::LimitReached(limit));
        }

        let images = self.images.clone();
        let (uploaded, prepared) = preparing::aside(move || {
            images
                .prepared(&uploaded)
                .map(|prepared| (uploaded, prepared))
        })
        .await?;
        let counted_against = CountedAgainst::of(&owner, agent);
        let total_size = prepared.total_size();
        transacting::retrying!(&*self.transactions, |transaction| {
            self.within_limits(transaction, &counted_against, total_size)
                .await
        })?;

        let id = AssetId::generate(self.clock.now());
        if let Err(failed) = self.kept(&id, &uploaded, &prepared).await {
            self.contents.forget(&id).await?;
            return Err(failed);
        }

        if let Err(refused) = self
            .recorded(&id, owner, &counted_against, prepared, agent)
            .await
        {
            self.contents.forget(&id).await?;
            return Err(refused);
        }

        Ok((id, self.events.latest(&AggregateId::from(&id)).await?))
    }

    /// Refuses an upload of this many bytes if it would pass a limit, as the catalog stands in this transaction.
    /// The installation's limit is checked first.
    async fn within_limits(
        &self,
        transaction: &mut Transaction,
        counted_against: &CountedAgainst,
        adding: u64,
    ) -> Result<(), AssetsError> {
        let uploader = counted_against.uploader.as_ref();
        let usage = self
            .catalog
            .usage_within(transaction, uploader, counted_against.project.as_ref())
            .await?;
        let limits = self.limits_of(transaction, uploader).await?;

        match crate::limits::first_passed(
            limits.on_what_is_kept(usage, counted_against.counted()),
            adding,
        ) {
            Some(limit) => Err(AssetsError::LimitReached(limit)),
            None => Ok(()),
        }
    }

    async fn kept(
        &self,
        id: &AssetId,
        uploaded: &[u8],
        prepared: &Prepared,
    ) -> Result<(), AssetsError> {
        self.contents.keep(id, Piece::Original, uploaded).await?;
        for (width, bytes) in &prepared.variants {
            self.contents.keep(id, Piece::Width(*width), bytes).await?;
        }

        Ok(())
    }

    async fn recorded(
        &self,
        id: &AssetId,
        owner: Owner,
        counted_against: &CountedAgainst,
        prepared: Prepared,
        agent: &Agent,
    ) -> Result<(), AssetsError> {
        let total_size = prepared.total_size();

        transacting::retrying!(&*self.transactions, |transaction| {
            self.within_limits(transaction, counted_against, total_size)
                .await?;
            self.events
                .begin(
                    transaction,
                    &AggregateId::from(id),
                    AssetCommand::Upload {
                        owner: owner.clone(),
                        image: prepared.image.clone(),
                        total_size,
                    },
                    agent,
                )
                .await?;
            self.catalog
                .remember(
                    transaction,
                    &AssetSummary {
                        id: *id,
                        owner: owner.clone(),
                        uploader: counted_against.uploader.clone(),
                        format: prepared.image.format,
                        width: prepared.image.width,
                        height: prepared.image.height,
                        original_size: prepared.image.original_size,
                        total_size,
                        uploaded_at: self.clock.now(),
                    },
                )
                .await?;

            Ok::<_, AssetsError>(())
        })
    }
}
