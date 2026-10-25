use std::collections::BTreeMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use appearances_core::{AppearanceCatalog, CatalogError, Place, ProjectLink, Subject};
use async_trait::async_trait;
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy)]
struct Seen {
    version: u64,
    present: bool,
}

#[derive(Debug, Default)]
struct Appearances {
    seen: BTreeMap<(Subject, Place), Seen>,
    gone_subjects: BTreeMap<Subject, OffsetDateTime>,
    gone_places: BTreeMap<Place, OffsetDateTime>,
    projects: BTreeMap<Subject, ProjectLink>,
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

        if appearances.gone_subjects.contains_key(subject)
            || appearances.gone_places.contains_key(place)
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

    async fn forget_subject(
        &self,
        subject: &Subject,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        let mut appearances = self.write();

        appearances
            .gone_subjects
            .entry(subject.clone())
            .or_insert(at);
        appearances.seen.retain(|(held, _), _| held != subject);

        Ok(())
    }

    async fn forget_place(
        &self,
        place: &Place,
        at: OffsetDateTime,
    ) -> Result<Vec<Subject>, CatalogError> {
        let mut appearances = self.write();
        let mut appeared = Vec::new();

        appearances.gone_places.entry(place.clone()).or_insert(at);
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
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        let mut appearances = self.write();
        let long_gone_subjects = oldest_laid_before(&appearances.gone_subjects, before, at_most);
        let long_gone_places = oldest_laid_before(&appearances.gone_places, before, at_most);

        for subject in &long_gone_subjects {
            appearances.gone_subjects.remove(subject);
            appearances.projects.remove(subject);
        }
        for place in &long_gone_places {
            appearances.gone_places.remove(place);
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
        subject: &Subject,
        project: &ProjectLink,
    ) -> Result<(), CatalogError> {
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
