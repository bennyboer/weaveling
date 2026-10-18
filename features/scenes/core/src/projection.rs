use quotation::QuotationMarks;
use yrs::types::Attrs;
use yrs::types::text::YChange;
use yrs::types::xml::{XmlOut, XmlTextRef};
use yrs::{Any, Doc, OffsetKind, Out, ReadTxn, Text, Transact, XmlFragment};

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

const QUOTE: &str = "quote";

struct QuoteRun {
    at: u32,
    length: u32,
    side: String,
    written: String,
    attrs: Attrs,
}

fn texts_in<T: ReadTxn>(nodes: impl Iterator<Item = XmlOut>, txn: &T, out: &mut Vec<XmlTextRef>) {
    for node in nodes {
        match node {
            XmlOut::Text(text) => out.push(text),
            XmlOut::Element(element) => texts_in(element.children(txn), txn, out),
            XmlOut::Fragment(fragment) => texts_in(fragment.children(txn), txn, out),
        }
    }
}

fn side_of(attrs: &Attrs) -> Option<String> {
    match attrs.get(QUOTE)? {
        Any::Map(held) => match held.get("side") {
            Some(Any::String(side)) => Some(side.to_string()),
            _ => Some("opening".to_owned()),
        },
        _ => None,
    }
}

fn quote_runs<T: ReadTxn>(text: &XmlTextRef, txn: &T, measured: OffsetKind) -> Vec<QuoteRun> {
    let mut runs: Vec<QuoteRun> = Vec::new();
    let mut at = 0;
    let mut open = false;

    for chunk in text.diff(txn, YChange::identity) {
        let Out::Any(Any::String(written)) = chunk.insert else {
            open = false;
            continue;
        };
        let length = match measured {
            OffsetKind::Bytes => written.len() as u32,
            OffsetKind::Utf16 => written.encode_utf16().count() as u32,
        };
        let attrs = chunk.attributes.map(|attrs| *attrs).unwrap_or_default();

        match side_of(&attrs) {
            Some(side) if open && runs.last().is_some_and(|run| run.side == side) => {
                if let Some(run) = runs.last_mut() {
                    run.length += length;
                    run.written.push_str(&written);
                }
            }
            Some(side) => {
                runs.push(QuoteRun {
                    at,
                    length,
                    side,
                    written: written.to_string(),
                    attrs,
                });
                open = true;
            }
            None => open = false,
        }
        at += length;
    }

    runs
}

pub(crate) fn restyle(doc: &Doc, style: QuotationMarks) -> bool {
    let fragment = doc.get_or_insert_xml_fragment(FRAGMENT);
    let measured = doc.offset_kind();
    let mut txn = doc.transact_mut();
    let mut texts = Vec::new();
    texts_in(fragment.children(&txn), &txn, &mut texts);
    let mut changed = false;

    for text in texts {
        for run in quote_runs(&text, &txn, measured).into_iter().rev() {
            let wanted = match run.side.as_str() {
                "closing" => style.closing(),
                _ => style.opening(),
            };
            if run.written != wanted {
                text.remove_range(&mut txn, run.at, run.length);
                text.insert_with_attributes(&mut txn, run.at, wanted, run.attrs);
                changed = true;
            }
        }
    }

    changed
}
