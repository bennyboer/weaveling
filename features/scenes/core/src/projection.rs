use yrs::types::text::YChange;
use yrs::types::xml::{XmlOut, XmlTextRef};
use yrs::{Any, Doc, Out, ReadTxn, Text, Transact, XmlFragment};

pub const FRAGMENT: &str = "prose";

const TEXTBLOCKS: &[&str] = &["paragraph", "heading", "code_block"];

fn unmarked<T: ReadTxn>(text: &XmlTextRef, txn: &T) -> String {
    text.diff(txn, YChange::identity)
        .into_iter()
        .filter_map(|run| match run.insert {
            Out::Any(Any::String(written)) => Some(written),
            _ => None,
        })
        .fold(String::new(), |text, written| text + &written)
}

fn inline_text<T: ReadTxn>(nodes: impl Iterator<Item = XmlOut>, txn: &T, out: &mut String) {
    for node in nodes {
        match node {
            XmlOut::Text(text) => out.push_str(&unmarked(&text, txn)),
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
            XmlOut::Text(text) => out.push(unmarked(&text, txn)),
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
