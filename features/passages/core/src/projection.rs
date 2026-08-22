use yrs::types::xml::XmlOut;
use yrs::{Doc, GetString, ReadTxn, Transact, XmlFragment};

pub const FRAGMENT: &str = "prose";

const TEXTBLOCKS: &[&str] = &["paragraph", "heading", "code_block"];

fn inline_text<T: ReadTxn>(nodes: impl Iterator<Item = XmlOut>, txn: &T, out: &mut String) {
    for node in nodes {
        match node {
            XmlOut::Text(text) => out.push_str(&text.get_string(txn)),
            XmlOut::Element(element) => inline_text(element.children(txn), txn, out),
            XmlOut::Fragment(fragment) => inline_text(fragment.children(txn), txn, out),
        }
    }
}

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

pub(crate) fn plain_text(doc: &Doc) -> String {
    let fragment = doc.get_or_insert_xml_fragment(FRAGMENT);
    let txn = doc.transact();
    let mut blocks = Vec::new();
    textblocks(fragment.children(&txn), &txn, &mut blocks);

    blocks.join("\n")
}
