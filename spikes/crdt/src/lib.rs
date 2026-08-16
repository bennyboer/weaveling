pub mod projection;

use yrs::updates::decoder::Decode;
use yrs::{Doc, GetString, Options, ReadTxn, StateVector, Text, TextRef, Transact, Update};

pub const PROSE: &str = "prose";

pub fn doc_for(client_id: u64) -> Doc {
    Doc::with_options(Options {
        client_id: yrs::ClientID::new(client_id),
        ..Options::default()
    })
}

pub fn doc_without_gc(client_id: u64) -> Doc {
    Doc::with_options(Options {
        client_id: yrs::ClientID::new(client_id),
        skip_gc: true,
        ..Options::default()
    })
}

pub fn prose(doc: &Doc) -> TextRef {
    doc.get_or_insert_text(PROSE)
}

pub fn read(doc: &Doc) -> String {
    let text = prose(doc);
    let txn = doc.transact();

    text.get_string(&txn)
}

pub fn insert(doc: &Doc, index: u32, chunk: &str) {
    let text = prose(doc);
    let mut txn = doc.transact_mut();

    text.insert(&mut txn, index, chunk);
}

pub fn remove(doc: &Doc, index: u32, len: u32) {
    let text = prose(doc);
    let mut txn = doc.transact_mut();

    text.remove_range(&mut txn, index, len);
}

pub fn everything(doc: &Doc) -> Vec<u8> {
    doc.transact()
        .encode_state_as_update_v1(&StateVector::default())
}

pub fn whats_missing(doc: &Doc, theirs: &StateVector) -> Vec<u8> {
    doc.transact().encode_state_as_update_v1(theirs)
}

pub fn state_vector(doc: &Doc) -> StateVector {
    doc.transact().state_vector()
}

pub fn absorb(doc: &Doc, update: &[u8]) {
    let mut txn = doc.transact_mut();

    txn.apply_update(Update::decode_v1(update).expect("update should decode"))
        .expect("update should apply");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_update_carries_an_edit_to_another_replica() {
        let alice = doc_for(1);
        let bob = doc_for(2);
        insert(&alice, 0, "abc");

        absorb(&bob, &everything(&alice));

        assert_eq!(read(&bob), "abc");
    }

    #[test]
    fn inserts_land_by_identity_not_by_index() {
        let alice = doc_for(1);
        let bob = doc_for(2);
        insert(&alice, 0, "abc");
        absorb(&bob, &everything(&alice));

        insert(&alice, 1, "X");
        insert(&bob, 2, "Y");
        absorb(&bob, &everything(&alice));
        absorb(&alice, &everything(&bob));

        assert_eq!(read(&alice), "aXbYc");
        assert_eq!(read(&bob), "aXbYc");
    }

    #[test]
    fn concurrent_inserts_in_the_same_gap_converge() {
        let alice = doc_for(1);
        let bob = doc_for(2);
        insert(&alice, 0, "ac");
        absorb(&bob, &everything(&alice));

        insert(&alice, 1, "b");
        insert(&bob, 1, "B");
        absorb(&bob, &everything(&alice));
        absorb(&alice, &everything(&bob));

        assert_eq!(
            read(&alice),
            read(&bob),
            "replicas must agree on the interleaving"
        );
        assert_eq!(read(&alice).len(), 4);
    }

    #[test]
    fn updates_can_arrive_in_any_order_and_more_than_once() {
        let alice = doc_for(1);
        insert(&alice, 0, "one ");
        let first = everything(&alice);
        insert(&alice, 4, "two ");
        let both = everything(&alice);

        let shuffled = doc_for(2);
        absorb(&shuffled, &both);
        absorb(&shuffled, &first);
        absorb(&shuffled, &first);

        assert_eq!(read(&shuffled), read(&alice));
    }

    #[test]
    fn a_state_vector_asks_only_for_what_is_missing() {
        let alice = doc_for(1);
        insert(&alice, 0, &"a settled paragraph. ".repeat(200));
        let bob = doc_for(2);
        absorb(&bob, &everything(&alice));

        insert(&alice, 0, "one late word ");
        let catch_up = whats_missing(&alice, &state_vector(&bob));

        let whole = everything(&alice).len();
        assert!(
            catch_up.len() * 20 < whole,
            "catching up cost {} bytes against a {} byte document",
            catch_up.len(),
            whole
        );
    }

    fn hammer(doc: &Doc, rounds: usize) -> Vec<Vec<u8>> {
        let sentence = "The loom stood silent in the grey morning light. ";
        let mut log = Vec::new();
        let mut seen = state_vector(doc);

        insert(doc, 0, &sentence.repeat(10));
        log.push(whats_missing(doc, &seen));
        seen = state_vector(doc);

        for round in 0..rounds {
            let length = read(doc).chars().count() as u32;
            let at = (round as u32 * 37) % length.max(1);

            remove(doc, at.min(length.saturating_sub(8)), 8.min(length));
            insert(doc, at.min(read(doc).chars().count() as u32), "rewoven ");

            log.push(whats_missing(doc, &seen));
            seen = state_vector(doc);
        }

        log
    }

    #[test]
    fn heavy_editing_stays_proportionate() {
        const ROUNDS: usize = 500;

        let collected = doc_for(1);
        let log = hammer(&collected, ROUNDS);

        let text_bytes = read(&collected).len();
        let log_bytes: usize = log.iter().map(Vec::len).sum();
        let compacted = yrs::merge_updates_v1(&log).expect("log should merge");
        let snapshot = everything(&collected);

        let ungathered = doc_without_gc(1);
        hammer(&ungathered, ROUNDS);
        let without_gc = everything(&ungathered);

        println!("\n--- {ROUNDS} rewrites of one paragraph ---");
        println!("final text           {text_bytes:>8} bytes");
        println!("append-only log      {:>8} bytes", log_bytes);
        println!("compacted log        {:>8} bytes", compacted.len());
        println!("snapshot (gc on)     {:>8} bytes", snapshot.len());
        println!("snapshot (gc off)    {:>8} bytes", without_gc.len());
        println!(
            "snapshot / text      {:>8.1}x",
            snapshot.len() as f64 / text_bytes as f64
        );

        assert!(
            compacted.len() < log_bytes,
            "compaction should shrink the append-only log"
        );
        assert!(
            snapshot.len() <= without_gc.len(),
            "gc should not make the document larger"
        );

        let rebuilt = doc_for(2);
        absorb(&rebuilt, &compacted);
        assert_eq!(
            read(&rebuilt),
            read(&collected),
            "a compacted log must rebuild the same text"
        );
    }
}
