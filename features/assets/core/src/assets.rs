use std::collections::HashMap;
use std::sync::Arc;

use access::{HearsAccess, HearsOperators, KeptAccess, Role};
use async_trait::async_trait;
use authorizing::{Act, Authorizer};
use clock::Clock;
use eventsourcing::{
    AccountId, Agent, AggregateId, EventSourcingService, EventStore, ServiceError, Standing,
    Version,
};
use time::OffsetDateTime;
use transacting::{Transaction, Transactions};

use crate::account_limits::AccountLimits;
use crate::asset::{Asset, AssetCommand, AssetEvent, KIND, Owner, ProjectLink};
use crate::catalog::{
    AssetCatalog, AssetSummary, Held, Holders, Person, Sorted, UploadMark, UploadsSorted,
};
use crate::contents::{Contents, Piece};
use crate::error::AssetsError;
use crate::id::AssetId;
use crate::images::{Images, Prepared};
use crate::limits::{Counted, Limits, MOST_COUNTED};
use crate::preparing;
use crate::storage::{
    AccountStorage, InstanceStorage, Listed, OwnUpload, ProjectOwner, ProjectStorage,
};
use crate::uses::{AssetUse, AssetUses, Holder, Purpose};

/// How many bytes an account's uploads use together, and their limit.
/// The limit is missing if there is none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnUsage {
    pub used: u64,
    pub most: Option<u64>,
}

/// The part of an asset that is handed out: a variant, as WebP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Content {
    pub bytes: Vec<u8>,
    pub content_type: &'static str,
}

const VARIANTS_ARE: &str = "image/webp";

/// How many assets a sweep deletes in one transaction.
pub const SWEPT_AT_ONCE: u64 = 50;

/// How many assets with stored pieces are checked against the catalog at once.
const STRAYS_CHECKED_AT_ONCE: u64 = 200;

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

    /// Uploads an image into a project.
    /// The agent must be allowed to write in the project.
    /// An upload larger than the upload limit is refused before anything is made from it.
    /// The image is prepared outside the async runtime, and its pieces are stored.
    /// It is recorded as uploaded only if it fits every limit it counts against.
    /// Those limits are checked in the same transaction that records the upload.
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

    /// Deletes an asset, if the agent may write where it is.
    /// An asset still in use is not deleted, whoever asks: the places using it would show a hole.
    /// Its stored pieces are removed afterwards.
    /// Deleting it again changes nothing.
    pub async fn delete(&self, id: &str, agent: &Agent) -> Result<(), AssetsError> {
        let id: AssetId = id.parse()?;
        let asset = self.standing(&id).await?.state;
        self.may_change(agent, asset.owner(), || not_found(&id))
            .await?;

        transacting::retrying!(&*self.transactions, |transaction| {
            if !self.uses.uses_of(transaction, &[id]).await?.is_empty() {
                return Err(AssetsError::InUse);
            }
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

    /// Returns an asset and what it is used for, if the agent may see it.
    /// An asset the agent may not see is reported as not found.
    pub async fn asset(
        &self,
        id: &str,
        agent: &Agent,
    ) -> Result<(Standing<Asset>, Vec<AssetUse>), AssetsError> {
        let id: AssetId = id.parse()?;
        let standing = self.standing(&id).await?;
        if standing.state.is_deleted() || !self.may_see(agent, standing.state.owner()).await? {
            return Err(not_found(&id));
        }
        let uses = transacting::retrying!(&*self.transactions, |transaction| {
            Ok::<_, AssetsError>(self.uses.uses_of(transaction, &[id]).await?)
        })?;

        Ok((standing, uses.into_iter().map(|(_, used)| used).collect()))
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
                    self.events
                        .execute_at(
                            transaction,
                            &AggregateId::from(id),
                            None,
                            AssetCommand::Delete,
                            &Agent::System,
                        )
                        .await?;
                    self.catalog.forget(transaction, id).await?;
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

    /// Gives an account its own storage limit, replacing the default limit for accounts.
    /// Only an operator may do this.
    pub async fn give_account_limit(
        &self,
        account: &str,
        most: u64,
        agent: &Agent,
    ) -> Result<(), AssetsError> {
        self.operating(agent).await?;
        if most > MOST_COUNTED {
            return Err(AssetsError::LimitUncountable);
        }
        let account = AccountId::from(account);

        transacting::retrying!(&*self.transactions, |transaction| {
            self.account_limits
                .give(transaction, &account, most)
                .await?;

            Ok::<_, AssetsError>(())
        })
    }

    /// Removes an account's own storage limit, so the default limit for accounts applies again.
    /// Only an operator may do this.
    pub async fn take_back_account_limit(
        &self,
        account: &str,
        agent: &Agent,
    ) -> Result<(), AssetsError> {
        self.operating(agent).await?;
        let account = AccountId::from(account);

        transacting::retrying!(&*self.transactions, |transaction| {
            self.account_limits.take_back(transaction, &account).await?;

            Ok::<_, AssetsError>(())
        })
    }

    /// Returns how much storage the whole instance uses, and its limit.
    /// Only an operator may ask.
    pub async fn instance_storage(&self, agent: &Agent) -> Result<InstanceStorage, AssetsError> {
        self.operating(agent).await?;

        transacting::retrying!(&*self.transactions, |transaction| {
            let usage = self.catalog.usage_within(transaction, None, None).await?;

            Ok::<_, AssetsError>(InstanceStorage {
                used: usage.instance,
                most: self.limits.instance,
            })
        })
    }

    /// Lists how much storage each account uses, with its limit, one page at a time.
    /// Only accounts that uploaded something are listed.
    /// With a search term, only accounts whose name or email contains it are listed.
    /// Only an operator may ask.
    pub async fn accounts_storage(
        &self,
        sorted: Sorted,
        after: Option<&Held>,
        matching: Option<&str>,
        at_most: u64,
        agent: &Agent,
    ) -> Result<Listed<AccountStorage>, AssetsError> {
        self.operating(agent).await?;

        transacting::retrying!(&*self.transactions, |transaction| {
            let held = self
                .catalog
                .held_by(
                    transaction,
                    Holders::Accounts,
                    sorted,
                    after,
                    searched(matching),
                    at_most,
                )
                .await?;
            let mut listed = Vec::with_capacity(held.len());
            for holding in &held {
                let account = AccountId::from(holding.id.as_str());
                let own = match self.limits.each_account {
                    Some(_) => self.account_limits.limit_of(transaction, &account).await?,
                    None => None,
                };
                listed.push(AccountStorage {
                    account,
                    name: holding.name.clone(),
                    email: holding.email.clone(),
                    used: holding.total,
                    most: own.or(self.limits.each_account),
                    own_limit: own.is_some(),
                });
            }

            Ok::<_, AssetsError>(Listed::paged(listed, held.last().cloned(), at_most))
        })
    }

    /// Lists how much storage each project uses, with its limit and its owners, one page at a time.
    /// Only projects that hold something are listed.
    /// With a search term, only projects whose name contains it are listed.
    /// Only an operator may ask.
    pub async fn projects_storage(
        &self,
        sorted: Sorted,
        after: Option<&Held>,
        matching: Option<&str>,
        at_most: u64,
        agent: &Agent,
    ) -> Result<Listed<ProjectStorage>, AssetsError> {
        self.operating(agent).await?;

        let held = transacting::retrying!(&*self.transactions, |transaction| {
            Ok::<_, AssetsError>(
                self.catalog
                    .held_by(
                        transaction,
                        Holders::Projects,
                        sorted,
                        after,
                        searched(matching),
                        at_most,
                    )
                    .await?,
            )
        })?;
        let mut owners = Vec::with_capacity(held.len());
        for holding in &held {
            owners.push(self.access.owners_of(&holding.id).await?);
        }
        let everyone: Vec<AccountId> = owners.iter().flatten().cloned().collect();
        let people = transacting::retrying!(&*self.transactions, |transaction| {
            Ok::<_, AssetsError>(self.catalog.people(transaction, &everyone).await?)
        })?;
        let owner = |account: AccountId| {
            let known = people.iter().find(|person| person.account == account);
            ProjectOwner {
                name: known.map(|person| person.name.clone()),
                email: known.map(|person| person.email.clone()),
                account,
            }
        };
        let listed = held
            .iter()
            .zip(owners)
            .map(|(holding, owners)| ProjectStorage {
                project: ProjectLink::from(holding.id.as_str()),
                name: holding.name.clone(),
                used: holding.total,
                most: self.limits.each_project,
                owners: owners.into_iter().map(owner).collect(),
            })
            .collect();

        Ok(Listed::paged(listed, held.last().cloned(), at_most))
    }

    /// Lists everything the signed-in account uploaded, one page at a time.
    /// This includes uploads into projects and the account's own images.
    /// Each entry says whether the account may delete it.
    /// Fails with `NoAccount` when nobody is signed in, which is always the case without accounts.
    pub async fn uploads_of(
        &self,
        sorted: UploadsSorted,
        after: Option<&UploadMark>,
        at_most: u64,
        agent: &Agent,
    ) -> Result<Listed<OwnUpload, UploadMark>, AssetsError> {
        let Agent::Account(account) = agent else {
            return Err(AssetsError::NoAccount);
        };

        let (uploads, uses) = transacting::retrying!(&*self.transactions, |transaction| {
            let uploads = self
                .catalog
                .uploaded_by(transaction, account, sorted, after, at_most)
                .await?;
            let ids: Vec<AssetId> = uploads.iter().map(|upload| upload.id).collect();
            let uses = self.uses.uses_of(transaction, &ids).await?;

            Ok::<_, AssetsError>((uploads, uses))
        })?;
        let mut writable: HashMap<ProjectLink, bool> = HashMap::new();
        let mut listed = Vec::with_capacity(uploads.len());
        for upload in uploads {
            let deletable = match &upload.owner {
                Owner::Account(owner) => owner == account,
                Owner::Project(project) => match writable.get(project) {
                    Some(may) => *may,
                    None => {
                        let may = self
                            .authorizer
                            .may(agent, Act::Write, project.as_str())
                            .await?;
                        writable.insert(project.clone(), may);
                        may
                    }
                },
            };
            let uses: Vec<AssetUse> = uses
                .iter()
                .filter(|(asset, _)| *asset == upload.id)
                .map(|(_, used)| used.clone())
                .collect();
            listed.push(OwnUpload {
                deletable: deletable && uses.is_empty(),
                upload,
                uses,
            });
        }
        let last = listed
            .last()
            .map(|own: &OwnUpload| UploadMark::from(&own.upload));

        Ok(Listed::paged(listed, last, at_most))
    }

    /// Returns how many bytes the signed-in account's uploads use, wherever they went, and its limit.
    /// The limit is the account's own, if an operator gave it one, or else the default for accounts.
    pub async fn usage_of(&self, agent: &Agent) -> Result<OwnUsage, AssetsError> {
        let Agent::Account(account) = agent else {
            return Err(AssetsError::NoAccount);
        };

        transacting::retrying!(&*self.transactions, |transaction| {
            let usage = self
                .catalog
                .usage_within(transaction, Some(account), None)
                .await?;
            let limits = self.limits_of(transaction, Some(account)).await?;

            Ok::<_, AssetsError>(OwnUsage {
                used: usage.account,
                most: limits.each_account,
            })
        })
    }

    /// Stores an account's name and email, as the accounts feature reported them.
    /// Operators see them in the storage overview and can search by them.
    pub async fn remember_person(
        &self,
        within: Option<&mut Transaction>,
        person: &Person,
        version: Version,
    ) -> Result<(), AssetsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog
                .remember_person(transaction, person, version)
                .await?;

            Ok::<_, AssetsError>(())
        })
    }

    /// Removes the name and email of a deleted account, and ends the use of its picture.
    pub async fn forget_person(
        &self,
        within: Option<&mut Transaction>,
        account: &AccountId,
        version: Version,
    ) -> Result<(), AssetsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog
                .forget_person(transaction, account, version, self.clock.now())
                .await?;
            self.uses
                .stop_using(
                    transaction,
                    Purpose::AccountPicture,
                    &Holder::Account(account.clone()),
                    version,
                    self.clock.now(),
                )
                .await?;

            Ok::<_, AssetsError>(())
        })
    }

    /// Records an account's new picture, as the accounts feature reported it at a version.
    ///
    /// Only an image of the account's own can be its picture. Any other id is not recorded,
    /// and the account then shows no picture as far as the assets feature is concerned.
    /// With `drop_previous`, the picture shown before is deleted, if it is the account's own and nothing uses it any more.
    /// Its stored bytes are removed later, by the chore that removes stray contents.
    pub async fn picture_changed(
        &self,
        within: Option<&mut Transaction>,
        account: &AccountId,
        version: Version,
        picture: &str,
        previous: Option<&str>,
        drop_previous: bool,
    ) -> Result<(), AssetsError> {
        let own = self.own_image(account, picture).await?;
        let dropped = match (drop_previous, previous) {
            (true, Some(previous)) => self.own_image(account, previous).await?,
            _ => None,
        };
        let holder = Holder::Account(account.clone());

        transacting::joining!(&*self.transactions, within, |transaction| {
            match own {
                Some(picture) => {
                    self.uses
                        .use_as(
                            transaction,
                            &picture,
                            Purpose::AccountPicture,
                            &holder,
                            version,
                        )
                        .await?
                }
                None => {
                    self.uses
                        .stop_using(
                            transaction,
                            Purpose::AccountPicture,
                            &holder,
                            version,
                            self.clock.now(),
                        )
                        .await?
                }
            }
            if let Some(previous) = dropped {
                self.dropped_if_unused(transaction, &previous).await?;
            }

            Ok::<_, AssetsError>(())
        })
    }

    /// Records that an account shows no picture any more, as the accounts feature reported it at a version.
    /// With `drop`, the picture is deleted, if it is the account's own and nothing uses it any more.
    pub async fn picture_removed(
        &self,
        within: Option<&mut Transaction>,
        account: &AccountId,
        version: Version,
        picture: &str,
        drop: bool,
    ) -> Result<(), AssetsError> {
        let dropped = match drop {
            true => self.own_image(account, picture).await?,
            false => None,
        };
        let holder = Holder::Account(account.clone());

        transacting::joining!(&*self.transactions, within, |transaction| {
            self.uses
                .stop_using(
                    transaction,
                    Purpose::AccountPicture,
                    &holder,
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

    /// Returns the asset with this id if it is an image of the account's own that is not deleted.
    async fn own_image(
        &self,
        account: &AccountId,
        id: &str,
    ) -> Result<Option<AssetId>, AssetsError> {
        let Ok(id) = id.parse::<AssetId>() else {
            return Ok(None);
        };
        match self.events.latest(&AggregateId::from(&id)).await {
            Ok(standing) => Ok((!standing.state.is_deleted()
                && *standing.state.owner() == Owner::Account(account.clone()))
            .then_some(id)),
            Err(ServiceError::NotFound { .. }) => Ok(None),
            Err(failed) => Err(failed.into()),
        }
    }

    /// Deletes an asset as the system, in the given transaction, unless something still uses it.
    /// Its stored bytes are left to the chore that removes stray contents.
    async fn dropped_if_unused(
        &self,
        transaction: &mut Transaction,
        id: &AssetId,
    ) -> Result<(), AssetsError> {
        if !self.uses.uses_of(transaction, &[*id]).await?.is_empty() {
            return Ok(());
        }
        self.events
            .execute_at(
                transaction,
                &AggregateId::from(id),
                None,
                AssetCommand::Delete,
                &Agent::System,
            )
            .await?;
        self.catalog.forget(transaction, id).await?;

        Ok(())
    }

    /// Stores a project's name, as the projects feature reported it.
    /// Operators see it in the storage overview and can search by it.
    pub async fn remember_project_name(
        &self,
        within: Option<&mut Transaction>,
        project: &ProjectLink,
        version: Version,
        name: &str,
    ) -> Result<(), AssetsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog
                .remember_project_name(transaction, project, version, name)
                .await?;

            Ok::<_, AssetsError>(())
        })
    }

    /// Removes the name of a deleted project.
    pub async fn forget_project_name(
        &self,
        within: Option<&mut Transaction>,
        project: &ProjectLink,
        version: Version,
    ) -> Result<(), AssetsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog
                .forget_project_name(transaction, project, version, self.clock.now())
                .await?;

            Ok::<_, AssetsError>(())
        })
    }

    /// Deletes old tombstones, from the access tables, the stored names and the uses of assets.
    /// A tombstone keeps a late, outdated message from bringing deleted data back.
    /// Once it is old enough, no such message can arrive any more.
    /// Returns how many tombstones were deleted.
    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, AssetsError> {
        transacting::retrying!(&*self.transactions, |transaction| {
            let access = self
                .access
                .forget_tombstones_laid_before(transaction, before, at_most)
                .await?;
            let names = self
                .catalog
                .forget_tombstones_laid_before(transaction, before, at_most)
                .await?;
            let uses = self
                .uses
                .forget_tombstones_laid_before(transaction, before, at_most)
                .await?;

            Ok::<_, AssetsError>(access + names + uses)
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
            let limits = self.limits_of(transaction, uploader.as_ref()).await?;
            if let Some(limit) =
                crate::limits::first_passed(limits.on_what_is_kept(usage, counted), total_size)
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

    async fn operating(&self, agent: &Agent) -> Result<(), AssetsError> {
        if self.authorizer.operates(agent).await? {
            Ok(())
        } else {
            Err(AssetsError::NotAnOperator)
        }
    }

    async fn standing(&self, id: &AssetId) -> Result<Standing<Asset>, AssetsError> {
        Ok(self.events.latest(&AggregateId::from(id)).await?)
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

#[async_trait]
impl HearsAccess for AssetsCore {
    type Refused = AssetsError;

    /// Stores an account's role in a project, or that the account is no longer a member.
    /// The members feature reports this at a version.
    /// The stored roles decide who may see what.
    async fn remember_role(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        account: &AccountId,
        version: Version,
        role: Option<Role>,
    ) -> Result<(), AssetsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.access
                .remember_role(
                    transaction,
                    project,
                    account,
                    version,
                    role,
                    self.clock.now(),
                )
                .await?;

            Ok::<_, AssetsError>(())
        })
    }

    /// Stores that a project's members were disbanded because the project was deleted.
    /// After this, nobody is a member of the project.
    async fn remember_disbanded(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        version: Version,
    ) -> Result<(), AssetsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.access
                .remember_disbanded(transaction, project, version, self.clock.now())
                .await?;

            Ok::<_, AssetsError>(())
        })
    }

    /// Stores whether a project is in the trash, as the projects feature reported it at a version.
    async fn remember_trash(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        version: Version,
        in_the_trash: bool,
    ) -> Result<(), AssetsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.access
                .remember_trash(transaction, project, version, in_the_trash)
                .await?;

            Ok::<_, AssetsError>(())
        })
    }
}

#[async_trait]
impl HearsOperators for AssetsCore {
    type Refused = AssetsError;

    /// Stores whether an account runs the instance, as the accounts feature reported it at a version.
    async fn remember_operator(
        &self,
        within: Option<&mut Transaction>,
        account: &AccountId,
        version: Version,
        operator: bool,
    ) -> Result<(), AssetsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.access
                .remember_operator(transaction, account, version, operator, self.clock.now())
                .await?;

            Ok::<_, AssetsError>(())
        })
    }
}

/// Trims the search term.
/// A term that is empty or only spaces means no search at all.
fn searched(matching: Option<&str>) -> Option<&str> {
    matching.map(str::trim).filter(|term| !term.is_empty())
}

/// Returns when an asset's id was made, which is just before its pieces were stored.
fn made_at(id: &AssetId) -> Option<OffsetDateTime> {
    let (seconds, nanoseconds) = id.as_uuid().get_timestamp()?.to_unix();
    let made = OffsetDateTime::from_unix_timestamp(i64::try_from(seconds).ok()?).ok()?;

    Some(made + time::Duration::nanoseconds(i64::from(nanoseconds)))
}

fn not_found(id: &AssetId) -> AssetsError {
    AssetsError::Events(ServiceError::NotFound {
        aggregate: AggregateId::from(id),
        kind: KIND,
    })
}
