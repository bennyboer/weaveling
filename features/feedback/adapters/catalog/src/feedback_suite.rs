use eventsourcing::{AccountId, Agent};
use feedback_core::{
    About, Asked, CatalogError, CataloguedFeedback, Comment, CommentBlock, CommentRun, Counted,
    FeedbackCatalog, FeedbackId, Order, Page, Places, ProjectLink, ReadingCopyLink,
    ReadingCopyPlace, SceneLink, SectionLink, Status, Subject,
};
use time::OffsetDateTime;
use transacting::Tried;

use crate::suite::{Committing, unreachable};

impl<C: FeedbackCatalog> Committing<'_, C> {
    pub async fn keep_piece(&self, piece: &CataloguedFeedback) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .keep(transaction, piece)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn forget_piece(&self, piece: &FeedbackId) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .forget(transaction, piece)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }
}

fn at(seconds: i64) -> OffsetDateTime {
    clock::at(seconds)
}

fn ada() -> Agent {
    Agent::Account(AccountId::from("account_ada"))
}

fn bea() -> Agent {
    Agent::Account(AccountId::from("account_bea"))
}

fn saying(text: &str) -> Comment {
    Comment::new(vec![CommentBlock::Paragraph(vec![CommentRun::Text {
        text: text.to_owned(),
        emphasis: false,
        strong: false,
        link: None,
    }])])
    .expect("a plain comment is fine")
}

fn in_copy(copy: &str, place: ReadingCopyPlace) -> About {
    About::ReadingCopy {
        reading_copy: ReadingCopyLink::from(copy),
        place,
    }
}

fn on_scene(scene: &str) -> ReadingCopyPlace {
    ReadingCopyPlace::Scene(SceneLink::from(scene))
}

fn on_section(section: &str) -> ReadingCopyPlace {
    ReadingCopyPlace::Section(SectionLink::from(section))
}

/// Ada's draft in the loom copy of the loom project, written at a time.
fn a_draft(about: About, written_at: i64) -> CataloguedFeedback {
    CataloguedFeedback {
        id: FeedbackId::generate(at(written_at)),
        project: ProjectLink::from("project_loom"),
        about,
        comment: saying("Slow."),
        written_by: ada(),
        written_at: at(written_at),
        submitted_at: None,
        edited: false,
        done: false,
    }
}

fn submitted_at(mut piece: CataloguedFeedback, seconds: i64) -> CataloguedFeedback {
    piece.submitted_at = Some(at(seconds));
    piece
}

fn by(mut piece: CataloguedFeedback, writer: Agent) -> CataloguedFeedback {
    piece.written_by = writer;
    piece
}

fn in_project(mut piece: CataloguedFeedback, project: &str) -> CataloguedFeedback {
    piece.project = ProjectLink::from(project);
    piece
}

fn done(mut piece: CataloguedFeedback) -> CataloguedFeedback {
    piece.done = true;
    piece
}

async fn kept(catalog: &Committing<'_, impl FeedbackCatalog>, pieces: &[&CataloguedFeedback]) {
    for piece in pieces {
        catalog
            .keep_piece(piece)
            .await
            .expect("keeping should succeed");
    }
}

fn ids(pieces: &[CataloguedFeedback]) -> Vec<FeedbackId> {
    pieces.iter().map(|piece| piece.id).collect()
}

fn in_the_loom() -> Asked {
    Asked {
        project: Some("project_loom".to_owned()),
        ..Asked::default()
    }
}

async fn submitted(
    catalog: &Committing<'_, impl FeedbackCatalog>,
    asked: &Asked,
    status: Option<Status>,
    page: &Page,
) -> Vec<FeedbackId> {
    ids(&catalog
        .uncommitted()
        .submitted(asked, status, page)
        .await
        .expect("asking should succeed"))
}

pub async fn what_was_written_in_a_transaction_that_is_dropped_is_gone(
    catalog: &Committing<'_, impl FeedbackCatalog>,
) {
    // given: a piece kept
    let kept_before = a_draft(in_copy("copy_loom", ReadingCopyPlace::Whole), 10);
    kept(catalog, &[&kept_before]).await;

    // when: in a transaction that is dropped, another piece is kept and the first forgotten
    {
        let mut transaction = catalog.begun().await;
        catalog
            .uncommitted()
            .keep(
                &mut transaction,
                &a_draft(in_copy("copy_loom", ReadingCopyPlace::Whole), 20),
            )
            .await
            .expect("keeping should succeed");
        catalog
            .uncommitted()
            .forget(&mut transaction, &kept_before.id)
            .await
            .expect("forgetting should succeed");
    }

    // then: only what was there before stands
    assert_eq!(
        catalog
            .uncommitted()
            .drafts_of(&in_the_loom(), &ada())
            .await
            .expect("asking should succeed"),
        [kept_before]
    );
}

pub async fn a_piece_kept_again_stands_as_kept_last_and_a_forgotten_one_is_gone(
    catalog: &Committing<'_, impl FeedbackCatalog>,
) {
    // given: a draft kept, and another
    let draft = a_draft(in_copy("copy_loom", ReadingCopyPlace::Whole), 10);
    let other = a_draft(in_copy("copy_loom", on_scene("scene_harbour")), 20);
    kept(catalog, &[&draft, &other]).await;

    // when: the first is kept again, reworded
    let mut reworded = draft.clone();
    reworded.comment = saying("Too slow.");
    kept(catalog, &[&reworded]).await;

    // then: it is found as kept last
    assert_eq!(
        catalog
            .uncommitted()
            .piece(&draft.id)
            .await
            .expect("asking should succeed"),
        Some(reworded)
    );

    // when: the other is forgotten
    catalog
        .forget_piece(&other.id)
        .await
        .expect("forgetting should succeed");

    // then: it is not found
    assert_eq!(
        catalog
            .uncommitted()
            .piece(&other.id)
            .await
            .expect("asking should succeed"),
        None
    );
}

pub async fn drafts_are_their_writer_s_alone_where_asked_oldest_first(
    catalog: &Committing<'_, impl FeedbackCatalog>,
) {
    // given: two drafts of ada written in the opposite order they are kept, one in another copy, bea's draft, and ada's feedback submitted
    let later = a_draft(in_copy("copy_loom", on_scene("scene_harbour")), 30);
    let earlier = a_draft(in_copy("copy_loom", ReadingCopyPlace::Whole), 10);
    let elsewhere = a_draft(in_copy("copy_weft", ReadingCopyPlace::Whole), 20);
    let beas = by(
        a_draft(in_copy("copy_loom", ReadingCopyPlace::Whole), 15),
        bea(),
    );
    let given = submitted_at(a_draft(in_copy("copy_loom", ReadingCopyPlace::Whole), 5), 6);
    kept(catalog, &[&later, &earlier, &elsewhere, &beas, &given]).await;

    // when: ada's drafts in the loom copy are asked for
    let drafts = catalog
        .uncommitted()
        .drafts_of(
            &Asked {
                subjects: Some(vec![Subject::ReadingCopy(ReadingCopyLink::from(
                    "copy_loom",
                ))]),
                ..Asked::default()
            },
            &ada(),
        )
        .await
        .expect("asking should succeed");

    // then: ada's in it, oldest first, and neither bea's nor what was submitted
    assert_eq!(ids(&drafts), [earlier.id, later.id]);
}

pub async fn feedback_submitted_is_narrowed_by_everything_named_and_by_status(
    catalog: &Committing<'_, impl FeedbackCatalog>,
) {
    // given: feedback submitted on a scene and a section of the loom copy, bea's on the weft copy, one done, one in another project, and a draft
    let on_harbour = submitted_at(
        a_draft(in_copy("copy_loom", on_scene("scene_harbour")), 1),
        10,
    );
    let on_morning = submitted_at(
        a_draft(in_copy("copy_loom", on_section("section_morning")), 2),
        20,
    );
    let beas = by(
        submitted_at(
            a_draft(in_copy("copy_weft", ReadingCopyPlace::Whole), 3),
            30,
        ),
        bea(),
    );
    let done_one = done(submitted_at(
        a_draft(in_copy("copy_loom", ReadingCopyPlace::Whole), 4),
        40,
    ));
    let elsewhere = in_project(
        submitted_at(
            a_draft(in_copy("copy_shuttle", on_scene("scene_harbour")), 5),
            50,
        ),
        "project_shuttle",
    );
    let draft = a_draft(in_copy("copy_loom", on_scene("scene_harbour")), 6);
    kept(
        catalog,
        &[
            &on_harbour,
            &on_morning,
            &beas,
            &done_one,
            &elsewhere,
            &draft,
        ],
    )
    .await;
    let all = Page::default();

    // expect: the project's, every submitted one of it and no draft
    assert_eq!(
        submitted(catalog, &in_the_loom(), None, &all).await,
        [on_harbour.id, on_morning.id, beas.id, done_one.id]
    );
    // and: what is pending in it, and what is done
    assert_eq!(
        submitted(catalog, &in_the_loom(), Some(Status::Pending), &all).await,
        [on_harbour.id, on_morning.id, beas.id]
    );
    assert_eq!(
        submitted(catalog, &in_the_loom(), Some(Status::Done), &all).await,
        [done_one.id]
    );
    // and: in the loom copy alone
    let in_the_loom_copy = Asked {
        subjects: Some(vec![Subject::ReadingCopy(ReadingCopyLink::from(
            "copy_loom",
        ))]),
        ..in_the_loom()
    };
    assert_eq!(
        submitted(catalog, &in_the_loom_copy, None, &all).await,
        [on_harbour.id, on_morning.id, done_one.id]
    );
    // and: on the harbour scene or the morning section, in every project
    let at_places = Asked {
        places: Some(Places::Text {
            scenes: vec![SceneLink::from("scene_harbour")],
            sections: vec![SectionLink::from("section_morning")],
        }),
        ..Asked::default()
    };
    assert_eq!(
        submitted(catalog, &at_places, None, &all).await,
        [on_harbour.id, on_morning.id, elsewhere.id]
    );
    // and: given by bea alone
    let by_bea = Asked {
        given_by: Some(vec![bea()]),
        ..in_the_loom()
    };
    assert_eq!(submitted(catalog, &by_bea, None, &all).await, [beas.id]);
}

pub async fn feedback_submitted_is_paged_oldest_or_newest_first_and_pieces_submitted_together_follow_the_order_written(
    catalog: &Committing<'_, impl FeedbackCatalog>,
) {
    // given: three pieces submitted together, kept in another order than they were written, and one submitted later
    let written_at = |seconds: i64| a_draft(in_copy("copy_loom", ReadingCopyPlace::Whole), seconds);
    let first = submitted_at(written_at(1), 10);
    let second = submitted_at(written_at(2), 10);
    let third = submitted_at(written_at(3), 10);
    let last = submitted_at(written_at(4), 20);
    kept(catalog, &[&third, &first, &last, &second]).await;

    // expect: oldest first, those submitted together in the order written
    assert_eq!(
        submitted(catalog, &in_the_loom(), None, &Page::default()).await,
        [first.id, second.id, third.id, last.id]
    );
    // and: newest first, two to a page, then the page after the second
    let newest = |after: Option<FeedbackId>| Page {
        order: Order::Newest,
        after: after.map(|id| id.to_string()),
        at_most: Some(2),
    };
    let page = submitted(catalog, &in_the_loom(), None, &newest(None)).await;
    assert_eq!(page, [last.id, third.id]);
    assert_eq!(
        submitted(catalog, &in_the_loom(), None, &newest(Some(page[1]))).await,
        [second.id, first.id]
    );
}

pub async fn what_is_pending_is_counted_by_scene_by_section_and_in_all(
    catalog: &Committing<'_, impl FeedbackCatalog>,
) {
    // given: two pieces pending on a scene, one on a section, one on the whole copy, one done, and a draft
    for piece in [
        submitted_at(
            a_draft(in_copy("copy_loom", on_scene("scene_harbour")), 1),
            10,
        ),
        submitted_at(
            a_draft(in_copy("copy_weft", on_scene("scene_harbour")), 2),
            20,
        ),
        submitted_at(
            a_draft(in_copy("copy_loom", on_section("section_morning")), 3),
            30,
        ),
        submitted_at(
            a_draft(in_copy("copy_loom", ReadingCopyPlace::Whole), 4),
            40,
        ),
        done(submitted_at(
            a_draft(in_copy("copy_loom", on_scene("scene_harbour")), 5),
            50,
        )),
        a_draft(in_copy("copy_loom", on_scene("scene_harbour")), 6),
    ] {
        kept(catalog, &[&piece]).await;
    }

    // when: what is pending in the project is counted
    let pending = catalog
        .uncommitted()
        .pending(&in_the_loom())
        .await
        .expect("counting should succeed");

    // then: two on the scene, one on the section, and four in all
    assert_eq!(
        pending.scenes,
        [(SceneLink::from("scene_harbour"), 2)].into()
    );
    assert_eq!(
        pending.sections,
        [(SectionLink::from("section_morning"), 1)].into()
    );
    assert_eq!(pending.total, 4);
}

pub async fn the_givers_in_a_project_are_listed_in_the_order_they_first_gave_any(
    catalog: &Committing<'_, impl FeedbackCatalog>,
) {
    // given: bea submitting after ada and before ada again, a draft of someone else, and a giver in another project
    let otto = Agent::Account(AccountId::from("account_otto"));
    let hedy = Agent::Account(AccountId::from("account_hedy"));
    for piece in [
        by(
            submitted_at(
                a_draft(in_copy("copy_loom", ReadingCopyPlace::Whole), 1),
                30,
            ),
            ada(),
        ),
        by(
            submitted_at(
                a_draft(in_copy("copy_loom", ReadingCopyPlace::Whole), 2),
                20,
            ),
            bea(),
        ),
        by(
            submitted_at(
                a_draft(in_copy("copy_loom", ReadingCopyPlace::Whole), 3),
                10,
            ),
            ada(),
        ),
        by(
            a_draft(in_copy("copy_loom", ReadingCopyPlace::Whole), 4),
            otto,
        ),
        in_project(
            by(
                submitted_at(
                    a_draft(in_copy("copy_shuttle", ReadingCopyPlace::Whole), 5),
                    5,
                ),
                hedy,
            ),
            "project_shuttle",
        ),
    ] {
        kept(catalog, &[&piece]).await;
    }

    // expect: ada, who gave first, then bea
    assert_eq!(
        catalog
            .uncommitted()
            .givers_in(&ProjectLink::from("project_loom"))
            .await
            .expect("asking should succeed"),
        [ada(), bea()]
    );
}

pub async fn the_givers_follow_their_pieces_as_they_are_submitted_and_deleted(
    catalog: &Committing<'_, impl FeedbackCatalog>,
) {
    // given: ada giving first, bea next, ada again later, and otto's draft
    let otto = Agent::Account(AccountId::from("account_otto"));
    let adas_first = by(
        submitted_at(
            a_draft(in_copy("copy_loom", ReadingCopyPlace::Whole), 1),
            10,
        ),
        ada(),
    );
    let beas = by(
        submitted_at(
            a_draft(in_copy("copy_loom", ReadingCopyPlace::Whole), 2),
            20,
        ),
        bea(),
    );
    let adas_later = by(
        submitted_at(
            a_draft(in_copy("copy_loom", ReadingCopyPlace::Whole), 3),
            30,
        ),
        ada(),
    );
    let ottos = by(
        a_draft(in_copy("copy_loom", ReadingCopyPlace::Whole), 4),
        otto.clone(),
    );
    kept(catalog, &[&adas_first, &beas, &adas_later, &ottos]).await;
    let givers = || async {
        catalog
            .uncommitted()
            .givers_in(&ProjectLink::from("project_loom"))
            .await
            .expect("asking should succeed")
    };

    // when: ada's first piece is deleted
    catalog
        .forget_piece(&adas_first.id)
        .await
        .expect("forgetting should succeed");
    // then: bea gave first now, and ada after her by the later piece
    assert_eq!(givers().await, [bea(), ada()]);

    // when: bea's only piece is deleted, and otto submits the draft
    catalog
        .forget_piece(&beas.id)
        .await
        .expect("forgetting should succeed");
    kept(catalog, &[&submitted_at(ottos.clone(), 40)]).await;
    // then: bea gave nothing any more, and otto is listed after ada
    assert_eq!(givers().await, [ada(), otto]);
}

pub async fn each_subject_counts_what_is_not_done_and_the_asker_s_drafts_in_the_order_asked(
    catalog: &Committing<'_, impl FeedbackCatalog>,
) {
    // given: in the loom copy two pieces not done, one done, a draft of ada and one of bea; in the weft copy one draft of ada
    for piece in [
        submitted_at(
            a_draft(in_copy("copy_loom", ReadingCopyPlace::Whole), 1),
            10,
        ),
        submitted_at(
            a_draft(in_copy("copy_loom", on_scene("scene_harbour")), 2),
            20,
        ),
        done(submitted_at(
            a_draft(in_copy("copy_loom", ReadingCopyPlace::Whole), 3),
            30,
        )),
        a_draft(in_copy("copy_loom", ReadingCopyPlace::Whole), 4),
        by(
            a_draft(in_copy("copy_loom", ReadingCopyPlace::Whole), 5),
            bea(),
        ),
        a_draft(in_copy("copy_weft", ReadingCopyPlace::Whole), 6),
    ] {
        kept(catalog, &[&piece]).await;
    }
    let copy = |named: &str| Subject::ReadingCopy(ReadingCopyLink::from(named));

    // when: ada counts the weft, the loom and the shuttle copy
    let counted = catalog
        .uncommitted()
        .counted_on(
            &[copy("copy_weft"), copy("copy_loom"), copy("copy_shuttle")],
            &ada(),
        )
        .await
        .expect("counting should succeed");

    // then: each in the order asked, the shuttle copy holding nothing
    assert_eq!(
        counted,
        [
            Counted {
                subject: copy("copy_weft"),
                not_done: 0,
                your_drafts: 1,
            },
            Counted {
                subject: copy("copy_loom"),
                not_done: 2,
                your_drafts: 1,
            },
            Counted {
                subject: copy("copy_shuttle"),
                not_done: 0,
                your_drafts: 0,
            },
        ]
    );
}

macro_rules! feedback_conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::feedback_suite::$case(&bench.committing()).await;
            bench.cleanup().await;
        }
    };
}

macro_rules! feedback_conformance_tests {
    ($workbench:ty) => {
        $crate::feedback_suite::feedback_conformance_case!(
            $workbench,
            what_was_written_in_a_transaction_that_is_dropped_is_gone
        );
        $crate::feedback_suite::feedback_conformance_case!(
            $workbench,
            a_piece_kept_again_stands_as_kept_last_and_a_forgotten_one_is_gone
        );
        $crate::feedback_suite::feedback_conformance_case!(
            $workbench,
            drafts_are_their_writer_s_alone_where_asked_oldest_first
        );
        $crate::feedback_suite::feedback_conformance_case!(
            $workbench,
            feedback_submitted_is_narrowed_by_everything_named_and_by_status
        );
        $crate::feedback_suite::feedback_conformance_case!(
            $workbench,
            feedback_submitted_is_paged_oldest_or_newest_first_and_pieces_submitted_together_follow_the_order_written
        );
        $crate::feedback_suite::feedback_conformance_case!(
            $workbench,
            what_is_pending_is_counted_by_scene_by_section_and_in_all
        );
        $crate::feedback_suite::feedback_conformance_case!(
            $workbench,
            the_givers_in_a_project_are_listed_in_the_order_they_first_gave_any
        );
        $crate::feedback_suite::feedback_conformance_case!(
            $workbench,
            the_givers_follow_their_pieces_as_they_are_submitted_and_deleted
        );
        $crate::feedback_suite::feedback_conformance_case!(
            $workbench,
            each_subject_counts_what_is_not_done_and_the_asker_s_drafts_in_the_order_asked
        );
    };
}

pub(crate) use {feedback_conformance_case, feedback_conformance_tests};
