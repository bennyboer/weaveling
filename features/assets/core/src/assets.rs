use std::sync::Arc;

use authorizing::{Act, Authorizer};
use clock::Clock;
use eventsourcing::{
    AccountId, Agent, AggregateId, EventSourcingService, EventStore, ServiceError, Standing,
    Version,
};
use time::OffsetDateTime;
use transacting::{Transaction, Transactions};

use crate::asset::{Asset, AssetCommand, AssetEvent, KIND, Owner, ProjectLink};
use crate::catalog::{AssetCatalog, AssetSummary};
use crate::contents::{Contents, Piece};
use crate::error::AssetsError;
use crate::id::AssetId;
use crate::images::{Images, Prepared};
use crate::limits::{Counted, Limits};
use crate::memberships::{Membership, MembershipCatalog};
use crate::preparing;

/// What is handed out of an asset: a variant, as WebP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Content {
    pub bytes: Vec<u8>,
    pub content_type: &'static str,
}

const VARIANTS_ARE: &str = "image/webp";

#[derive(Clone)]
pub struct AssetsCore {
    events: Arc<EventSourcingService<Asset>>,
    transactions: Arc<dyn Transactions>,
    catalog: Arc<dyn AssetCatalog>,
    memberships: Arc<dyn MembershipCatalog>,
    contents: Arc<dyn Contents>,
    images: Arc<dyn Images>,
    authorizer: Arc<dyn Authorizer>,
    clock: Arc<dyn Clock>,
    limits: Limits,
}

impl AssetsCore {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        store: Arc<dyn EventStore<AssetEvent>>,
        transactions: Arc<dyn Transactions>,
        catalog: Arc<dyn AssetCatalog>,
        memberships: Arc<dyn MembershipCatalog>,
        contents: Arc<dyn Contents>,
        images: Arc<dyn Images>,
        authorizer: Arc<dyn Authorizer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: Arc::new(EventSourcingService::new(store, clock.clone())),
            transactions,
            catalog,
            memberships,
            contents,
            images,
            authorizer,
            clock,
            limits: Limits::default(),
        }
    }

    pub fn limited(self, limits: Limits) -> Self {
        Self { limits, ..self }
    }

    pub fn limits(&self) -> Limits {
        self.limits
    }

    /// Takes an image into a project for whoever may write in it: refused past the upload's limit before anything is made of it, prepared beside the runtime, its pieces kept, and told uploaded only where every limit it counts against takes it, weighed in the transaction that tells it.
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

    /// Takes an image as the uploading account's own, as its picture is; counted against the installation and the account alone, and refused where nobody is signed in, as without accounts.
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
        let id = AssetId::generate(self.clock.now());
        if let Err(failed) = self.kept(&id, &uploaded, &prepared).await {
            self.contents.forget(&id).await?;
            return Err(failed);
        }

        if let Err(refused) = self.recorded(&id, owner, prepared, agent).await {
            self.contents.forget(&id).await?;
            return Err(refused);
        }

        Ok((id, self.events.latest(&AggregateId::from(&id)).await?))
    }

    /// The variant of an asset for whoever may see it: the narrowest at least as wide as asked, else the widest, which is also what is handed out asked for no width; never the original.
    pub async fn content(
        &self,
        id: &str,
        width: Option<u32>,
        agent: &Agent,
    ) -> Result<Content, AssetsError> {
        let id: AssetId = id.parse()?;
        let asset = self.standing(&id).await?.state;
        if asset.is_deleted() || !self.may_see(agent, asset.owner()).await? {
            return Err(not_found(&id));
        }

        let variants = &asset.picture().variants;
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

    /// Deletes an asset for whoever may write where it is, and then lets go of its pieces; deleting it again changes nothing.
    pub async fn delete(&self, id: &str, agent: &Agent) -> Result<(), AssetsError> {
        let id: AssetId = id.parse()?;
        let asset = self.standing(&id).await?.state;
        self.may_change(agent, asset.owner(), || not_found(&id))
            .await?;

        transacting::retrying!(&*self.transactions, |transaction| {
            self.events
                .execute_at(
                    transaction,
                    &AggregateId::from(&id),
                    None,
                    AssetCommand::Delete,
                    agent,
                )
                .await?;
            self.catalog.forget(transaction, &id).await?;

            Ok::<_, AssetsError>(())
        })?;

        Ok(self.contents.forget(&id).await?)
    }

    /// Remembers an account a member of a project, or no more, as the project's members told it at a version, so whom its own assets are shown to follows.
    pub async fn remember_membership(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        account: &AccountId,
        version: Version,
        member: bool,
    ) -> Result<(), AssetsError> {
        let membership = Membership {
            project: ProjectLink::from(project),
            account: account.clone(),
        };
        transacting::joining!(&*self.transactions, within, |transaction| {
            if member {
                self.memberships
                    .remember(transaction, &membership, version)
                    .await?;
            } else {
                self.memberships
                    .forget(transaction, &membership, version, self.clock.now())
                    .await?;
            }

            Ok::<_, AssetsError>(())
        })
    }

    /// Remembers a project's members disbanded, its project deleted: nobody shares it any more.
    pub async fn remember_disbanded(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        version: Version,
    ) -> Result<(), AssetsError> {
        let project = ProjectLink::from(project);
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.memberships
                .forget_project(transaction, &project, version, self.clock.now())
                .await?;

            Ok::<_, AssetsError>(())
        })
    }

    /// Lets go of what only guarded against a word about members heard late, once none may be any more.
    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, AssetsError> {
        transacting::retrying!(&*self.transactions, |transaction| {
            Ok::<_, AssetsError>(
                self.memberships
                    .forget_tombstones_laid_before(transaction, before, at_most)
                    .await?,
            )
        })
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
        prepared: Prepared,
        agent: &Agent,
    ) -> Result<(), AssetsError> {
        let total_size = prepared.total_size();
        let uploader = match agent {
            Agent::Account(account) => Some(account.clone()),
            _ => None,
        };
        let project = match &owner {
            Owner::Project(project) => Some(project.clone()),
            Owner::Account(_) => None,
        };
        let counted = Counted {
            by_an_account: uploader.is_some(),
            into_a_project: project.is_some(),
        };

        transacting::retrying!(&*self.transactions, |transaction| {
            let usage = self
                .catalog
                .usage_within(transaction, uploader.as_ref(), project.as_ref())
                .await?;
            if let Some(limit) =
                crate::limits::first_passed(self.limits.on_what_is_kept(usage, counted), total_size)
            {
                return Err(AssetsError::LimitReached(limit));
            }
            self.events
                .begin(
                    transaction,
                    &AggregateId::from(id),
                    AssetCommand::Upload {
                        owner: owner.clone(),
                        picture: prepared.picture.clone(),
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
                        uploader: uploader.clone(),
                        format: prepared.picture.format,
                        width: prepared.picture.width,
                        height: prepared.picture.height,
                        original_size: prepared.picture.original_size,
                        total_size,
                        uploaded_at: self.clock.now(),
                    },
                )
                .await?;

            Ok::<_, AssetsError>(())
        })
    }

    async fn standing(&self, id: &AssetId) -> Result<Standing<Asset>, AssetsError> {
        Ok(self.events.latest(&AggregateId::from(id)).await?)
    }

    /// Whether the agent may see an asset: a project's, by reading the project; an account's, by being the account or sharing a project with it.
    async fn may_see(&self, agent: &Agent, owner: &Owner) -> Result<bool, AssetsError> {
        match (owner, agent) {
            (Owner::Project(project), _) => Ok(self
                .authorizer
                .may(agent, Act::Read, project.as_str())
                .await?),
            (Owner::Account(account), Agent::Account(acting)) if acting == account => Ok(true),
            (Owner::Account(account), Agent::Account(acting)) => {
                transacting::retrying!(&*self.transactions, |transaction| {
                    Ok::<_, AssetsError>(
                        self.memberships
                            .share_a_project(transaction, acting, account)
                            .await?,
                    )
                })
            }
            (Owner::Account(_), _) => Ok(false),
        }
    }

    async fn may_change(
        &self,
        agent: &Agent,
        owner: &Owner,
        otherwise: impl FnOnce() -> AssetsError,
    ) -> Result<(), AssetsError> {
        match owner {
            Owner::Project(project) => self.authorized(agent, Act::Write, project, otherwise).await,
            Owner::Account(account) if matches!(agent, Agent::Account(acting) if acting == account) => {
                Ok(())
            }
            Owner::Account(_) => Err(otherwise()),
        }
    }

    async fn authorized(
        &self,
        agent: &Agent,
        act: Act,
        project: &ProjectLink,
        otherwise: impl FnOnce() -> AssetsError,
    ) -> Result<(), AssetsError> {
        if self.authorizer.may(agent, act, project.as_str()).await? {
            Ok(())
        } else if self
            .authorizer
            .may(agent, Act::Read, project.as_str())
            .await?
        {
            Err(AssetsError::NotAnAuthor)
        } else {
            Err(otherwise())
        }
    }
}

fn not_found(id: &AssetId) -> AssetsError {
    AssetsError::Events(ServiceError::NotFound {
        aggregate: AggregateId::from(id),
        kind: KIND,
    })
}
