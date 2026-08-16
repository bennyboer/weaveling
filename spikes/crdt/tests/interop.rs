use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use weaveling_spike_crdt::{
    absorb, doc_for, everything, insert, read, state_vector, whats_missing,
};
use yrs::StateVector;
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;

const YJS: &str = "13.6.32";
const YRS: &str = "0.27.3";

fn interop_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("interop")
}

fn yjs_is_installed() -> bool {
    interop_dir().join("node_modules/yjs").exists()
}

fn exchange_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("weaveling-crdt-interop");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("exchange directory should be creatable");

    dir
}

fn run_yjs(exchange: &Path) {
    let outcome = Command::new("node")
        .arg(interop_dir().join("roundtrip.mjs"))
        .arg(exchange)
        .output()
        .expect("node should be runnable");

    assert!(
        outcome.status.success(),
        "the Yjs side failed:\n{}",
        String::from_utf8_lossy(&outcome.stderr)
    );
}

fn text_at(exchange: &Path, name: &str) -> String {
    fs::read_to_string(exchange.join(name)).expect("Yjs should have written this file")
}

fn bytes_at(exchange: &Path, name: &str) -> Vec<u8> {
    fs::read(exchange.join(name)).expect("Yjs should have written this file")
}

fn stage(exchange: &Path, prefix: &str, start: &str, edit: impl Fn(&yrs::Doc)) -> yrs::Doc {
    let base = doc_for(1);
    insert(&base, 0, start);
    let base_state = state_vector(&base);

    fs::write(
        exchange.join(format!("{prefix}base.bin")),
        everything(&base),
    )
    .expect("base should be writable");
    fs::write(
        exchange.join(format!("{prefix}base-sv.bin")),
        base_state.encode_v1(),
    )
    .expect("state vector should be writable");

    let rust = doc_for(100);
    absorb(&rust, &everything(&base));
    edit(&rust);
    fs::write(
        exchange.join(format!("{prefix}rust.bin")),
        whats_missing(&rust, &base_state),
    )
    .expect("rust update should be writable");

    rust
}

#[test]
fn yrs_and_yjs_agree_on_the_same_document() {
    if !yjs_is_installed() {
        eprintln!("skipping: run `npm install` in spikes/crdt/interop first");
        return;
    }

    let exchange = exchange_dir();

    let apart = stage(&exchange, "apart-", "The loom stood silent. ", |doc| {
        let end = read(doc).chars().count() as u32;
        insert(doc, end, "Rust wrote last.");
    });
    let tie = stage(&exchange, "tie-", "ac", |doc| insert(doc, 1, "b"));

    run_yjs(&exchange);

    assert_eq!(
        text_at(&exchange, "apart-js-saw-base.txt"),
        "The loom stood silent. ",
        "Yjs should read what yrs wrote"
    );

    absorb(&apart, &bytes_at(&exchange, "apart-js.bin"));
    let apart_rust = read(&apart);
    let apart_js = text_at(&exchange, "apart-js-final.txt");
    assert_eq!(
        apart_rust, apart_js,
        "the two implementations merged distant edits differently"
    );
    assert!(apart_rust.contains("JS wrote first."));
    assert!(apart_rust.contains("Rust wrote last."));

    absorb(&tie, &bytes_at(&exchange, "tie-js.bin"));
    let tie_rust = read(&tie);
    let tie_js = text_at(&exchange, "tie-js-final.txt");
    assert_eq!(
        tie_rust, tie_js,
        "the two implementations broke an insertion tie differently"
    );
    assert_eq!(tie_rust.len(), 4, "both inserts should survive");

    let js_state = StateVector::decode_v1(&bytes_at(&exchange, "apart-js-final-sv.bin"))
        .expect("a Yjs state vector should decode in yrs");
    assert!(
        whats_missing(&apart, &js_state).len() < everything(&apart).len(),
        "a Yjs state vector should let yrs compute a real diff"
    );

    println!("\n--- yrs {YRS} <-> Yjs {YJS} ---");
    println!("distant edits    {apart_rust}");
    println!("same-gap tie     {tie_rust}");
}
