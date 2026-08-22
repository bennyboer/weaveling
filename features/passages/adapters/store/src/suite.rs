use passages_core::{FRAGMENT, Passage, PassageId, PassageStore, StoreError};
use time::{Duration, OffsetDateTime};
use yrs::{Doc, ReadTxn, StateVector, Transact, XmlElementPrelim, XmlFragment, XmlTextPrelim};

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

pub fn a_passage(id: PassageId, saying: &str) -> Passage {
    let passage = Passage::empty(id);
    passage
        .absorb(&a_paragraph(saying))
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
        .create(&Passage::empty(id))
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
        .absorb(missing, &a_paragraph("into the void"))
        .await
        .expect_err("absorb should not find the passage");

    assert!(
        matches!(&error, StoreError::NotFound(id) if *id == missing),
        "expected NotFound({missing}), got {error:?}"
    );
}

pub async fn absorbed_updates_are_visible_on_load(store: &impl PassageStore) {
    let id = an_id(1_000);
    store
        .create(&Passage::empty(id))
        .await
        .expect("create should succeed");

    store
        .absorb(id, &a_paragraph("The loom stood silent."))
        .await
        .expect("absorb should succeed");

    let found = store.load(id).await.expect("load should find the passage");
    assert_eq!(found.text(), "The loom stood silent.");
}

pub async fn absorbing_the_same_update_twice_changes_nothing(store: &impl PassageStore) {
    let id = an_id(1_000);
    store
        .create(&Passage::empty(id))
        .await
        .expect("create should succeed");
    let update = a_paragraph("The loom stood silent.");

    store.absorb(id, &update).await.expect("first absorb");
    store.absorb(id, &update).await.expect("second absorb");

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
            .create(&Passage::empty(id))
            .await
            .expect("create should succeed");
    }

    store.absorb(one, &ada).await.expect("absorb ada");
    store.absorb(one, &bo).await.expect("absorb bo");
    store.absorb(other, &bo).await.expect("absorb bo");
    store.absorb(other, &ada).await.expect("absorb ada");

    let first = store.load(one).await.expect("load one").text();
    let second = store.load(other).await.expect("load other").text();
    assert_eq!(first, second, "absorb order must not matter");
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
        .absorb(id, &[255, 255, 255, 255])
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

macro_rules! conformance_case {
    ($make_store:expr, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            $crate::suite::$case(&$make_store).await;
        }
    };
}

macro_rules! conformance_tests {
    ($make_store:expr) => {
        $crate::suite::conformance_case!($make_store, create_then_load_returns_the_prose);
        $crate::suite::conformance_case!($make_store, an_empty_passage_can_be_stored);
        $crate::suite::conformance_case!($make_store, load_missing_passage_is_not_found);
        $crate::suite::conformance_case!($make_store, create_rejects_a_duplicate_id);
        $crate::suite::conformance_case!(
            $make_store,
            a_rejected_create_leaves_the_stored_passage_untouched
        );
        $crate::suite::conformance_case!($make_store, absorb_missing_passage_is_not_found);
        $crate::suite::conformance_case!($make_store, absorbed_updates_are_visible_on_load);
        $crate::suite::conformance_case!(
            $make_store,
            absorbing_the_same_update_twice_changes_nothing
        );
        $crate::suite::conformance_case!($make_store, updates_absorbed_in_either_order_converge);
        $crate::suite::conformance_case!(
            $make_store,
            an_unusable_update_is_refused_and_changes_nothing
        );
        $crate::suite::conformance_case!($make_store, delete_removes_the_passage);
        $crate::suite::conformance_case!($make_store, delete_missing_passage_is_not_found);
    };
}

pub(crate) use {conformance_case, conformance_tests};
