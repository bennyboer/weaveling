use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};

use assets_core::{
    AssetCatalog, AssetId, AssetSummary, CatalogError, Held, Holders, Owner, Person, ProjectLink,
    Sorted, Upload, UploadMark, UploadsSorted, Usage,
};
use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use time::OffsetDateTime;
use transacting::{Transaction, memory};

/// A name as heard at a version.
/// It holds the name, an account's email, the text it is searched by, and when it was forgotten.
/// The time it was forgotten is missing while the name is known.
#[derive(Debug, Clone)]
struct Named {
    version: u64,
    name: Option<String>,
    email: Option<String>,
    searched: Option<String>,
    gone_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Default)]
struct Kept {
    summaries: HashMap<AssetId, AssetSummary>,
    people: HashMap<String, Named>,
    project_names: HashMap<String, Named>,
}

#[derive(Debug, Default)]
pub struct InMemoryAssetCatalog {
    kept: Arc<Mutex<Kept>>,
}

impl InMemoryAssetCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    fn kept(&self) -> MutexGuard<'_, Kept> {
        self.kept.lock().expect("the asset catalog is not poisoned")
    }

    fn undo_on_rollback(&self, transaction: &mut Transaction) -> Result<(), CatalogError> {
        let before = self.kept().clone();
        let into = self.kept.clone();

        memory::undo_on_rollback(transaction, move || {
            *into.lock().expect("the asset catalog is not poisoned") = before;
        })
        .map_err(|failure| CatalogError::Backend(Box::new(failure)))
    }
}

fn told(names: &mut HashMap<String, Named>, id: String, version: Version, heard: Named) {
    let held = names.entry(id).or_insert(Named {
        version: 0,
        ..heard.clone()
    });
    if held.version < version.count() {
        *held = Named {
            version: version.count(),
            ..heard
        };
    }
}

fn forgotten_at(at: OffsetDateTime) -> Named {
    Named {
        version: 0,
        name: None,
        email: None,
        searched: None,
        gone_at: Some(at),
    }
}

fn oldest(names: &HashMap<String, Named>, before: OffsetDateTime, at_most: u64) -> Vec<String> {
    let mut old: Vec<(OffsetDateTime, String)> = names
        .iter()
        .filter_map(|(id, named)| {
            named
                .gone_at
                .filter(|at| *at < before)
                .map(|at| (at, id.clone()))
        })
        .collect();
    old.sort();
    old.truncate(usize::try_from(at_most).unwrap_or(usize::MAX));

    old.into_iter().map(|(_, id)| id).collect()
}

#[async_trait]
impl AssetCatalog for InMemoryAssetCatalog {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        summary: &AssetSummary,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.kept().summaries.insert(summary.id, summary.clone());

        Ok(())
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        id: &AssetId,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.kept().summaries.remove(id);

        Ok(())
    }

    async fn usage_within(
        &self,
        _transaction: &mut Transaction,
        uploader: Option<&AccountId>,
        project: Option<&ProjectLink>,
    ) -> Result<Usage, CatalogError> {
        let kept = self.kept();
        let sum = |counts: &dyn Fn(&AssetSummary) -> bool| {
            kept.summaries
                .values()
                .filter(|summary| counts(summary))
                .map(|summary| summary.total_size)
                .sum::<u64>()
        };

        Ok(Usage {
            instance: sum(&|_| true),
            account: sum(&|summary| uploader.is_some() && summary.uploader.as_ref() == uploader),
            project: sum(
                &|summary| matches!(&summary.owner, Owner::Project(held) if Some(held) == project),
            ),
        })
    }

    async fn held_by(
        &self,
        _transaction: &mut Transaction,
        holders: Holders,
        sorted: Sorted,
        after: Option<&Held>,
        matching: Option<&str>,
        at_most: u64,
    ) -> Result<Vec<Held>, CatalogError> {
        let kept = self.kept();
        let mut totals: BTreeMap<String, u64> = BTreeMap::new();
        for summary in kept.summaries.values() {
            let holder = match (holders, &summary.owner, &summary.uploader) {
                (Holders::Accounts, _, Some(uploader)) => uploader.to_string(),
                (Holders::Projects, Owner::Project(project), _) => project.to_string(),
                _ => continue,
            };
            *totals.entry(holder).or_default() += summary.total_size;
        }
        let names = match holders {
            Holders::Accounts => &kept.people,
            Holders::Projects => &kept.project_names,
        };
        let term = matching.map(str::to_lowercase);
        let mut held: Vec<Held> = totals
            .into_iter()
            .filter(|(_, total)| *total > 0)
            .map(|(id, total)| {
                let named = names.get(&id);
                Held {
                    total,
                    name: named.and_then(|named| named.name.clone()),
                    email: named.and_then(|named| named.email.clone()),
                    id,
                }
            })
            .filter(|held| match &term {
                None => true,
                Some(term) => names
                    .get(&held.id)
                    .and_then(|named| named.searched.as_ref())
                    .is_some_and(|searched| searched.contains(term.as_str())),
            })
            .collect();
        if sorted == Sorted::MostUsed {
            held.sort_by(|one, other| other.total.cmp(&one.total).then(one.id.cmp(&other.id)));
        }
        let past = |held: &Held| match (after, sorted) {
            (None, _) => true,
            (Some(last), Sorted::ById) => held.id > last.id,
            (Some(last), Sorted::MostUsed) => {
                held.total < last.total || (held.total == last.total && held.id > last.id)
            }
        };

        Ok(held
            .into_iter()
            .filter(past)
            .take(usize::try_from(at_most).unwrap_or(usize::MAX))
            .collect())
    }

    async fn held_of(
        &self,
        _transaction: &mut Transaction,
        ids: &[AssetId],
    ) -> Result<HashSet<AssetId>, CatalogError> {
        let kept = self.kept();

        Ok(ids
            .iter()
            .filter(|id| kept.summaries.contains_key(id))
            .copied()
            .collect())
    }

    async fn owned_by(
        &self,
        _transaction: &mut Transaction,
        owner: &Owner,
        at_most: u64,
    ) -> Result<Vec<AssetId>, CatalogError> {
        let mut owned: Vec<AssetId> = self
            .kept()
            .summaries
            .values()
            .filter(|summary| &summary.owner == owner)
            .map(|summary| summary.id)
            .collect();
        owned.sort_by_key(ToString::to_string);
        owned.truncate(usize::try_from(at_most).unwrap_or(usize::MAX));

        Ok(owned)
    }

    async fn uploaded_by(
        &self,
        _transaction: &mut Transaction,
        account: &AccountId,
        sorted: UploadsSorted,
        after: Option<&UploadMark>,
        at_most: u64,
    ) -> Result<Vec<Upload>, CatalogError> {
        let kept = self.kept();
        let key = |total_size: u64, uploaded_at: OffsetDateTime| match sorted {
            UploadsSorted::Largest => (total_size, OffsetDateTime::UNIX_EPOCH),
            UploadsSorted::Newest => (0, uploaded_at),
        };
        let mut uploads: Vec<Upload> = kept
            .summaries
            .values()
            .filter(|summary| summary.uploader.as_ref() == Some(account))
            .map(|summary| Upload {
                id: summary.id,
                owner: summary.owner.clone(),
                project_name: match &summary.owner {
                    Owner::Project(project) => kept
                        .project_names
                        .get(project.as_str())
                        .and_then(|named| named.name.clone()),
                    Owner::Account(_) => None,
                },
                format: summary.format,
                width: summary.width,
                height: summary.height,
                total_size: summary.total_size,
                uploaded_at: summary.uploaded_at,
            })
            .filter(|upload| match after {
                None => true,
                Some(last) => {
                    let (this, that) = (
                        key(upload.total_size, upload.uploaded_at),
                        key(last.total_size, last.uploaded_at),
                    );
                    this < that || (this == that && upload.id.to_string() > last.id.to_string())
                }
            })
            .collect();
        uploads.sort_by(|one, other| {
            key(other.total_size, other.uploaded_at)
                .cmp(&key(one.total_size, one.uploaded_at))
                .then_with(|| one.id.to_string().cmp(&other.id.to_string()))
        });
        uploads.truncate(usize::try_from(at_most).unwrap_or(usize::MAX));

        Ok(uploads)
    }

    async fn remember_person(
        &self,
        transaction: &mut Transaction,
        person: &Person,
        version: Version,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        told(
            &mut self.kept().people,
            person.account.to_string(),
            version,
            Named {
                version: 0,
                name: Some(person.name.clone()),
                email: Some(person.email.clone()),
                searched: Some(format!("{} {}", person.name, person.email).to_lowercase()),
                gone_at: None,
            },
        );

        Ok(())
    }

    async fn forget_person(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        version: Version,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        told(
            &mut self.kept().people,
            account.to_string(),
            version,
            forgotten_at(at),
        );

        Ok(())
    }

    async fn remember_project_name(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        version: Version,
        name: &str,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        told(
            &mut self.kept().project_names,
            project.to_string(),
            version,
            Named {
                version: 0,
                name: Some(name.to_owned()),
                email: None,
                searched: Some(name.to_lowercase()),
                gone_at: None,
            },
        );

        Ok(())
    }

    async fn forget_project_name(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        version: Version,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        told(
            &mut self.kept().project_names,
            project.to_string(),
            version,
            forgotten_at(at),
        );

        Ok(())
    }

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut kept = self.kept();
        let people = oldest(&kept.people, before, at_most);
        for account in &people {
            kept.people.remove(account);
        }
        let projects = oldest(&kept.project_names, before, at_most);
        for project in &projects {
            kept.project_names.remove(project);
        }

        Ok((people.len() + projects.len()) as u64)
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use transacting::{InMemoryTransactions, Transactions};

    use super::*;
    use crate::suite::Workbench;

    struct InMemory(InMemoryAssetCatalog, InMemoryTransactions);

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemoryAssetCatalog;

        async fn setup() -> Self {
            Self(InMemoryAssetCatalog::new(), InMemoryTransactions::new())
        }

        fn store(&self) -> &Self::Store {
            &self.0
        }

        fn transactions(&self) -> &dyn Transactions {
            &self.1
        }

        async fn cleanup(self) {}
    }

    crate::conformance_tests!(InMemory);
}
