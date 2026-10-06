use std::collections::{BTreeMap, BTreeSet};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use appearances_core::{AppearanceCatalog, CatalogError, Place, Subject};
use async_trait::async_trait;

type Appearances = BTreeMap<Subject, BTreeSet<Place>>;

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
}

#[async_trait]
impl AppearanceCatalog for InMemoryAppearanceCatalog {
    async fn remember(&self, subject: &Subject, place: &Place) -> Result<(), CatalogError> {
        self.write()
            .entry(subject.clone())
            .or_default()
            .insert(place.clone());

        Ok(())
    }

    async fn forget(&self, subject: &Subject, place: &Place) -> Result<(), CatalogError> {
        let mut appearances = self.write();

        if let Some(places) = appearances.get_mut(subject) {
            places.remove(place);

            if places.is_empty() {
                appearances.remove(subject);
            }
        }

        Ok(())
    }

    async fn forget_subject(&self, subject: &Subject) -> Result<(), CatalogError> {
        self.write().remove(subject);

        Ok(())
    }

    async fn forget_place(&self, place: &Place) -> Result<(), CatalogError> {
        let mut appearances = self.write();

        for places in appearances.values_mut() {
            places.remove(place);
        }
        appearances.retain(|_, places| !places.is_empty());

        Ok(())
    }

    async fn places_of(&self, subject: &Subject) -> Result<Vec<Place>, CatalogError> {
        Ok(self
            .read()
            .get(subject)
            .map(|places| places.iter().cloned().collect())
            .unwrap_or_default())
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
