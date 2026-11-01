use eventsourcing::Version;
use ideas_core::{IdeaCatalog, IdeaId, IdeaSummary, IdeaTitle, ProjectLink};
use time::OffsetDateTime;

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: IdeaCatalog;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

    async fn cleanup(self);
}

pub fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(seconds).expect("a plausible moment")
}

pub fn a_summary(id: IdeaId, project: &str, title: &str) -> IdeaSummary {
    IdeaSummary {
        id,
        version: Version::of(1),
        project: ProjectLink::from(project),
        title: IdeaTitle::new(title).expect("a plain title is fine"),
    }
}

pub async fn a_remembered_idea_is_listed_in_its_project(catalog: &impl IdeaCatalog) {
    let summary = a_summary(IdeaId::generate(at(1_000)), "project_1", "The Loom");

    catalog
        .remember(&summary)
        .await
        .expect("remembering should succeed");

    let found = catalog
        .in_project(&ProjectLink::from("project_1"))
        .await
        .expect("listing should succeed");

    assert_eq!(found, vec![summary]);
}

pub async fn a_project_nobody_wrote_in_lists_nothing(catalog: &impl IdeaCatalog) {
    let found = catalog
        .in_project(&ProjectLink::from("project_empty"))
        .await
        .expect("listing an unknown project should not fail");

    assert!(found.is_empty());
}

pub async fn ideas_of_other_projects_are_not_listed(catalog: &impl IdeaCatalog) {
    catalog
        .remember(&a_summary(
            IdeaId::generate(at(1_000)),
            "project_mine",
            "Mine",
        ))
        .await
        .expect("remembering should succeed");
    catalog
        .remember(&a_summary(
            IdeaId::generate(at(1_000)),
            "project_theirs",
            "Theirs",
        ))
        .await
        .expect("remembering should succeed");

    let found = catalog
        .in_project(&ProjectLink::from("project_mine"))
        .await
        .expect("listing should succeed");

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].title.as_str(), "Mine");
}

pub async fn remembering_the_same_idea_again_replaces_what_was_there(catalog: &impl IdeaCatalog) {
    let id = IdeaId::generate(at(1_000));
    catalog
        .remember(&a_summary(id, "project_1", "The Loom"))
        .await
        .expect("remembering should succeed");

    let mut later = a_summary(id, "project_1", "The Silent Loom");
    later.version = Version::of(2);
    catalog
        .remember(&later)
        .await
        .expect("remembering should succeed");

    let found = catalog
        .in_project(&ProjectLink::from("project_1"))
        .await
        .expect("listing should succeed");

    assert_eq!(found, vec![later], "a catalog holds one row per idea");
}

pub async fn a_forgotten_idea_is_no_longer_listed(catalog: &impl IdeaCatalog) {
    let id = IdeaId::generate(at(1_000));
    catalog
        .remember(&a_summary(id, "project_1", "The Loom"))
        .await
        .expect("remembering should succeed");

    catalog
        .forget(&id, at(5_000))
        .await
        .expect("forgetting should succeed");

    assert!(
        catalog
            .in_project(&ProjectLink::from("project_1"))
            .await
            .expect("listing should succeed")
            .is_empty()
    );
}

pub async fn forgetting_something_never_remembered_is_harmless(catalog: &impl IdeaCatalog) {
    catalog
        .forget(&IdeaId::generate(at(1_000)), at(5_000))
        .await
        .expect("forgetting an unknown idea should not fail");
}

pub async fn the_newest_idea_is_listed_first(catalog: &impl IdeaCatalog) {
    let captured: Vec<IdeaId> = (1..=6)
        .map(|nth| IdeaId::generate(at(nth * 1_000)))
        .collect();

    for id in &captured {
        catalog
            .remember(&a_summary(*id, "project_1", "An idea"))
            .await
            .expect("remembering should succeed");
    }

    let found = catalog
        .in_project(&ProjectLink::from("project_1"))
        .await
        .expect("listing should succeed");

    assert_eq!(
        found.iter().map(|summary| summary.id).collect::<Vec<_>>(),
        captured.into_iter().rev().collect::<Vec<_>>(),
        "the idea an author had most recently should be the first they see"
    );
}

pub async fn a_batch_starts_at_the_oldest_idea(catalog: &impl IdeaCatalog) {
    let captured: Vec<IdeaId> = (1..=5)
        .map(|nth| IdeaId::generate(at(nth * 1_000)))
        .collect();
    for id in &captured {
        catalog
            .remember(&a_summary(*id, "project_1", "An idea"))
            .await
            .expect("remembering should succeed");
    }

    let batch = catalog
        .in_project_after(&ProjectLink::from("project_1"), None, 2)
        .await
        .expect("listing should succeed");

    assert_eq!(
        batch.iter().map(|summary| summary.id).collect::<Vec<_>>(),
        captured[..2].to_vec(),
        "a sweep walks forward from the oldest, so the cursor it carries only ever grows"
    );
}

pub async fn a_batch_after_a_cursor_takes_what_comes_next(catalog: &impl IdeaCatalog) {
    let captured: Vec<IdeaId> = (1..=5)
        .map(|nth| IdeaId::generate(at(nth * 1_000)))
        .collect();
    for id in &captured {
        catalog
            .remember(&a_summary(*id, "project_1", "An idea"))
            .await
            .expect("remembering should succeed");
    }

    let batch = catalog
        .in_project_after(&ProjectLink::from("project_1"), Some(captured[1]), 2)
        .await
        .expect("listing should succeed");

    assert_eq!(
        batch.iter().map(|summary| summary.id).collect::<Vec<_>>(),
        captured[2..4].to_vec()
    );
}

pub async fn a_batch_past_the_end_is_empty(catalog: &impl IdeaCatalog) {
    let only = IdeaId::generate(at(1_000));
    catalog
        .remember(&a_summary(only, "project_1", "An idea"))
        .await
        .expect("remembering should succeed");

    let batch = catalog
        .in_project_after(&ProjectLink::from("project_1"), Some(only), 8)
        .await
        .expect("listing should succeed");

    assert!(
        batch.is_empty(),
        "an empty batch is how a sweep learns it is finished, so it must not wrap around"
    );
}

pub async fn a_batch_stays_inside_its_project(catalog: &impl IdeaCatalog) {
    let mine = IdeaId::generate(at(1_000));
    let theirs = IdeaId::generate(at(2_000));
    catalog
        .remember(&a_summary(mine, "project_mine", "Mine"))
        .await
        .expect("remembering should succeed");
    catalog
        .remember(&a_summary(theirs, "project_theirs", "Theirs"))
        .await
        .expect("remembering should succeed");

    let batch = catalog
        .in_project_after(&ProjectLink::from("project_mine"), None, 8)
        .await
        .expect("listing should succeed");

    assert_eq!(
        batch.iter().map(|summary| summary.id).collect::<Vec<_>>(),
        vec![mine],
        "a sweep of one deleted project must never reach into another"
    );
}

async fn listed(catalog: &impl IdeaCatalog) -> Vec<IdeaSummary> {
    catalog
        .in_project(&ProjectLink::from("project_1"))
        .await
        .expect("listing should succeed")
}

pub async fn a_forgotten_idea_heard_of_late_is_not_remembered_again(catalog: &impl IdeaCatalog) {
    let gone = a_summary(IdeaId::generate(at(1_000)), "project_1", "The Loom");
    catalog
        .remember(&gone)
        .await
        .expect("remembering should succeed");
    catalog
        .forget(&gone.id, at(5_000))
        .await
        .expect("forgetting should succeed");

    catalog
        .remember(&gone)
        .await
        .expect("remembering should succeed");
    let mut newer = gone.clone();
    newer.version = Version::of(newer.version.count() + 1);
    catalog
        .remember(&newer)
        .await
        .expect("remembering should succeed");

    assert!(
        listed(catalog).await.is_empty(),
        "what was forgotten stays forgotten, however late or new a summary of it comes"
    );
}

pub async fn idea_tombstones_are_forgotten_oldest_first_and_at_most_so_many(
    catalog: &impl IdeaCatalog,
) {
    let (older, newer, latest) = (
        a_summary(IdeaId::generate(at(1_000)), "project_1", "The Loom"),
        a_summary(IdeaId::generate(at(2_000)), "project_1", "The Loom"),
        a_summary(IdeaId::generate(at(3_000)), "project_1", "The Loom"),
    );
    for (summary, laid) in [(&newer, 6_000), (&older, 5_000), (&latest, 9_000)] {
        catalog
            .forget(&summary.id, at(laid))
            .await
            .expect("forgetting should succeed");
    }

    let forgotten = catalog
        .forget_tombstones_laid_before(at(8_000), 1)
        .await
        .expect("forgetting tombstones should succeed");
    for summary in [&older, &newer, &latest] {
        catalog
            .remember(summary)
            .await
            .expect("remembering should succeed");
    }

    assert_eq!(forgotten, 1);
    assert_eq!(
        listed(catalog).await,
        vec![older],
        "only the oldest tombstone is gone, so only it can be remembered again"
    );
}

pub async fn an_older_idea_summary_heard_late_changes_nothing(catalog: &impl IdeaCatalog) {
    let summary = a_summary(IdeaId::generate(at(1_000)), "project_1", "The Loom");
    let mut newer = summary.clone();
    newer.version = Version::of(2);
    catalog
        .remember(&newer)
        .await
        .expect("remembering should succeed");

    catalog
        .remember(&summary)
        .await
        .expect("remembering should succeed");

    assert_eq!(listed(catalog).await, vec![newer]);
}

#[macro_export]
macro_rules! catalog_conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::suite::$case(bench.store()).await;
            bench.cleanup().await;
        }
    };
}

#[macro_export]
macro_rules! conformance_tests {
    ($workbench:ty) => {
        $crate::catalog_conformance_case!(
            $workbench,
            a_forgotten_idea_heard_of_late_is_not_remembered_again
        );
        $crate::catalog_conformance_case!(
            $workbench,
            idea_tombstones_are_forgotten_oldest_first_and_at_most_so_many
        );
        $crate::catalog_conformance_case!(
            $workbench,
            an_older_idea_summary_heard_late_changes_nothing
        );
        $crate::catalog_conformance_case!($workbench, a_batch_starts_at_the_oldest_idea);
        $crate::catalog_conformance_case!($workbench, a_batch_after_a_cursor_takes_what_comes_next);
        $crate::catalog_conformance_case!($workbench, a_batch_past_the_end_is_empty);
        $crate::catalog_conformance_case!($workbench, a_batch_stays_inside_its_project);
        $crate::catalog_conformance_case!($workbench, a_remembered_idea_is_listed_in_its_project);
        $crate::catalog_conformance_case!($workbench, a_project_nobody_wrote_in_lists_nothing);
        $crate::catalog_conformance_case!($workbench, ideas_of_other_projects_are_not_listed);
        $crate::catalog_conformance_case!(
            $workbench,
            remembering_the_same_idea_again_replaces_what_was_there
        );
        $crate::catalog_conformance_case!($workbench, a_forgotten_idea_is_no_longer_listed);
        $crate::catalog_conformance_case!(
            $workbench,
            forgetting_something_never_remembered_is_harmless
        );
        $crate::catalog_conformance_case!($workbench, the_newest_idea_is_listed_first);
    };
}
