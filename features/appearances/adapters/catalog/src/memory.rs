use std::collections::BTreeMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use appearances_core::{AppearanceCatalog, CatalogError, Place, ProjectLink, Subject};
use async_trait::async_trait;
use time::OffsetDateTime;
use transacting::{Failure, Transaction};

#[derive(Debug, Clone, Copy)]
struct Seen {
    version: u64,
    present: bool,
}

#[derive(Debug, Clone, Default)]
struct Appearances {
    seen: BTreeMap<(Subject, Place), Seen>,
    subject_tombstones: BTreeMap<Subject, OffsetDateTime>,
    place_tombstones: BTreeMap<Place, OffsetDateTime>,
    projects: BTreeMap<Subject, ProjectLink>,
}

#[derive(Debug, Default)]
pub struct InMemoryAppearanceCatalog {
    appearances: Arc<RwLock<Appearances>>,
}

impl InMemoryAppearanceCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> RwLockReadGuard<'_, Appearances> {
        self.appearances
            .read()
            .expect("appearance catalog lock poisoned")
    }

    fn write(&self) -> RwLockWriteGuard<'_, Appearances> {
        self.appearances
            .write()
            .expect("appearance catalog lock poisoned")
    }

    fn mark(&self, subject: &Subject, place: &Place, now: Seen) {
        let mut appearances = self.write();

        if appearances.subject_tombstones.contains_key(subject)
            || appearances.place_tombstones.contains_key(place)
        {
            return;
        }

        let held = appearances
            .seen
            .entry((subject.clone(), place.clone()))
            .or_insert(now);

        if held.version <= now.version {
            *held = now;
        }
    }

    fn undo_on_rollback(&self, transaction: &mut Transaction) -> Result<(), CatalogError> {
        let appearances = self
            .appearances
            .read()
            .expect("appearance catalog lock poisoned")
            .clone();
        let into_appearances = self.appearances.clone();

        transacting::memory::undo_on_rollback(transaction, move || {
            *into_appearances
                .write()
                .expect("appearance catalog lock poisoned") = appearances;
        })
        .map_err(unreachable)
    }
}

fn oldest_laid_before<Gone: Clone + Ord>(
    tombstones: &BTreeMap<Gone, OffsetDateTime>,
    before: OffsetDateTime,
    at_most: u64,
) -> Vec<Gone> {
    let mut long_gone: Vec<(OffsetDateTime, Gone)> = tombstones
        .iter()
        .filter(|(_, laid)| **laid < before)
        .map(|(gone, laid)| (*laid, gone.clone()))
        .collect();
    long_gone.sort();

    long_gone
        .into_iter()
        .take(usize::try_from(at_most).unwrap_or(usize::MAX))
        .map(|(_, gone)| gone)
        .collect()
}

fn unreachable(failure: Failure) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

#[async_trait]
impl AppearanceCatalog for InMemoryAppearanceCatalog {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        subject: &Subject,
        place: &Place,
        version: u64,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.mark(
            subject,
            place,
            Seen {
                version,
                present: true,
            },
        );

        Ok(())
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        subject: &Subject,
        place: &Place,
        version: u64,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.mark(
            subject,
            place,
            Seen {
                version,
                present: false,
            },
        );

        Ok(())
    }

    async fn forget_subject(
        &self,
        transaction: &mut Transaction,
        subject: &Subject,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut appearances = self.write();

        appearances
            .subject_tombstones
            .entry(subject.clone())
            .or_insert(at);
        appearances.seen.retain(|(held, _), _| held != subject);

        Ok(())
    }

    async fn forget_place(
        &self,
        transaction: &mut Transaction,
        place: &Place,
        at: OffsetDateTime,
    ) -> Result<Vec<Subject>, CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut appearances = self.write();
        let mut appeared = Vec::new();

        appearances
            .place_tombstones
            .entry(place.clone())
            .or_insert(at);
        appearances.seen.retain(|(subject, held), seen| {
            if held != place {
                return true;
            }
            if seen.present {
                appeared.push(subject.clone());
            }

            false
        });

        Ok(appeared)
    }

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut appearances = self.write();
        let long_gone_subjects =
            oldest_laid_before(&appearances.subject_tombstones, before, at_most);
        let long_gone_places = oldest_laid_before(&appearances.place_tombstones, before, at_most);

        for subject in &long_gone_subjects {
            appearances.subject_tombstones.remove(subject);
            appearances.projects.remove(subject);
        }
        for place in &long_gone_places {
            appearances.place_tombstones.remove(place);
        }

        Ok(
            u64::try_from(long_gone_subjects.len() + long_gone_places.len())
                .expect("a count of tombstones fits"),
        )
    }

    async fn places_of(&self, subject: &Subject) -> Result<Vec<Place>, CatalogError> {
        Ok(self
            .read()
            .seen
            .iter()
            .filter(|((held, _), seen)| held == subject && seen.present)
            .map(|((_, place), _)| place.clone())
            .collect())
    }

    async fn remember_project(
        &self,
        transaction: &mut Transaction,
        subject: &Subject,
        project: &ProjectLink,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.write()
            .projects
            .insert(subject.clone(), project.clone());

        Ok(())
    }

    async fn project_of(&self, subject: &Subject) -> Result<Option<ProjectLink>, CatalogError> {
        Ok(self.read().projects.get(subject).cloned())
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use transacting::{InMemoryTransactions, Transactions};

    use super::*;
    use crate::suite::Workbench;

    struct InMemory(InMemoryAppearanceCatalog, InMemoryTransactions);

    #[async_trait]
    impl Workbench for InMemory {
        type Catalog = InMemoryAppearanceCatalog;

        async fn setup() -> Self {
            Self(
                InMemoryAppearanceCatalog::new(),
                InMemoryTransactions::new(),
            )
        }

        fn catalog(&self) -> &Self::Catalog {
            &self.0
        }

        fn transactions(&self) -> &dyn Transactions {
            &self.1
        }

        async fn cleanup(self) {}
    }

    crate::suite::conformance_tests!(InMemory);
}
