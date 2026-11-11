use std::ops::Deref;

use appearances_core::{
    AppearanceCatalog, CatalogError, IdeaLink, Place, ProjectLink, SceneLink, SectionLink, Subject,
};
use time::macros::datetime;
use time::{Duration, OffsetDateTime};
use transacting::{Transaction, Transactions, Tried};

const LAID: OffsetDateTime = datetime!(2026-10-25 12:00 UTC);

const ALL_AT_ONCE: u64 = u64::MAX;

const PROJECT: &str = "project_1";

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Catalog: AppearanceCatalog;

    async fn setup() -> Self;

    fn catalog(&self) -> &Self::Catalog;

    fn transactions(&self) -> &dyn Transactions;

    fn committing(&self) -> Committing<'_, Self::Catalog> {
        Committing {
            catalog: self.catalog(),
            transactions: self.transactions(),
        }
    }

    async fn cleanup(self);
}

pub struct Committing<'a, C> {
    catalog: &'a C,
    transactions: &'a dyn Transactions,
}

fn unreachable(failure: transacting::Failure) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

impl<C: AppearanceCatalog> Committing<'_, C> {
    pub async fn begun(&self) -> Transaction {
        self.transactions
            .begin()
            .await
            .expect("beginning should succeed")
    }

    pub fn uncommitted(&self) -> &C {
        self.catalog
    }

    /// Remembered as an idea of `PROJECT` in a place of `PROJECT`, as nearly every case has it.
    pub async fn remember(
        &self,
        subject: &Subject,
        place: &Place,
        version: u64,
    ) -> Result<(), CatalogError> {
        let project = ProjectLink::from(PROJECT);

        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .remember_project(transaction, subject, &project)
                .await
                .map_err(Tried::Refused)?;
            self.catalog
                .remember_place_project(transaction, place, &project)
                .await
                .map_err(Tried::Refused)?;
            self.catalog
                .remember(transaction, subject, place, version)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn remember_alone(
        &self,
        subject: &Subject,
        place: &Place,
        version: u64,
    ) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .remember(transaction, subject, place, version)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn remember_place_project(
        &self,
        place: &Place,
        project: &ProjectLink,
    ) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .remember_place_project(transaction, place, project)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn forget(
        &self,
        subject: &Subject,
        place: &Place,
        version: u64,
    ) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .forget(transaction, subject, place, version)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn forget_subject(
        &self,
        subject: &Subject,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .forget_subject(transaction, subject, at)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn forget_place(
        &self,
        place: &Place,
        at: OffsetDateTime,
    ) -> Result<Vec<Subject>, CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            let done = self
                .catalog
                .forget_place(transaction, place, at)
                .await
                .map_err(Tried::Refused)?;

            Ok(done)
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            let done = self
                .catalog
                .forget_tombstones_laid_before(transaction, before, at_most)
                .await
                .map_err(Tried::Refused)?;

            Ok(done)
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn remember_project(
        &self,
        subject: &Subject,
        project: &ProjectLink,
    ) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .remember_project(transaction, subject, project)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }
}

impl<C> Deref for Committing<'_, C> {
    type Target = C;

    fn deref(&self) -> &C {
        self.catalog
    }
}

fn an_idea(named: &str) -> Subject {
    Subject::Idea(IdeaLink::from(named))
}

fn a_section(named: &str) -> Place {
    Place::Section(SectionLink::from(named))
}

fn a_scene(named: &str) -> Place {
    Place::Scene(SceneLink::from(named))
}

async fn remembered(
    catalog: &Committing<'_, impl AppearanceCatalog>,
    idea: &str,
    places: &[Place],
) {
    for place in places {
        catalog
            .remember(&an_idea(idea), place, 1)
            .await
            .expect("remembering should succeed");
    }
}

async fn places_of(catalog: &Committing<'_, impl AppearanceCatalog>, idea: &str) -> Vec<Place> {
    catalog
        .places_of(&an_idea(idea))
        .await
        .expect("asking should succeed")
}

pub async fn an_idea_nobody_placed_appears_nowhere(
    catalog: &Committing<'_, impl AppearanceCatalog>,
) {
    // expect: an idea never placed appears nowhere
    assert!(places_of(catalog, "idea_1").await.is_empty());
}

pub async fn an_idea_appears_wherever_it_was_remembered(
    catalog: &Committing<'_, impl AppearanceCatalog>,
) {
    // given: an idea remembered in a scene and a section
    remembered(
        catalog,
        "idea_1",
        &[a_scene("scene_1"), a_section("section_1")],
    )
    .await;

    // then: it appears in both
    assert_eq!(
        places_of(catalog, "idea_1").await,
        vec![a_scene("scene_1"), a_section("section_1")]
    );
}

pub async fn places_come_back_in_one_order_whatever_order_they_were_remembered_in(
    catalog: &Committing<'_, impl AppearanceCatalog>,
) {
    // given: an idea remembered in places out of order
    remembered(
        catalog,
        "idea_1",
        &[
            a_section("section_2"),
            a_scene("scene_1"),
            a_section("section_1"),
        ],
    )
    .await;

    // then: its places come back scenes first, each by id
    assert_eq!(
        places_of(catalog, "idea_1").await,
        vec![
            a_scene("scene_1"),
            a_section("section_1"),
            a_section("section_2"),
        ],
        "remembered shuffled on purpose: scenes come back before sections, each by id, \
         so both backends answer in the same order"
    );
}

pub async fn remembering_the_same_place_twice_is_one_appearance(
    catalog: &Committing<'_, impl AppearanceCatalog>,
) {
    // given: an idea remembered in one section twice
    remembered(
        catalog,
        "idea_1",
        &[a_section("section_1"), a_section("section_1")],
    )
    .await;

    // then: it appears there once
    assert_eq!(
        places_of(catalog, "idea_1").await,
        vec![a_section("section_1")],
        "a redelivered attachment must not make an idea appear twice"
    );
}

pub async fn forgetting_one_place_leaves_the_others(
    catalog: &Committing<'_, impl AppearanceCatalog>,
) {
    // given: an idea remembered in a section and a scene
    remembered(
        catalog,
        "idea_1",
        &[a_section("section_1"), a_scene("scene_1")],
    )
    .await;

    // when: the section is forgotten for it
    catalog
        .forget(&an_idea("idea_1"), &a_section("section_1"), 2)
        .await
        .expect("forgetting should succeed");

    // then: it appears only in the scene
    assert_eq!(places_of(catalog, "idea_1").await, vec![a_scene("scene_1")]);
}

pub async fn forgetting_an_idea_leaves_every_other_idea(
    catalog: &Committing<'_, impl AppearanceCatalog>,
) {
    // given: two ideas remembered in one section
    remembered(catalog, "idea_gone", &[a_section("section_1")]).await;
    remembered(catalog, "idea_kept", &[a_section("section_1")]).await;

    // when: one of them is forgotten
    catalog
        .forget_subject(&an_idea("idea_gone"), LAID)
        .await
        .expect("forgetting should succeed");

    // then: it appears nowhere
    assert!(places_of(catalog, "idea_gone").await.is_empty());
    // and: the other stays in the section
    assert_eq!(
        places_of(catalog, "idea_kept").await,
        vec![a_section("section_1")],
        "one idea being discarded must never empty the section for the others"
    );
}

pub async fn forgetting_a_place_reaches_every_idea_in_it(
    catalog: &Committing<'_, impl AppearanceCatalog>,
) {
    // given: two ideas remembered in a section to go, one of them also in another
    remembered(
        catalog,
        "idea_1",
        &[a_section("section_gone"), a_section("section_kept")],
    )
    .await;
    remembered(catalog, "idea_2", &[a_section("section_gone")]).await;

    // when: the section to go is forgotten
    catalog
        .forget_place(&a_section("section_gone"), LAID)
        .await
        .expect("forgetting should succeed");

    // then: the first appears only in the other
    assert_eq!(
        places_of(catalog, "idea_1").await,
        vec![a_section("section_kept")]
    );
    // and: the second appears nowhere
    assert!(
        places_of(catalog, "idea_2").await.is_empty(),
        "a removed section takes every idea noted in it, not just the first one found"
    );
}

pub async fn forgetting_a_place_says_which_ideas_appeared_there(
    catalog: &Committing<'_, impl AppearanceCatalog>,
) {
    // given: three ideas remembered in a section to go
    remembered(catalog, "idea_2", &[a_section("section_gone")]).await;
    remembered(catalog, "idea_1", &[a_section("section_gone")]).await;
    remembered(catalog, "idea_3", &[a_section("section_gone")]).await;
    // and: the last of them taken out of it again
    catalog
        .forget(&an_idea("idea_3"), &a_section("section_gone"), 2)
        .await
        .expect("forgetting should succeed");
    // and: a fourth remembered in another section
    remembered(catalog, "idea_4", &[a_section("section_kept")]).await;

    // when: the section to go is forgotten
    let appeared = catalog
        .forget_place(&a_section("section_gone"), LAID)
        .await
        .expect("forgetting should succeed");
    // and: a section nobody appeared in is forgotten
    let nobody = catalog
        .forget_place(&a_section("section_empty"), LAID)
        .await
        .expect("forgetting should succeed");

    // then: the ideas still in the first are named, in order
    assert_eq!(
        appeared,
        vec![an_idea("idea_1"), an_idea("idea_2")],
        "an idea already taken out of the place did not appear there"
    );
    // and: nobody is named for the empty one
    assert!(nobody.is_empty());
}

pub async fn a_section_and_a_scene_sharing_an_id_are_different_places(
    catalog: &Committing<'_, impl AppearanceCatalog>,
) {
    // given: an idea in a section and a scene sharing an id
    remembered(catalog, "idea_1", &[a_section("same_1"), a_scene("same_1")]).await;

    // when: the scene is forgotten
    catalog
        .forget_place(&a_scene("same_1"), LAID)
        .await
        .expect("forgetting should succeed");

    // then: the idea appears only in the section
    assert_eq!(
        places_of(catalog, "idea_1").await,
        vec![a_section("same_1")],
        "the type is part of a place's identity"
    );
}

pub async fn forgetting_what_was_never_remembered_is_harmless(
    catalog: &Committing<'_, impl AppearanceCatalog>,
) {
    // expect: forgetting an appearance, an idea and a place never remembered all succeed
    catalog
        .forget(&an_idea("idea_1"), &a_section("section_1"), 1)
        .await
        .expect("forgetting nothing is not a failure");
    catalog
        .forget_subject(&an_idea("idea_1"), LAID)
        .await
        .expect("forgetting nothing is not a failure");
    catalog
        .forget_place(&a_section("section_1"), LAID)
        .await
        .expect("forgetting nothing is not a failure");
}

async fn seen(
    catalog: &Committing<'_, impl AppearanceCatalog>,
    at: &Place,
    changes: &[(bool, u64)],
) {
    for (present, version) in changes {
        if *present {
            catalog
                .remember(&an_idea("idea_1"), at, *version)
                .await
                .expect("remembering should succeed");
        } else {
            catalog
                .forget(&an_idea("idea_1"), at, *version)
                .await
                .expect("forgetting should succeed");
        }
    }
}

pub async fn a_late_remembering_does_not_undo_a_later_forgetting(
    catalog: &Committing<'_, impl AppearanceCatalog>,
) {
    // when: an idea is remembered in a section, forgotten, and an older remembering comes late
    seen(
        catalog,
        &a_section("section_1"),
        &[(true, 1), (false, 3), (true, 2)],
    )
    .await;

    // then: it appears nowhere
    assert!(
        places_of(catalog, "idea_1").await.is_empty(),
        "an attachment tried again after its detachment must not put the idea back"
    );
}

pub async fn a_late_forgetting_does_not_undo_a_later_remembering(
    catalog: &Committing<'_, impl AppearanceCatalog>,
) {
    // when: an idea is remembered, forgotten, remembered again, and an older forgetting comes late
    seen(
        catalog,
        &a_section("section_1"),
        &[(true, 1), (false, 2), (true, 3), (false, 2)],
    )
    .await;

    // then: it stays in the section
    assert_eq!(
        places_of(catalog, "idea_1").await,
        vec![a_section("section_1")],
        "a detachment tried again after the idea was attached anew must not take it away"
    );
}

pub async fn nothing_comes_back_to_a_forgotten_place(
    catalog: &Committing<'_, impl AppearanceCatalog>,
) {
    // given: an idea remembered in a section
    seen(catalog, &a_section("section_1"), &[(true, 1)]).await;
    // and: the section forgotten
    catalog
        .forget_place(&a_section("section_1"), LAID)
        .await
        .expect("forgetting should succeed");

    // when: the idea is remembered there again at a later version
    seen(catalog, &a_section("section_1"), &[(true, 9)]).await;

    // then: it appears nowhere
    assert!(
        places_of(catalog, "idea_1").await.is_empty(),
        "a removed section is gone for good, whatever arrives about it later"
    );
}

pub async fn a_forgotten_idea_comes_back_nowhere(catalog: &Committing<'_, impl AppearanceCatalog>) {
    // given: an idea remembered in a section
    seen(catalog, &a_section("section_1"), &[(true, 1)]).await;
    // and: the idea forgotten
    catalog
        .forget_subject(&an_idea("idea_1"), LAID)
        .await
        .expect("forgetting should succeed");

    // when: it is remembered in another section at a later version
    seen(catalog, &a_section("section_2"), &[(true, 9)]).await;

    // then: it appears nowhere
    assert!(
        places_of(catalog, "idea_1").await.is_empty(),
        "a discarded idea is gone for good, and its versions come from other streams, so only \
         remembering that it went can keep it out"
    );
}

pub async fn an_idea_s_project_is_kept_and_none_known_is_none(
    catalog: &Committing<'_, impl AppearanceCatalog>,
) {
    // when: two ideas have their projects remembered
    catalog
        .remember_project(&an_idea("idea_1"), &ProjectLink::from("project_1"))
        .await
        .expect("remembering should succeed");
    catalog
        .remember_project(&an_idea("idea_2"), &ProjectLink::from("project_2"))
        .await
        .expect("remembering should succeed");

    // then: each is kept in its project, and an idea never remembered in none
    for (idea, project) in [
        ("idea_1", Some(ProjectLink::from("project_1"))),
        ("idea_2", Some(ProjectLink::from("project_2"))),
        ("idea_never", None),
    ] {
        assert_eq!(
            catalog
                .project_of(&an_idea(idea))
                .await
                .expect("asking should succeed"),
            project,
            "{idea}"
        );
    }
}

pub async fn a_discarded_idea_s_project_outlives_it_as_its_tombstone_does(
    catalog: &Committing<'_, impl AppearanceCatalog>,
) {
    // given: an idea of a project
    catalog
        .remember_project(&an_idea("idea_1"), &ProjectLink::from("project_1"))
        .await
        .expect("remembering should succeed");

    // when: the idea is forgotten
    catalog
        .forget_subject(&an_idea("idea_1"), LAID)
        .await
        .expect("forgetting should succeed");

    // then: its project is still known
    assert_eq!(
        catalog
            .project_of(&an_idea("idea_1"))
            .await
            .expect("asking should succeed"),
        Some(ProjectLink::from("project_1")),
        "whoever followed the idea is told it appears nowhere now, which only who may read its \
         project may hear"
    );
}

pub async fn remembering_an_idea_s_project_again_keeps_one_the_last(
    catalog: &Committing<'_, impl AppearanceCatalog>,
) {
    // when: an idea's project is remembered again and then as another
    for project in ["project_1", "project_1", "project_2"] {
        catalog
            .remember_project(&an_idea("idea_1"), &ProjectLink::from(project))
            .await
            .expect("remembering again is no failure");
    }

    // then: the last one is kept
    assert_eq!(
        catalog
            .project_of(&an_idea("idea_1"))
            .await
            .expect("asking should succeed"),
        Some(ProjectLink::from("project_2"))
    );
}

pub async fn tombstones_laid_before_a_moment_are_forgotten_and_the_later_kept(
    catalog: &Committing<'_, impl AppearanceCatalog>,
) {
    // given: an idea and a section forgotten long ago
    let before = LAID + Duration::DAY;
    catalog
        .forget_subject(&an_idea("idea_long_gone"), LAID)
        .await
        .expect("forgetting should succeed");
    catalog
        .forget_place(&a_section("section_long_gone"), LAID)
        .await
        .expect("forgetting should succeed");
    // and: another idea and section forgotten just now
    catalog
        .forget_subject(&an_idea("idea_just_gone"), before)
        .await
        .expect("forgetting should succeed");
    catalog
        .forget_place(&a_section("section_just_gone"), before)
        .await
        .expect("forgetting should succeed");

    // when: tombstones laid before now are forgotten
    let forgotten = catalog
        .forget_tombstones_laid_before(before, ALL_AT_ONCE)
        .await
        .expect("forgetting tombstones should succeed");

    // then: the two long gone are forgotten
    assert_eq!(forgotten, 2);
    // when: the ideas and sections are remembered again
    for (idea, place) in [
        ("idea_long_gone", a_scene("scene_1")),
        ("idea_just_gone", a_scene("scene_1")),
        ("idea_1", a_section("section_long_gone")),
        ("idea_1", a_section("section_just_gone")),
    ] {
        remembered(catalog, idea, &[place]).await;
    }
    // then: the long gone ones come back
    assert_eq!(
        places_of(catalog, "idea_long_gone").await,
        vec![a_scene("scene_1")],
        "a tombstone forgotten keeps nothing out"
    );
    // and: the ones just gone stay out
    assert!(places_of(catalog, "idea_just_gone").await.is_empty());
    assert_eq!(
        places_of(catalog, "idea_1").await,
        vec![a_section("section_long_gone")]
    );
}

pub async fn a_tombstone_is_laid_once_when_what_it_marks_first_went(
    catalog: &Committing<'_, impl AppearanceCatalog>,
) {
    // given: an idea and a section forgotten
    catalog
        .forget_subject(&an_idea("idea_1"), LAID)
        .await
        .expect("forgetting should succeed");
    catalog
        .forget_place(&a_section("section_1"), LAID)
        .await
        .expect("forgetting should succeed");
    // and: forgotten again later
    catalog
        .forget_subject(&an_idea("idea_1"), LAID + Duration::DAY * 2)
        .await
        .expect("hearing it again is no failure");
    catalog
        .forget_place(&a_section("section_1"), LAID + Duration::DAY * 2)
        .await
        .expect("hearing it again is no failure");

    // then: their tombstones were laid the first time, once each
    assert_eq!(
        catalog
            .forget_tombstones_laid_before(LAID + Duration::DAY, ALL_AT_ONCE)
            .await
            .expect("forgetting tombstones should succeed"),
        2,
        "a discard heard again is the same discard"
    );
}

pub async fn a_long_gone_idea_s_project_is_forgotten_with_its_tombstone(
    catalog: &Committing<'_, impl AppearanceCatalog>,
) {
    // given: three ideas of a project
    for idea in ["idea_long_gone", "idea_just_gone", "idea_here"] {
        catalog
            .remember_project(&an_idea(idea), &ProjectLink::from("project_1"))
            .await
            .expect("remembering should succeed");
    }
    // and: one forgotten long ago and one just now
    catalog
        .forget_subject(&an_idea("idea_long_gone"), LAID)
        .await
        .expect("forgetting should succeed");
    catalog
        .forget_subject(&an_idea("idea_just_gone"), LAID + Duration::DAY)
        .await
        .expect("forgetting should succeed");

    // when: tombstones laid before now are forgotten
    catalog
        .forget_tombstones_laid_before(LAID + Duration::DAY, ALL_AT_ONCE)
        .await
        .expect("forgetting tombstones should succeed");

    // then: only the long gone idea's project is forgotten
    for (idea, project) in [
        ("idea_long_gone", None),
        ("idea_just_gone", Some(ProjectLink::from("project_1"))),
        ("idea_here", Some(ProjectLink::from("project_1"))),
    ] {
        assert_eq!(
            catalog
                .project_of(&an_idea(idea))
                .await
                .expect("asking should succeed"),
            project,
            "{idea}"
        );
    }
}

pub async fn tombstones_are_forgotten_oldest_first_and_at_most_so_many_of_each_kind(
    catalog: &Committing<'_, impl AppearanceCatalog>,
) {
    // given: three ideas and sections forgotten at different moments, each idea of a project
    for (idea, section, laid) in [
        ("idea_newest", "section_newest", LAID + Duration::SECOND * 2),
        ("idea_oldest", "section_oldest", LAID),
        ("idea_older", "section_older", LAID + Duration::SECOND),
    ] {
        catalog
            .remember_project(&an_idea(idea), &ProjectLink::from("project_1"))
            .await
            .expect("remembering should succeed");
        catalog
            .forget_subject(&an_idea(idea), laid)
            .await
            .expect("forgetting should succeed");
        catalog
            .forget_place(&a_section(section), laid)
            .await
            .expect("forgetting should succeed");
    }
    // when: tombstones are forgotten at most two of each kind at a time
    let before = LAID + Duration::DAY;

    let forgotten = catalog
        .forget_tombstones_laid_before(before, 2)
        .await
        .expect("forgetting tombstones should succeed");

    // then: two of each kind are forgotten
    assert_eq!(forgotten, 4, "two of each kind");
    // and: the oldest ideas lose their project
    for (idea, project) in [
        ("idea_newest", Some(ProjectLink::from("project_1"))),
        ("idea_oldest", None),
        ("idea_older", None),
    ] {
        assert_eq!(
            catalog
                .project_of(&an_idea(idea))
                .await
                .expect("asking should succeed"),
            project,
            "{idea}"
        );
    }
    // when: the ideas and sections are remembered again
    for idea in ["idea_newest", "idea_oldest", "idea_older"] {
        remembered(catalog, idea, &[a_scene("scene_1")]).await;
    }
    remembered(
        catalog,
        "idea_here",
        &[
            a_section("section_newest"),
            a_section("section_oldest"),
            a_section("section_older"),
        ],
    )
    .await;
    // then: the oldest ones come back and the newest stays out
    assert!(places_of(catalog, "idea_newest").await.is_empty());
    assert_eq!(
        places_of(catalog, "idea_oldest").await,
        vec![a_scene("scene_1")]
    );
    assert_eq!(
        places_of(catalog, "idea_older").await,
        vec![a_scene("scene_1")]
    );
    assert_eq!(
        places_of(catalog, "idea_here").await,
        vec![a_section("section_older"), a_section("section_oldest")]
    );
    // and: what is left is forgotten in the next batch
    for left in [2, 0] {
        assert_eq!(
            catalog
                .forget_tombstones_laid_before(before, 2)
                .await
                .expect("forgetting tombstones should succeed"),
            left
        );
    }
}

pub async fn what_was_written_in_a_transaction_that_is_dropped_is_gone(
    catalog: &Committing<'_, impl AppearanceCatalog>,
) {
    // given: an idea remembered in a scene and a section
    remembered(
        catalog,
        "idea_kept",
        &[a_scene("scene_1"), a_section("section_1")],
    )
    .await;
    // and: a transaction remembering another idea and its project, and forgetting the first idea, its section and the scene
    let mut transaction = catalog.begun().await;
    let uncommitted = catalog.uncommitted();
    uncommitted
        .remember(
            &mut transaction,
            &an_idea("idea_new"),
            &a_scene("scene_1"),
            1,
        )
        .await
        .expect("remembering should succeed");
    uncommitted
        .remember_project(
            &mut transaction,
            &an_idea("idea_new"),
            &ProjectLink::from("project_1"),
        )
        .await
        .expect("remembering should succeed");
    uncommitted
        .forget(
            &mut transaction,
            &an_idea("idea_kept"),
            &a_section("section_1"),
            2,
        )
        .await
        .expect("forgetting should succeed");
    uncommitted
        .forget_place(&mut transaction, &a_scene("scene_1"), LAID)
        .await
        .expect("forgetting should succeed");
    uncommitted
        .forget_subject(&mut transaction, &an_idea("idea_kept"), LAID)
        .await
        .expect("forgetting should succeed");

    // when: the transaction is dropped
    drop(transaction);

    // then: the first idea stays where it was
    assert_eq!(
        places_of(catalog, "idea_kept").await,
        vec![a_scene("scene_1"), a_section("section_1")]
    );
    // and: the other is nowhere and of no project
    assert!(places_of(catalog, "idea_new").await.is_empty());
    assert_eq!(
        catalog
            .project_of(&an_idea("idea_new"))
            .await
            .expect("asking should succeed"),
        None
    );
}

pub async fn an_idea_appears_only_in_places_of_its_own_project(
    catalog: &Committing<'_, impl AppearanceCatalog>,
) {
    // given: an idea of one project
    let idea = an_idea("idea_1");
    catalog
        .remember_project(&idea, &ProjectLink::from("project_1"))
        .await
        .expect("remembering should succeed");
    // and: a scene of that project, a scene of another and a section of no project known
    for (scene, project) in [("scene_own", "project_1"), ("scene_strange", "project_2")] {
        catalog
            .remember_place_project(&a_scene(scene), &ProjectLink::from(project))
            .await
            .expect("remembering should succeed");
    }

    // when: the idea is linked from each
    for place in [
        a_scene("scene_own"),
        a_scene("scene_strange"),
        a_section("section_unknown"),
    ] {
        catalog
            .remember_alone(&idea, &place, 1)
            .await
            .expect("remembering should succeed");
    }

    // then: it appears in the scene of its own project alone
    assert_eq!(
        places_of(catalog, "idea_1").await,
        vec![a_scene("scene_own")],
        "a link from another project, made with an idea's id known from elsewhere, must not show \
         where the idea appears"
    );
}

pub async fn an_idea_of_no_project_known_appears_nowhere_until_its_project_is(
    catalog: &Committing<'_, impl AppearanceCatalog>,
) {
    // given: an idea linked from a scene of a project before its own project is known
    catalog
        .remember_place_project(&a_scene("scene_1"), &ProjectLink::from("project_1"))
        .await
        .expect("remembering should succeed");
    catalog
        .remember_alone(&an_idea("idea_1"), &a_scene("scene_1"), 1)
        .await
        .expect("remembering should succeed");
    // and: so it appears nowhere
    assert!(places_of(catalog, "idea_1").await.is_empty());

    // when: its project is known, the scene's
    catalog
        .remember_project(&an_idea("idea_1"), &ProjectLink::from("project_1"))
        .await
        .expect("remembering should succeed");

    // then: it appears there, whichever was heard first
    assert_eq!(places_of(catalog, "idea_1").await, vec![a_scene("scene_1")]);
}

pub async fn a_forgotten_place_keeps_no_project_not_even_one_heard_late(
    catalog: &Committing<'_, impl AppearanceCatalog>,
) {
    // given: an idea of a project and a scene of the same
    let (idea, scene, project) = (
        an_idea("idea_1"),
        a_scene("scene_1"),
        ProjectLink::from("project_1"),
    );
    catalog
        .remember_project(&idea, &project)
        .await
        .expect("remembering should succeed");
    catalog
        .remember_place_project(&scene, &project)
        .await
        .expect("remembering should succeed");
    // and: the scene forgotten
    catalog
        .forget_place(&scene, LAID)
        .await
        .expect("forgetting should succeed");
    // and: its project heard again, late
    catalog
        .remember_place_project(&scene, &project)
        .await
        .expect("remembering late is no failure");
    // and: its tombstone forgotten long after
    catalog
        .forget_tombstones_laid_before(LAID + Duration::DAY, ALL_AT_ONCE)
        .await
        .expect("forgetting tombstones should succeed");

    // when: the idea is remembered there once more
    catalog
        .remember_alone(&idea, &scene, 9)
        .await
        .expect("remembering should succeed");

    // then: it appears nowhere, the scene's project having gone with it
    assert!(
        places_of(catalog, "idea_1").await.is_empty(),
        "a place's project is forgotten with the place, and one heard after it went is never kept"
    );
}

pub async fn forgetting_a_project_forgets_every_place_of_it_for_good(
    catalog: &Committing<'_, impl AppearanceCatalog>,
) {
    // given: an idea in a scene and a section of `PROJECT`
    remembered(
        catalog,
        "idea_1",
        &[a_scene("scene_1"), a_section("section_1")],
    )
    .await;
    // and: an idea of another project in a scene of it
    let other = ProjectLink::from("project_2");
    catalog
        .remember_project(&an_idea("idea_2"), &other)
        .await
        .expect("remembering should succeed");
    catalog
        .remember_place_project(&a_scene("scene_2"), &other)
        .await
        .expect("remembering should succeed");
    catalog
        .remember_alone(&an_idea("idea_2"), &a_scene("scene_2"), 1)
        .await
        .expect("remembering should succeed");

    // when: `PROJECT` is forgotten
    let mut transaction = catalog.begun().await;
    let appeared = catalog
        .uncommitted()
        .forget_places_of_project(&mut transaction, &ProjectLink::from(PROJECT), LAID)
        .await
        .expect("forgetting should succeed");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");

    // then: the idea that appeared in it is told, once
    assert_eq!(appeared, vec![an_idea("idea_1")]);
    // and: it appears nowhere
    assert!(places_of(catalog, "idea_1").await.is_empty());
    // and: the idea of the other project stays where it was
    assert_eq!(places_of(catalog, "idea_2").await, vec![a_scene("scene_2")]);
    // when: a link to the section is heard late, its project with it
    catalog
        .remember(&an_idea("idea_1"), &a_section("section_1"), 9)
        .await
        .expect("remembering late is no failure");
    // then: nothing comes back to it
    assert!(
        places_of(catalog, "idea_1").await.is_empty(),
        "every place of a deleted project is gone for good, as a removed section is"
    );
}

pub async fn tombstones_forgotten_in_a_transaction_that_is_dropped_still_stand(
    catalog: &Committing<'_, impl AppearanceCatalog>,
) {
    // given: a forgotten idea
    catalog
        .forget_subject(&an_idea("idea_gone"), LAID)
        .await
        .expect("forgetting should succeed");
    // and: a transaction forgetting its tombstone
    let mut transaction = catalog.begun().await;
    catalog
        .uncommitted()
        .forget_tombstones_laid_before(&mut transaction, LAID + Duration::seconds(1), ALL_AT_ONCE)
        .await
        .expect("forgetting tombstones should succeed");

    // when: the transaction is dropped
    drop(transaction);
    // and: the idea is remembered again
    remembered(catalog, "idea_gone", &[a_scene("scene_1")]).await;

    // then: it stays out
    assert!(places_of(catalog, "idea_gone").await.is_empty());
}

macro_rules! conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::suite::$case(&bench.committing()).await;
            bench.cleanup().await;
        }
    };
}

macro_rules! conformance_tests {
    ($workbench:ty) => {
        $crate::suite::conformance_case!(
            $workbench,
            what_was_written_in_a_transaction_that_is_dropped_is_gone
        );
        $crate::suite::conformance_case!(
            $workbench,
            tombstones_forgotten_in_a_transaction_that_is_dropped_still_stand
        );
        $crate::suite::conformance_case!($workbench, an_idea_nobody_placed_appears_nowhere);
        $crate::suite::conformance_case!(
            $workbench,
            an_idea_appears_only_in_places_of_its_own_project
        );
        $crate::suite::conformance_case!(
            $workbench,
            an_idea_of_no_project_known_appears_nowhere_until_its_project_is
        );
        $crate::suite::conformance_case!(
            $workbench,
            a_forgotten_place_keeps_no_project_not_even_one_heard_late
        );
        $crate::suite::conformance_case!(
            $workbench,
            forgetting_a_project_forgets_every_place_of_it_for_good
        );
        $crate::suite::conformance_case!(
            $workbench,
            an_idea_s_project_is_kept_and_none_known_is_none
        );
        $crate::suite::conformance_case!(
            $workbench,
            remembering_an_idea_s_project_again_keeps_one_the_last
        );
        $crate::suite::conformance_case!(
            $workbench,
            a_discarded_idea_s_project_outlives_it_as_its_tombstone_does
        );
        $crate::suite::conformance_case!($workbench, an_idea_appears_wherever_it_was_remembered);
        $crate::suite::conformance_case!(
            $workbench,
            places_come_back_in_one_order_whatever_order_they_were_remembered_in
        );
        $crate::suite::conformance_case!(
            $workbench,
            remembering_the_same_place_twice_is_one_appearance
        );
        $crate::suite::conformance_case!($workbench, forgetting_one_place_leaves_the_others);
        $crate::suite::conformance_case!(
            $workbench,
            a_late_remembering_does_not_undo_a_later_forgetting
        );
        $crate::suite::conformance_case!(
            $workbench,
            a_late_forgetting_does_not_undo_a_later_remembering
        );
        $crate::suite::conformance_case!($workbench, nothing_comes_back_to_a_forgotten_place);
        $crate::suite::conformance_case!($workbench, a_forgotten_idea_comes_back_nowhere);
        $crate::suite::conformance_case!($workbench, forgetting_an_idea_leaves_every_other_idea);
        $crate::suite::conformance_case!($workbench, forgetting_a_place_reaches_every_idea_in_it);
        $crate::suite::conformance_case!(
            $workbench,
            forgetting_a_place_says_which_ideas_appeared_there
        );
        $crate::suite::conformance_case!(
            $workbench,
            a_section_and_a_scene_sharing_an_id_are_different_places
        );
        $crate::suite::conformance_case!(
            $workbench,
            forgetting_what_was_never_remembered_is_harmless
        );
        $crate::suite::conformance_case!(
            $workbench,
            tombstones_laid_before_a_moment_are_forgotten_and_the_later_kept
        );
        $crate::suite::conformance_case!(
            $workbench,
            a_tombstone_is_laid_once_when_what_it_marks_first_went
        );
        $crate::suite::conformance_case!(
            $workbench,
            a_long_gone_idea_s_project_is_forgotten_with_its_tombstone
        );
        $crate::suite::conformance_case!(
            $workbench,
            tombstones_are_forgotten_oldest_first_and_at_most_so_many_of_each_kind
        );
    };
}

pub(crate) use {conformance_case, conformance_tests};
