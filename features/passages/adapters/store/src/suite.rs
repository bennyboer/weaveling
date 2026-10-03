use passages_core::{
    FRAGMENT, Passage, PassageId, PassageStore, PassageTitle, ProjectLink, StoreError,
};
use time::{Duration, OffsetDateTime};
use yrs::{Doc, ReadTxn, StateVector, Transact, XmlElementPrelim, XmlFragment, XmlTextPrelim};

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: PassageStore;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

    async fn cleanup(self);
}

pub fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

pub fn an_id(seconds: i64) -> PassageId {
    PassageId::generate(at(seconds))
}

pub fn a_paragraph(saying: &str) -> Vec<u8> {
    let doc = Doc::new();
    let fragment = doc.get_or_insert_xml_fragment(FRAGMENT);
    {
        let mut txn = doc.transact_mut();
        let paragraph = fragment.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
        paragraph.insert(&mut txn, 0, XmlTextPrelim::new(saying));
    }

    doc.transact()
        .encode_state_as_update_v1(&StateVector::default())
}

pub fn a_project() -> ProjectLink {
    ProjectLink::from("project_1")
}

pub fn a_passage(id: PassageId, saying: &str) -> Passage {
    let passage = Passage::empty(id, a_project());
    passage
        .apply(&a_paragraph(saying))
        .expect("sample prose should apply");

    passage
}

pub async fn create_then_load_returns_the_prose(store: &impl PassageStore) {
    let id = an_id(1_000);

    store
        .create(&a_passage(id, "The loom stood silent."))
        .await
        .expect("create should succeed");
    let found = store.load(id).await.expect("load should find the passage");

    assert_eq!(found.text(), "The loom stood silent.");
    assert_eq!(found.id(), id);
}

pub async fn an_empty_passage_can_be_stored(store: &impl PassageStore) {
    let id = an_id(1_000);

    store
        .create(&Passage::empty(id, a_project()))
        .await
        .expect("create should succeed");

    let found = store.load(id).await.expect("load should find the passage");
    assert_eq!(found.text(), "");
}

pub async fn load_missing_passage_is_not_found(store: &impl PassageStore) {
    let missing = an_id(1_000);

    let error = store
        .load(missing)
        .await
        .expect_err("load should not find the passage");

    assert!(
        matches!(&error, StoreError::NotFound(id) if *id == missing),
        "expected NotFound({missing}), got {error:?}"
    );
}

pub async fn create_rejects_a_duplicate_id(store: &impl PassageStore) {
    let id = an_id(1_000);
    store
        .create(&a_passage(id, "The loom stood silent."))
        .await
        .expect("first create should succeed");

    let error = store
        .create(&a_passage(id, "Something else entirely."))
        .await
        .expect_err("second create should conflict");

    assert!(
        matches!(&error, StoreError::Conflict(existing) if *existing == id),
        "expected Conflict({id}), got {error:?}"
    );
}

pub async fn a_rejected_create_leaves_the_stored_passage_untouched(store: &impl PassageStore) {
    let id = an_id(1_000);
    store
        .create(&a_passage(id, "The loom stood silent."))
        .await
        .expect("first create should succeed");

    let _ = store
        .create(&a_passage(id, "Something else entirely."))
        .await;

    let found = store.load(id).await.expect("load should find the passage");
    assert_eq!(found.text(), "The loom stood silent.");
}

pub async fn absorb_missing_passage_is_not_found(store: &impl PassageStore) {
    let missing = an_id(1_000);

    let error = store
        .apply(missing, &a_paragraph("into the void"))
        .await
        .expect_err("apply should not find the passage");

    assert!(
        matches!(&error, StoreError::NotFound(id) if *id == missing),
        "expected NotFound({missing}), got {error:?}"
    );
}

pub async fn absorbed_updates_are_visible_on_load(store: &impl PassageStore) {
    let id = an_id(1_000);
    store
        .create(&Passage::empty(id, a_project()))
        .await
        .expect("create should succeed");

    store
        .apply(id, &a_paragraph("The loom stood silent."))
        .await
        .expect("apply should succeed");

    let found = store.load(id).await.expect("load should find the passage");
    assert_eq!(found.text(), "The loom stood silent.");
}

pub async fn absorbing_the_same_update_twice_changes_nothing(store: &impl PassageStore) {
    let id = an_id(1_000);
    store
        .create(&Passage::empty(id, a_project()))
        .await
        .expect("create should succeed");
    let update = a_paragraph("The loom stood silent.");

    store.apply(id, &update).await.expect("first apply");
    store.apply(id, &update).await.expect("second apply");

    let found = store.load(id).await.expect("load should find the passage");
    assert_eq!(
        found.text(),
        "The loom stood silent.",
        "a repeated update must not duplicate the prose"
    );
}

pub async fn updates_absorbed_in_either_order_converge(store: &impl PassageStore) {
    let ada = a_paragraph("Ada wrote this.");
    let bo = a_paragraph("Bo wrote this.");
    let one = an_id(1_000);
    let other = an_id(2_000);
    for id in [one, other] {
        store
            .create(&Passage::empty(id, a_project()))
            .await
            .expect("create should succeed");
    }

    store.apply(one, &ada).await.expect("apply ada");
    store.apply(one, &bo).await.expect("apply bo");
    store.apply(other, &bo).await.expect("apply bo");
    store.apply(other, &ada).await.expect("apply ada");

    let first = store.load(one).await.expect("load one").text();
    let second = store.load(other).await.expect("load other").text();
    assert_eq!(first, second, "apply order must not matter");
    assert!(first.contains("Ada wrote this."));
    assert!(first.contains("Bo wrote this."));
}

pub async fn an_unusable_update_is_refused_and_changes_nothing(store: &impl PassageStore) {
    let id = an_id(1_000);
    store
        .create(&a_passage(id, "The loom stood silent."))
        .await
        .expect("create should succeed");

    let error = store
        .apply(id, &[255, 255, 255, 255])
        .await
        .expect_err("garbage must not be accepted");

    assert!(
        matches!(&error, StoreError::Unusable(bad) if *bad == id),
        "expected Unusable({id}), got {error:?}"
    );
    let found = store.load(id).await.expect("load should find the passage");
    assert_eq!(
        found.text(),
        "The loom stood silent.",
        "a refused update must leave the passage as it was"
    );
}

pub async fn delete_removes_the_passage(store: &impl PassageStore) {
    let id = an_id(1_000);
    store
        .create(&a_passage(id, "The loom stood silent."))
        .await
        .expect("create should succeed");

    store.delete(id).await.expect("delete should succeed");

    let found = store.load(id).await;
    assert!(
        matches!(&found, Err(StoreError::NotFound(gone)) if *gone == id),
        "expected NotFound({id}) after delete, got {found:?}"
    );
}

pub async fn delete_missing_passage_is_not_found(store: &impl PassageStore) {
    let missing = an_id(1_000);

    let error = store
        .delete(missing)
        .await
        .expect_err("delete should not find the passage");

    assert!(
        matches!(&error, StoreError::NotFound(id) if *id == missing),
        "expected NotFound({missing}), got {error:?}"
    );
}

pub async fn a_loaded_passage_still_knows_its_project(store: &impl PassageStore) {
    let id = an_id(1_000);
    store
        .create(&a_passage(id, "The loom stood silent."))
        .await
        .expect("create should succeed");

    let found = store.load(id).await.expect("load should find the passage");

    assert_eq!(found.project(), &a_project());
}

pub async fn a_project_lists_its_own_passages_in_order(store: &impl PassageStore) {
    let first = an_id(1_000);
    let second = an_id(2_000);
    for id in [second, first] {
        store
            .create(&Passage::empty(id, a_project()))
            .await
            .expect("create should succeed");
    }

    let found = store
        .in_project(&a_project(), None, 10)
        .await
        .expect("listing should succeed");

    assert_eq!(found, vec![first, second]);
}

pub async fn another_project_sees_none_of_them(store: &impl PassageStore) {
    let mine = an_id(1_000);
    store
        .create(&Passage::empty(mine, a_project()))
        .await
        .expect("create should succeed");

    let found = store
        .in_project(&ProjectLink::from("project_2"), None, 10)
        .await
        .expect("listing should succeed");

    assert!(
        found.is_empty(),
        "a sweep must never reach into another author's project, got {found:?}"
    );
}

pub async fn listing_resumes_after_the_cursor(store: &impl PassageStore) {
    let first = an_id(1_000);
    let second = an_id(2_000);
    let third = an_id(3_000);
    for id in [first, second, third] {
        store
            .create(&Passage::empty(id, a_project()))
            .await
            .expect("create should succeed");
    }

    let batch = store
        .in_project(&a_project(), None, 2)
        .await
        .expect("first batch should succeed");
    let rest = store
        .in_project(&a_project(), batch.last().copied(), 2)
        .await
        .expect("second batch should succeed");

    assert_eq!(batch, vec![first, second]);
    assert_eq!(
        rest,
        vec![third],
        "without an exclusive cursor a sweep hands itself the same batch forever"
    );
}

pub async fn listing_past_the_last_passage_is_empty(store: &impl PassageStore) {
    let only = an_id(1_000);
    store
        .create(&Passage::empty(only, a_project()))
        .await
        .expect("create should succeed");

    let found = store
        .in_project(&a_project(), Some(only), 10)
        .await
        .expect("listing should succeed");

    assert!(found.is_empty(), "expected nothing after the last id");
}

pub async fn a_deleted_passage_leaves_the_listing(store: &impl PassageStore) {
    let id = an_id(1_000);
    store
        .create(&Passage::empty(id, a_project()))
        .await
        .expect("create should succeed");

    store.delete(id).await.expect("delete should succeed");

    let found = store
        .in_project(&a_project(), None, 10)
        .await
        .expect("listing should succeed");
    assert!(found.is_empty());
}

fn a_title(saying: &str) -> PassageTitle {
    PassageTitle::new(saying).expect("a plain title is fine")
}

pub async fn a_new_passage_starts_untitled(store: &impl PassageStore) {
    let id = an_id(1_000);
    store
        .create(&Passage::empty(id, a_project()))
        .await
        .expect("create should succeed");

    let found = store.load(id).await.expect("load should find the passage");

    assert!(found.title().is_untitled());
}

pub async fn a_retitled_passage_keeps_its_title(store: &impl PassageStore) {
    let id = an_id(1_000);
    store
        .create(&Passage::empty(id, a_project()))
        .await
        .expect("create should succeed");

    store
        .retitle(id, &a_title("The loom"))
        .await
        .expect("retitle should succeed");

    let found = store.load(id).await.expect("load should find the passage");
    assert_eq!(found.title(), &a_title("The loom"));
}

pub async fn retitling_a_missing_passage_is_not_found(store: &impl PassageStore) {
    let missing = an_id(1_000);

    let error = store
        .retitle(missing, &a_title("Nowhere"))
        .await
        .expect_err("retitle should not find the passage");

    assert!(
        matches!(&error, StoreError::NotFound(id) if *id == missing),
        "expected NotFound({missing}), got {error:?}"
    );
}

pub async fn retitling_leaves_the_text_as_it_was(store: &impl PassageStore) {
    let id = an_id(1_000);
    store
        .create(&a_passage(id, "The loom stood silent."))
        .await
        .expect("create should succeed");

    store
        .retitle(id, &a_title("The loom"))
        .await
        .expect("retitle should succeed");

    let found = store.load(id).await.expect("load should find the passage");
    assert_eq!(found.text(), "The loom stood silent.");
}

pub async fn a_title_survives_further_writing(store: &impl PassageStore) {
    let id = an_id(1_000);
    store
        .create(&Passage::empty(id, a_project()))
        .await
        .expect("create should succeed");
    store
        .retitle(id, &a_title("The loom"))
        .await
        .expect("retitle should succeed");

    store
        .apply(id, &a_paragraph("The loom stood silent."))
        .await
        .expect("apply should succeed");

    let found = store.load(id).await.expect("load should find the passage");
    assert_eq!(
        found.title(),
        &a_title("The loom"),
        "writing rewrites the stored passage, and the title must ride along rather than \
         reset to untitled with every keystroke"
    );
}

macro_rules! conformance_case {
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

macro_rules! conformance_tests {
    ($workbench:ty) => {
        $crate::suite::conformance_case!($workbench, create_then_load_returns_the_prose);
        $crate::suite::conformance_case!($workbench, an_empty_passage_can_be_stored);
        $crate::suite::conformance_case!($workbench, load_missing_passage_is_not_found);
        $crate::suite::conformance_case!($workbench, create_rejects_a_duplicate_id);
        $crate::suite::conformance_case!(
            $workbench,
            a_rejected_create_leaves_the_stored_passage_untouched
        );
        $crate::suite::conformance_case!($workbench, absorb_missing_passage_is_not_found);
        $crate::suite::conformance_case!($workbench, absorbed_updates_are_visible_on_load);
        $crate::suite::conformance_case!(
            $workbench,
            absorbing_the_same_update_twice_changes_nothing
        );
        $crate::suite::conformance_case!($workbench, updates_absorbed_in_either_order_converge);
        $crate::suite::conformance_case!(
            $workbench,
            an_unusable_update_is_refused_and_changes_nothing
        );
        $crate::suite::conformance_case!($workbench, delete_removes_the_passage);
        $crate::suite::conformance_case!($workbench, delete_missing_passage_is_not_found);
        $crate::suite::conformance_case!($workbench, a_loaded_passage_still_knows_its_project);
        $crate::suite::conformance_case!($workbench, a_project_lists_its_own_passages_in_order);
        $crate::suite::conformance_case!($workbench, another_project_sees_none_of_them);
        $crate::suite::conformance_case!($workbench, listing_resumes_after_the_cursor);
        $crate::suite::conformance_case!($workbench, listing_past_the_last_passage_is_empty);
        $crate::suite::conformance_case!($workbench, a_deleted_passage_leaves_the_listing);
        $crate::suite::conformance_case!($workbench, a_new_passage_starts_untitled);
        $crate::suite::conformance_case!($workbench, a_retitled_passage_keeps_its_title);
        $crate::suite::conformance_case!($workbench, retitling_a_missing_passage_is_not_found);
        $crate::suite::conformance_case!($workbench, retitling_leaves_the_text_as_it_was);
        $crate::suite::conformance_case!($workbench, a_title_survives_further_writing);
    };
}

pub(crate) use {conformance_case, conformance_tests};
