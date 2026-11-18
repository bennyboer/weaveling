use std::collections::HashMap;
use std::sync::Arc;

use yrs::types::Attrs;
use yrs::types::xml::XmlElementRef;
use yrs::{
    Any, Doc, Text, Transact, TransactionMut, Xml, XmlElementPrelim, XmlFragment, XmlTextPrelim,
};

use crate::projection::FRAGMENT;
use crate::readable::{self, Block, FigureSize, Marks, Run};

fn marked(marks: &[&str]) -> Attrs {
    marks
        .iter()
        .map(|mark| (Arc::from(*mark), Any::Map(Default::default())))
        .collect()
}

fn linked(href: &str) -> Attrs {
    Attrs::from([(
        Arc::from("link"),
        Any::from(HashMap::from([("href".to_owned(), Any::from(href))])),
    )])
}

fn quoting(side: &str) -> Attrs {
    Attrs::from([(
        Arc::from("quote"),
        Any::from(HashMap::from([("side".to_owned(), Any::from(side))])),
    )])
}

/// Writes a run of marked pieces into an element, as the editor keeps a textblock's text.
fn write_into(txn: &mut TransactionMut, element: &XmlElementRef, pieces: Vec<(&str, Attrs)>) {
    let at = element.len(txn);
    let text = element.insert(txn, at, XmlTextPrelim::new(""));
    for (piece, attrs) in pieces {
        let end = text.len(txn);
        text.insert_with_attributes(txn, end, piece, attrs);
    }
}

fn plain(text: &str) -> Run {
    Run::Text {
        text: text.to_owned(),
        marks: Marks::default(),
    }
}

fn with(text: &str, marks: Marks) -> Run {
    Run::Text {
        text: text.to_owned(),
        marks,
    }
}

#[test]
fn an_empty_scene_reads_as_nothing() {
    // given: a scene that was never written in
    let doc = Doc::new();

    // when: it is read
    let blocks = readable::blocks_of(&doc);

    // then: there is nothing to read
    assert_eq!(blocks, []);
}

#[test]
fn a_paragraph_keeps_the_marks_a_reader_sees() {
    // given: a paragraph with emphasis, strong, code, a link and both together
    let doc = Doc::new();
    {
        let fragment = doc.get_or_insert_xml_fragment(FRAGMENT);
        let mut txn = doc.transact_mut();
        let paragraph = fragment.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
        write_into(
            &mut txn,
            &paragraph,
            vec![
                ("The ", Attrs::new()),
                ("loom", marked(&["em"])),
                (" stood ", Attrs::new()),
                ("silent", marked(&["strong"])),
                (" by ", Attrs::new()),
                ("weft()", marked(&["code"])),
                (", see ", Attrs::new()),
                ("the map", linked("https://example.org/map")),
                (" and ", Attrs::new()),
                ("this", marked(&["em", "strong"])),
            ],
        );
    }

    // when: it is read
    let blocks = readable::blocks_of(&doc);

    // then: each run keeps its marks
    assert_eq!(
        blocks,
        [Block::Paragraph(vec![
            plain("The "),
            with(
                "loom",
                Marks {
                    emphasis: true,
                    ..Marks::default()
                }
            ),
            plain(" stood "),
            with(
                "silent",
                Marks {
                    strong: true,
                    ..Marks::default()
                }
            ),
            plain(" by "),
            with(
                "weft()",
                Marks {
                    code: true,
                    ..Marks::default()
                }
            ),
            plain(", see "),
            with(
                "the map",
                Marks {
                    link: Some("https://example.org/map".to_owned()),
                    ..Marks::default()
                }
            ),
            plain(" and "),
            with(
                "this",
                Marks {
                    emphasis: true,
                    strong: true,
                    ..Marks::default()
                }
            ),
        ])]
    );
}

#[test]
fn quotation_marks_and_speech_are_left_out_and_what_they_split_is_joined() {
    // given: a paragraph of speech between quotation marks
    let doc = Doc::new();
    {
        let fragment = doc.get_or_insert_xml_fragment(FRAGMENT);
        let mut txn = doc.transact_mut();
        let paragraph = fragment.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
        write_into(
            &mut txn,
            &paragraph,
            vec![
                ("„", quoting("opening")),
                ("What am I to do?", marked(&["speech"])),
                ("“", quoting("closing")),
                (" she asked.", Attrs::new()),
            ],
        );
    }

    // when: it is read
    let blocks = readable::blocks_of(&doc);

    // then: it is one plain run, the quotation marks kept as the characters they are
    assert_eq!(
        blocks,
        [Block::Paragraph(vec![plain(
            "„What am I to do?“ she asked."
        )])]
    );
}

#[test]
fn a_hard_break_stays_a_break_within_its_paragraph() {
    // given: a paragraph of two lines
    let doc = Doc::new();
    {
        let fragment = doc.get_or_insert_xml_fragment(FRAGMENT);
        let mut txn = doc.transact_mut();
        let paragraph = fragment.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
        write_into(
            &mut txn,
            &paragraph,
            vec![("Nobody believed the sign.", Attrs::new())],
        );
        paragraph.insert(&mut txn, 1, XmlElementPrelim::empty("hard_break"));
        write_into(
            &mut txn,
            &paragraph,
            vec![("Everybody bought the fish.", Attrs::new())],
        );
    }

    // when: it is read
    let blocks = readable::blocks_of(&doc);

    // then: the lines are apart by a break
    assert_eq!(
        blocks,
        [Block::Paragraph(vec![
            plain("Nobody believed the sign."),
            Run::Break,
            plain("Everybody bought the fish."),
        ])]
    );
}

#[test]
fn headings_quotes_lists_code_and_rules_read_as_their_blocks() {
    // given: a scene of a heading, a quote, a list of each kind, a code block and a rule
    let doc = Doc::new();
    {
        let fragment = doc.get_or_insert_xml_fragment(FRAGMENT);
        let mut txn = doc.transact_mut();
        let heading = fragment.insert(&mut txn, 0, XmlElementPrelim::empty("heading"));
        heading.insert_attribute(&mut txn, "level", Any::from(2.0));
        write_into(&mut txn, &heading, vec![("At the harbour", Attrs::new())]);

        let quote = fragment.insert(&mut txn, 1, XmlElementPrelim::empty("blockquote"));
        let quoted = quote.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
        write_into(&mut txn, &quoted, vec![("Fresh today.", Attrs::new())]);

        for (at, kind, items) in [
            (2, "bullet_list", ["nets", "lamps"]),
            (3, "ordered_list", ["first", "second"]),
        ] {
            let list = fragment.insert(&mut txn, at, XmlElementPrelim::empty(kind));
            for (nth, item) in items.iter().enumerate() {
                let entry = list.insert(&mut txn, nth as u32, XmlElementPrelim::empty("list_item"));
                let paragraph = entry.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
                write_into(&mut txn, &paragraph, vec![(*item, Attrs::new())]);
            }
        }

        let code = fragment.insert(&mut txn, 4, XmlElementPrelim::empty("code_block"));
        write_into(&mut txn, &code, vec![("let tide = high;", Attrs::new())]);

        fragment.insert(&mut txn, 5, XmlElementPrelim::empty("horizontal_rule"));
    }

    // when: it is read
    let blocks = readable::blocks_of(&doc);

    // then: each reads as its block
    assert_eq!(
        blocks,
        [
            Block::Heading {
                level: 2,
                runs: vec![plain("At the harbour")],
            },
            Block::Quote(vec![Block::Paragraph(vec![plain("Fresh today.")])]),
            Block::List {
                ordered: false,
                items: vec![
                    vec![Block::Paragraph(vec![plain("nets")])],
                    vec![Block::Paragraph(vec![plain("lamps")])],
                ],
            },
            Block::List {
                ordered: true,
                items: vec![
                    vec![Block::Paragraph(vec![plain("first")])],
                    vec![Block::Paragraph(vec![plain("second")])],
                ],
            },
            Block::Code("let tide = high;".to_owned()),
            Block::Rule,
        ]
    );
}

#[test]
fn a_heading_whose_level_is_out_of_reach_is_kept_within_it() {
    // given: a heading without a level and one too deep
    let doc = Doc::new();
    {
        let fragment = doc.get_or_insert_xml_fragment(FRAGMENT);
        let mut txn = doc.transact_mut();
        let unleveled = fragment.insert(&mut txn, 0, XmlElementPrelim::empty("heading"));
        write_into(&mut txn, &unleveled, vec![("Unleveled", Attrs::new())]);
        let deep = fragment.insert(&mut txn, 1, XmlElementPrelim::empty("heading"));
        deep.insert_attribute(&mut txn, "level", Any::from(9.0));
        write_into(&mut txn, &deep, vec![("Deep", Attrs::new())]);
    }

    // when: they are read
    let blocks = readable::blocks_of(&doc);

    // then: the first is at the top level, the other at the lowest there is
    assert_eq!(
        blocks,
        [
            Block::Heading {
                level: 1,
                runs: vec![plain("Unleveled")],
            },
            Block::Heading {
                level: 6,
                runs: vec![plain("Deep")],
            },
        ]
    );
}

#[test]
fn a_figure_reads_with_its_image_size_and_caption_and_one_naming_no_image_is_left_out() {
    // given: a captioned figure, one of a size not known, and one naming no image
    let doc = Doc::new();
    {
        let fragment = doc.get_or_insert_xml_fragment(FRAGMENT);
        let mut txn = doc.transact_mut();
        for (at, asset, size, caption) in [
            (0, Some("asset_frames"), "medium", "The drying frames."),
            (1, Some("asset_harbour"), "huge", ""),
            (2, Some("javascript:alert(1)"), "small", "Not an image."),
            (3, None, "small", "No image at all."),
        ] {
            let figure = fragment.insert(&mut txn, at, XmlElementPrelim::empty("figure"));
            if let Some(asset) = asset {
                figure.insert_attribute(&mut txn, "asset", asset);
            }
            figure.insert_attribute(&mut txn, "size", size);
            write_into(&mut txn, &figure, vec![(caption, Attrs::new())]);
        }
    }

    // when: it is read
    let blocks = readable::blocks_of(&doc);

    // then: the first two are figures, the unknown size shown full
    assert_eq!(
        blocks,
        [
            Block::Figure {
                asset: "asset_frames".to_owned(),
                size: FigureSize::Medium,
                caption: "The drying frames.".to_owned(),
            },
            Block::Figure {
                asset: "asset_harbour".to_owned(),
                size: FigureSize::Full,
                caption: String::new(),
            },
        ]
    );
}

#[test]
fn an_element_not_known_is_read_as_the_blocks_it_holds() {
    // given: a paragraph inside an element the reader does not know
    let doc = Doc::new();
    {
        let fragment = doc.get_or_insert_xml_fragment(FRAGMENT);
        let mut txn = doc.transact_mut();
        let unknown = fragment.insert(&mut txn, 0, XmlElementPrelim::empty("aside"));
        let paragraph = unknown.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
        write_into(
            &mut txn,
            &paragraph,
            vec![("Kept all the same.", Attrs::new())],
        );
    }

    // when: it is read
    let blocks = readable::blocks_of(&doc);

    // then: what it holds is read
    assert_eq!(
        blocks,
        [Block::Paragraph(vec![plain("Kept all the same.")])]
    );
}
