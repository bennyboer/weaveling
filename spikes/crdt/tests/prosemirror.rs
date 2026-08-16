use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use weaveling_spike_crdt::{absorb, doc_for, everything};
use yrs::types::xml::XmlOut;
use yrs::{GetString, ReadTxn, Transact, XmlFragment};

/// ProseMirror separates *textblocks* (leaf blocks holding inline content), not
/// every block node. A blockquote wrapping a list wrapping paragraphs yields one
/// separator per paragraph, not one per level.
const TEXTBLOCKS: &[&str] = &["paragraph", "heading", "code_block"];

fn interop_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("interop")
}

fn ready() -> bool {
    interop_dir().join("node_modules/y-prosemirror").exists()
}

fn exchange_dir(scenario: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("weaveling-crdt-pm-{scenario}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("exchange directory should be creatable");

    dir
}

fn run_yjs(exchange: &Path, phase: &str) {
    let outcome = Command::new("node")
        .arg(interop_dir().join("prosemirror.mjs"))
        .arg(exchange)
        .arg(phase)
        .output()
        .expect("node should be runnable");

    assert!(
        outcome.status.success(),
        "the Yjs side failed during `{phase}`:\n{}",
        String::from_utf8_lossy(&outcome.stderr)
    );
}

fn text_at(exchange: &Path, name: &str) -> String {
    fs::read_to_string(exchange.join(name)).expect("Yjs should have written this file")
}

fn bytes_at(exchange: &Path, name: &str) -> Vec<u8> {
    fs::read(exchange.join(name)).expect("Yjs should have written this file")
}

/// Collects the text of a single textblock. Inline nodes carrying no prose, such
/// as an image reference, contribute nothing.
fn inline_text<T: ReadTxn>(nodes: impl Iterator<Item = XmlOut>, txn: &T, out: &mut String) {
    for node in nodes {
        match node {
            XmlOut::Text(text) => out.push_str(&text.get_string(txn)),
            XmlOut::Element(element) => inline_text(element.children(txn), txn, out),
            XmlOut::Fragment(fragment) => inline_text(fragment.children(txn), txn, out),
        }
    }
}

/// Walks whatever structure ProseMirror produced and gathers one entry per
/// textblock. Deliberately generic about containers, so deeper nesting such as
/// tables flows through the same path. An empty textblock still counts, which is
/// what keeps this in step with ProseMirror's own `textBetween`.
fn textblocks<T: ReadTxn>(nodes: impl Iterator<Item = XmlOut>, txn: &T, out: &mut Vec<String>) {
    for node in nodes {
        match node {
            XmlOut::Element(element) if TEXTBLOCKS.contains(&element.tag().as_ref()) => {
                let mut text = String::new();
                inline_text(element.children(txn), txn, &mut text);
                out.push(text);
            }
            XmlOut::Element(element) => textblocks(element.children(txn), txn, out),
            XmlOut::Fragment(fragment) => textblocks(fragment.children(txn), txn, out),
            XmlOut::Text(text) => out.push(text.get_string(txn)),
        }
    }
}

fn plain_text(doc: &yrs::Doc) -> String {
    let fragment = doc.get_or_insert_xml_fragment("prose");
    let txn = doc.transact();
    let mut blocks = Vec::new();
    textblocks(fragment.children(&txn), &txn, &mut blocks);

    blocks.join("\n")
}

fn outline(doc: &yrs::Doc) -> Vec<String> {
    let fragment = doc.get_or_insert_xml_fragment("prose");
    let txn = doc.transact();

    fragment
        .children(&txn)
        .filter_map(|node| match node {
            XmlOut::Element(element) => Some(element.tag().to_string()),
            _ => None,
        })
        .collect()
}

#[test]
fn yrs_can_read_a_prosemirror_document() {
    if !ready() {
        eprintln!("skipping: run `npm install` in spikes/crdt/interop first");
        return;
    }

    let exchange = exchange_dir("read");
    run_yjs(&exchange, "emit");

    let doc = doc_for(1);
    absorb(&doc, &bytes_at(&exchange, "pm-doc.bin"));

    // the server can see the shape ProseMirror authored
    assert_eq!(
        outline(&doc),
        vec!["heading", "paragraph", "blockquote", "paragraph", "paragraph"],
        "yrs should see the block structure y-prosemirror wrote"
    );

    // and can flatten it exactly the way ProseMirror does, for search,
    // the in-order walk and export
    let extracted = plain_text(&doc);
    assert_eq!(
        extracted,
        text_at(&exchange, "pm-expected-text.txt"),
        "yrs extracted different prose than ProseMirror reports"
    );
    assert!(
        extracted.contains("warp threads"),
        "nested lists must survive"
    );
    assert!(
        !extracted.contains("blob://"),
        "an image reference is not prose"
    );

    println!("\n--- yrs reading a y-prosemirror document ---");
    for line in extracted.lines() {
        println!("  |{line}");
    }
}

#[test]
fn a_rust_edit_survives_the_trip_back_into_prosemirror() {
    if !ready() {
        eprintln!("skipping: run `npm install` in spikes/crdt/interop first");
        return;
    }

    let exchange = exchange_dir("roundtrip");
    run_yjs(&exchange, "emit");

    let doc = doc_for(1);
    absorb(&doc, &bytes_at(&exchange, "pm-doc.bin"));

    // edit the prose from Rust, inside the structure ProseMirror created
    {
        let fragment = doc.get_or_insert_xml_fragment("prose");
        let txn = doc.transact();
        let paragraph = match fragment.children(&txn).nth(1) {
            Some(XmlOut::Element(element)) => element,
            other => panic!("expected a paragraph element, found {other:?}"),
        };
        let prose = match paragraph.children(&txn).next() {
            Some(XmlOut::Text(text)) => text,
            other => panic!("expected a text node, found {other:?}"),
        };
        drop(txn);

        let mut txn = doc.transact_mut();
        yrs::Text::insert(&prose, &mut txn, 0, "Even now, ");
    }

    fs::write(exchange.join("pm-after-rust.bin"), everything(&doc))
        .expect("update should be writable");
    run_yjs(&exchange, "verify");

    assert_eq!(
        text_at(&exchange, "pm-reparsed-kinds.txt"),
        "heading,paragraph,blockquote,paragraph,paragraph",
        "the document must still validate against the ProseMirror schema"
    );
    assert!(
        text_at(&exchange, "pm-reparsed-text.txt").contains("Even now, The loom stood silent"),
        "the Rust edit should be visible to ProseMirror"
    );
}

#[test]
fn heavy_editing_of_real_prose_stays_proportionate() {
    if !ready() {
        eprintln!("skipping: run `npm install` in spikes/crdt/interop first");
        return;
    }

    let exchange = exchange_dir("churn");
    run_yjs(&exchange, "emit");

    let snapshot = bytes_at(&exchange, "pm-churn-snapshot.bin");
    let compacted = bytes_at(&exchange, "pm-churn-compacted.bin");
    let log_bytes: usize = text_at(&exchange, "pm-churn-log-bytes.txt")
        .trim()
        .parse()
        .expect("log size should be a number");

    let churned = doc_for(2);
    absorb(&churned, &snapshot);
    let text_bytes = plain_text(&churned).len();

    println!("\n--- 500 rewrites inside a ProseMirror document (XmlFragment) ---");
    println!("final text           {text_bytes:>8} bytes");
    println!("append-only log      {log_bytes:>8} bytes");
    println!("compacted log        {:>8} bytes", compacted.len());
    println!("snapshot             {:>8} bytes", snapshot.len());

    assert!(
        compacted.len() < log_bytes,
        "compaction should shrink the append-only log"
    );

    let rebuilt = doc_for(3);
    absorb(&rebuilt, &compacted);
    assert_eq!(
        plain_text(&rebuilt),
        plain_text(&churned),
        "a compacted log must rebuild the same prose"
    );
}
