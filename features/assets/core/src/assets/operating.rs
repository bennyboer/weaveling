use eventsourcing::{AccountId, Agent};

use crate::asset::ProjectLink;
use crate::assets::AssetsCore;
use crate::catalog::{HeldMark, Holders, Sorted};
use crate::error::AssetsError;
use crate::limits::MOST_COUNTED;
use crate::storage::{AccountStorage, InstanceStorage, Listed, ProjectOwner, ProjectStorage};

impl AssetsCore {
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
        after: Option<&HeldMark>,
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

            Ok::<_, AssetsError>(Listed::paged(
                listed,
                held.last().map(HeldMark::from),
                at_most,
            ))
        })
    }

    /// Lists how much storage each project uses, with its limit and its owners, one page at a time.
    /// Only projects that hold something are listed.
    /// With a search term, only projects whose name contains it are listed.
    /// Only an operator may ask.
    pub async fn projects_storage(
        &self,
        sorted: Sorted,
        after: Option<&HeldMark>,
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

        Ok(Listed::paged(
            listed,
            held.last().map(HeldMark::from),
            at_most,
        ))
    }

    async fn operating(&self, agent: &Agent) -> Result<(), AssetsError> {
        if self.authorizer.operates(agent).await? {
            Ok(())
        } else {
            Err(AssetsError::NotAnOperator)
        }
    }
}

/// Trims the search term.
/// A term that is empty or only spaces means no search at all.
fn searched(matching: Option<&str>) -> Option<&str> {
    matching.map(str::trim).filter(|term| !term.is_empty())
}
