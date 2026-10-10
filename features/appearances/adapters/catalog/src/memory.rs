use std::collections::{BTreeMap, BTreeSet};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use appearances_core::{AppearanceCatalog, CatalogError, Place, Subject};
use async_trait::async_trait;

#[derive(Debug, Clone, Copy)]
struct Seen {
    version: u64,
    present: bool,
}

#[derive(Debug, Default)]
struct Appearances {
    seen: BTreeMap<(Subject, Place), Seen>,
    gone_subjects: BTreeSet<Subject>,
    gone_places: BTreeSet<Place>,
}

#[derive(Debug, Default)]
pub struct InMemoryAppearanceCatalog {
    appearances: RwLock<Appearances>,
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

        if appearances.gone_subjects.contains(subject) || appearances.gone_places.contains(place) {
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
}

#[async_trait]
impl AppearanceCatalog for InMemoryAppearanceCatalog {
    async fn remember(
        &self,
        subject: &Subject,
        place: &Place,
        version: u64,
    ) -> Result<(), CatalogError> {
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
        subject: &Subject,
        place: &Place,
        version: u64,
    ) -> Result<(), CatalogError> {
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

    async fn forget_subject(&self, subject: &Subject) -> Result<(), CatalogError> {
        let mut appearances = self.write();

        appearances.gone_subjects.insert(subject.clone());
        appearances.seen.retain(|(held, _), _| held != subject);

        Ok(())
    }

    async fn forget_place(&self, place: &Place) -> Result<(), CatalogError> {
        let mut appearances = self.write();

        appearances.gone_places.insert(place.clone());
        appearances.seen.retain(|(_, held), _| held != place);

        Ok(())
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
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use super::*;
    use crate::suite::Workbench;

    struct InMemory(InMemoryAppearanceCatalog);

    #[async_trait]
    impl Workbench for InMemory {
        type Catalog = InMemoryAppearanceCatalog;

        async fn setup() -> Self {
            Self(InMemoryAppearanceCatalog::new())
        }

        fn catalog(&self) -> &Self::Catalog {
            &self.0
        }

        async fn cleanup(self) {}
    }

    crate::suite::conformance_tests!(InMemory);
}
