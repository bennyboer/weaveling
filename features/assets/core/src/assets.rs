mod deleting;
mod heard;
mod in_use;
mod operating;
mod own_uploads;
mod reading;
mod uploading;

use std::sync::Arc;

use access::KeptAccess;
use authorizing::{Act, Authorizer};
use clock::Clock;
use eventsourcing::{
    AccountId, Agent, AggregateId, EventSourcingService, EventStore, ServiceError, Standing,
};
use transacting::{Transaction, Transactions};

use crate::account_limits::AccountLimits;
use crate::asset::{Asset, AssetCommand, AssetEvent, KIND, Owner, ProjectLink};
use crate::catalog::AssetCatalog;
use crate::contents::Contents;
use crate::error::AssetsError;
use crate::id::AssetId;
use crate::images::Images;
use crate::limits::Limits;
use crate::uses::AssetUses;

pub use deleting::SWEPT_AT_ONCE;
pub use own_uploads::OwnUsage;
pub use reading::Content;

/// The assets feature's only way in.
/// Its methods are grouped by job in the modules beside this one:
/// uploading, reading, deleting, operating, own uploads, what the feature heard, and what holds an asset in use.
#[derive(Clone)]
pub struct AssetsCore {
    events: Arc<EventSourcingService<Asset>>,
    transactions: Arc<dyn Transactions>,
    catalog: Arc<dyn AssetCatalog>,
    access: KeptAccess,
    account_limits: Arc<dyn AccountLimits>,
    contents: Arc<dyn Contents>,
    images: Arc<dyn Images>,
    authorizer: Arc<dyn Authorizer>,
    uses: Arc<dyn AssetUses>,
    clock: Arc<dyn Clock>,
    limits: Limits,
}

impl AssetsCore {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        store: Arc<dyn EventStore<AssetEvent>>,
        transactions: Arc<dyn Transactions>,
        catalog: Arc<dyn AssetCatalog>,
        access: KeptAccess,
        account_limits: Arc<dyn AccountLimits>,
        contents: Arc<dyn Contents>,
        images: Arc<dyn Images>,
        authorizer: Arc<dyn Authorizer>,
        uses: Arc<dyn AssetUses>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: Arc::new(EventSourcingService::new(store, clock.clone())),
            transactions,
            catalog,
            access,
            account_limits,
            contents,
            images,
            authorizer,
            uses,
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

    /// Returns the limits an upload by the account must fit.
    /// If an operator gave the account its own limit, it replaces the default limit for accounts.
    /// This only applies if accounts have a limit at all.
    async fn limits_of(
        &self,
        transaction: &mut Transaction,
        uploader: Option<&AccountId>,
    ) -> Result<Limits, AssetsError> {
        let (Some(account), Some(_)) = (uploader, self.limits.each_account) else {
            return Ok(self.limits);
        };

        Ok(
            match self.account_limits.limit_of(transaction, account).await? {
                Some(own) => Limits {
                    each_account: Some(own),
                    ..self.limits
                },
                None => self.limits,
            },
        )
    }

    /// Records an asset as deleted by the agent, in the given transaction, and removes it from the catalog.
    /// Its stored pieces are left alone, since they can only be removed after the transaction has committed.
    async fn record_deleted(
        &self,
        transaction: &mut Transaction,
        id: &AssetId,
        agent: &Agent,
    ) -> Result<(), AssetsError> {
        self.events
            .execute_at(
                transaction,
                &AggregateId::from(id),
                None,
                AssetCommand::Delete,
                agent,
            )
            .await?;
        self.catalog.forget(transaction, id).await?;

        Ok(())
    }

    async fn standing(&self, id: &AssetId) -> Result<Standing<Asset>, AssetsError> {
        Ok(self.events.latest(&AggregateId::from(id)).await?)
    }

    /// Returns an asset as it stands, if it is not deleted and the agent may see it.
    /// Otherwise the asset is not found, so nobody learns of an asset they may not see.
    async fn seen_by(&self, id: &AssetId, agent: &Agent) -> Result<Standing<Asset>, AssetsError> {
        let standing = self.standing(id).await?;
        if standing.state.is_deleted() || !self.may_see(agent, standing.state.owner()).await? {
            return Err(not_found(id));
        }

        Ok(standing)
    }

    /// Whether the agent may see an asset.
    /// A project's asset may be seen by whoever may read the project.
    /// An account's asset may be seen by the account itself and by those who share a project with it.
    async fn may_see(&self, agent: &Agent, owner: &Owner) -> Result<bool, AssetsError> {
        match (owner, agent) {
            (Owner::Project(project), _) => Ok(self
                .authorizer
                .may(agent, Act::Read, project.as_str())
                .await?),
            (Owner::Account(account), Agent::Account(acting)) if acting == account => Ok(true),
            (Owner::Account(account), Agent::Account(acting)) => {
                Ok(self.access.share_a_project(acting, account).await?)
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
