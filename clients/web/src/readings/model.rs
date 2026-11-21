use std::collections::HashMap;
use std::sync::Arc;

use readings_contract::{
    BlockDTO, BookChangeDTO, ContentsDTO, MarksDTO, OutlinedSectionDTO, PartDTO, RunDTO,
};

use crate::outline::model::SectionId;
use crate::scenes::model::SceneId;

/// A book as a reader sees it, held as the pieces it is told in: its title, its sections in reading order, and each scene's text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Book {
    pub title: String,
    pub sections: Vec<OutlinedSection>,
    texts: HashMap<SceneId, Text>,
}

/// A section, how deep it lies, and the scenes it holds in their order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutlinedSection {
    pub section: SectionId,
    pub depth: u8,
    pub title: String,
    pub scenes: Vec<SceneId>,
}

/// A scene's text as it was told.
/// Two are the same only if they are one telling, so a scene told nothing new is not drawn again.
#[derive(Clone, Debug, Eq)]
pub struct Text(Arc<Vec<Block>>);

impl PartialEq for Text {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Text {
    pub fn blocks(&self) -> &[Block] {
        &self.0
    }

    /// Whether two texts say the same, word for word, whether or not they are one telling.
    pub fn says_as(&self, other: &Text) -> bool {
        self.0 == other.0
    }
}

/// One piece of the book, as it changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    Text { scene: SceneId, blocks: Vec<Block> },
    Outline(Vec<OutlinedSection>),
    Title(String),
}

/// Whether the book, once changed, still holds the text of every scene it names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applied {
    Whole,
    LackingText,
}

impl Book {
    /// Swaps the changed piece in for the one held.
    /// An outline may name a scene the book was never told the text of, as one placed since it was read; then the book is to be read again.
    pub fn apply(&mut self, change: Change) -> Applied {
        match change {
            Change::Text { scene, blocks } => {
                self.texts.insert(scene, Text(Arc::new(blocks)));
            }
            Change::Outline(sections) => self.sections = sections,
            Change::Title(title) => self.title = title,
        }

        if self
            .sections
            .iter()
            .flat_map(|section| &section.scenes)
            .all(|scene| self.texts.contains_key(scene))
        {
            Applied::Whole
        } else {
            Applied::LackingText
        }
    }

    /// The book read again, holding on to each scene's text that says what it said before,
    /// so a scene whose words did not change is not drawn again, nor a selection in it lost.
    pub fn keeping_unchanged(mut self, before: &Book) -> Self {
        for (scene, text) in &mut self.texts {
            if let Some(held) = before.texts.get(scene).filter(|held| held.says_as(text)) {
                *text = held.clone();
            }
        }

        self
    }

    /// A scene's text, if it says anything.
    /// A scene with nothing written is not shown.
    pub fn text_of(&self, scene: &SceneId) -> Option<Text> {
        self.texts
            .get(scene)
            .filter(|text| !text.blocks().is_empty())
            .cloned()
    }
}

impl From<ContentsDTO> for Book {
    fn from(dto: ContentsDTO) -> Self {
        let mut sections: Vec<OutlinedSection> = Vec::new();
        let mut texts = HashMap::new();
        for part in dto.parts {
            match part {
                PartDTO::Section {
                    section,
                    depth,
                    title,
                } => sections.push(OutlinedSection {
                    section: SectionId::from(section),
                    depth,
                    title,
                    scenes: Vec::new(),
                }),
                PartDTO::Scene { scene, blocks } => {
                    let scene = SceneId::from(scene);
                    if let Some(holding) = sections.last_mut() {
                        holding.scenes.push(scene.clone());
                    }
                    texts.insert(scene, Text(Arc::new(blocks_of(blocks))));
                }
            }
        }

        Self {
            title: dto.title,
            sections,
            texts,
        }
    }
}

impl From<BookChangeDTO> for Change {
    fn from(dto: BookChangeDTO) -> Self {
        match dto {
            BookChangeDTO::Text { scene, blocks } => Self::Text {
                scene: SceneId::from(scene),
                blocks: blocks_of(blocks),
            },
            BookChangeDTO::Outline { sections } => {
                Self::Outline(sections.into_iter().map(OutlinedSection::from).collect())
            }
            BookChangeDTO::Title { title } => Self::Title(title),
        }
    }
}

impl From<OutlinedSectionDTO> for OutlinedSection {
    fn from(dto: OutlinedSectionDTO) -> Self {
        Self {
            section: SectionId::from(dto.section),
            depth: dto.depth,
            title: dto.title,
            scenes: dto.scenes.into_iter().map(SceneId::from).collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Paragraph(Vec<Run>),
    Heading {
        level: u8,
        runs: Vec<Run>,
    },
    Quote(Vec<Block>),
    List {
        ordered: bool,
        items: Vec<Vec<Block>>,
    },
    Code(String),
    Rule,
    Figure {
        asset: String,
        size: FigureSize,
        caption: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FigureSize {
    Small,
    Medium,
    Full,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Run {
    Text { text: String, marks: Marks },
    Break,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Marks {
    pub emphasis: bool,
    pub strong: bool,
    pub code: bool,
    pub link: Option<String>,
}

impl From<BlockDTO> for Block {
    fn from(dto: BlockDTO) -> Self {
        match dto {
            BlockDTO::Paragraph { runs } => Self::Paragraph(runs_of(runs)),
            BlockDTO::Heading { level, runs } => Self::Heading {
                level,
                runs: runs_of(runs),
            },
            BlockDTO::Quote { blocks } => Self::Quote(blocks_of(blocks)),
            BlockDTO::List { ordered, items } => Self::List {
                ordered,
                items: items.into_iter().map(blocks_of).collect(),
            },
            BlockDTO::Code { text } => Self::Code(text),
            BlockDTO::Rule => Self::Rule,
            BlockDTO::Figure {
                asset,
                size,
                caption,
            } => Self::Figure {
                asset,
                size: FigureSize::named(&size),
                caption,
            },
        }
    }
}

impl FigureSize {
    fn named(told: &str) -> Self {
        match told {
            "small" => Self::Small,
            "medium" => Self::Medium,
            _ => Self::Full,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Full => "full",
        }
    }
}

impl From<RunDTO> for Run {
    fn from(dto: RunDTO) -> Self {
        match dto {
            RunDTO::Text { text, marks } => Self::Text {
                text,
                marks: Marks::from(marks),
            },
            RunDTO::Break => Self::Break,
        }
    }
}

impl From<MarksDTO> for Marks {
    fn from(dto: MarksDTO) -> Self {
        Self {
            emphasis: dto.emphasis,
            strong: dto.strong,
            code: dto.code,
            link: dto.link,
        }
    }
}

fn blocks_of(told: Vec<BlockDTO>) -> Vec<Block> {
    told.into_iter().map(Block::from).collect()
}

fn runs_of(told: Vec<RunDTO>) -> Vec<Run> {
    told.into_iter().map(Run::from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saying(words: &str) -> Vec<BlockDTO> {
        vec![BlockDTO::Paragraph {
            runs: vec![RunDTO::Text {
                text: words.to_owned(),
                marks: MarksDTO::default(),
            }],
        }]
    }

    fn said(words: &str) -> Vec<Block> {
        blocks_of(saying(words))
    }

    fn scene(id: &str) -> SceneId {
        SceneId::from(id.to_owned())
    }

    fn outlined(section: &str, scenes: &[&str]) -> OutlinedSection {
        OutlinedSection {
            section: SectionId::from(section.to_owned()),
            depth: 0,
            title: section.to_owned(),
            scenes: scenes.iter().map(|id| scene(id)).collect(),
        }
    }

    /// A chapter of two scenes, the second with nothing written yet.
    fn a_book() -> Book {
        Book::from(ContentsDTO {
            project: "project_1".to_owned(),
            title: "The Loom".to_owned(),
            parts: vec![
                PartDTO::Section {
                    section: "morning".to_owned(),
                    depth: 0,
                    title: "morning".to_owned(),
                },
                PartDTO::Scene {
                    scene: "harbour".to_owned(),
                    blocks: saying("The boats came in."),
                },
                PartDTO::Scene {
                    scene: "wall".to_owned(),
                    blocks: vec![],
                },
            ],
        })
    }

    #[test]
    fn a_book_read_holds_its_sections_with_their_scenes_and_shows_only_what_is_written() {
        // when: a book is read
        let book = a_book();

        // then: its section holds both scenes
        assert_eq!(
            book.sections,
            vec![outlined("morning", &["harbour", "wall"])]
        );
        // and: the written one is shown, the empty one not
        assert_eq!(
            book.text_of(&scene("harbour"))
                .map(|text| text.blocks().to_vec()),
            Some(said("The boats came in."))
        );
        assert_eq!(book.text_of(&scene("wall")), None);
    }

    #[test]
    fn a_scene_told_anew_is_swapped_in_and_the_others_keep_their_telling() {
        // given: a book and what it holds of one scene
        let mut book = a_book();
        let before = book.text_of(&scene("harbour"));

        // when: the other scene is told
        let applied = book.apply(Change::Text {
            scene: scene("wall"),
            blocks: said("Hedy counted them."),
        });

        // then: it is shown, and the first is still the same telling, so it is not drawn again
        assert_eq!(applied, Applied::Whole);
        assert_eq!(
            book.text_of(&scene("wall"))
                .map(|text| text.blocks().to_vec()),
            Some(said("Hedy counted them."))
        );
        assert_eq!(book.text_of(&scene("harbour")), before);
    }

    #[test]
    fn a_book_read_again_keeps_the_telling_of_each_scene_that_says_the_same() {
        // given: a book, and the same book read again with its second scene written meanwhile
        let before = a_book();
        let mut read = a_book();
        read.apply(Change::Text {
            scene: scene("wall"),
            blocks: said("Hedy counted them."),
        });

        // when: the book read again keeps what did not change
        let kept = read.keeping_unchanged(&before);

        // then: the first scene is the same telling as before, so it is not drawn again
        assert_eq!(
            kept.text_of(&scene("harbour")),
            before.text_of(&scene("harbour"))
        );
        // and: the second says what it says now
        assert_eq!(
            kept.text_of(&scene("wall"))
                .map(|text| text.blocks().to_vec()),
            Some(said("Hedy counted them."))
        );
    }

    #[test]
    fn two_tellings_of_the_same_words_say_the_same_though_they_are_not_one() {
        // given: one scene read twice
        let once = a_book().text_of(&scene("harbour")).expect("it is written");
        let twice = a_book().text_of(&scene("harbour")).expect("it is written");

        // expect: they say the same, though each is a telling of its own
        assert!(once.says_as(&twice));
        assert_ne!(once, twice);
    }

    #[test]
    fn an_outline_naming_a_scene_never_told_asks_for_the_book_again() {
        // given: a book
        let mut book = a_book();

        // when: its scenes are rearranged, and then a scene it was never told is placed
        let rearranged = book.apply(Change::Outline(vec![
            outlined("morning", &["wall"]),
            outlined("market", &["harbour"]),
        ]));
        let placed = book.apply(Change::Outline(vec![outlined("morning", &["stalls"])]));

        // then: the rearranging is whole, and the placing lacks a text
        assert_eq!(rearranged, Applied::Whole);
        assert_eq!(placed, Applied::LackingText);
        assert_eq!(book.sections, vec![outlined("morning", &["stalls"])]);
    }

    #[test]
    fn a_title_told_is_the_book_s_title() {
        // given: a book
        let mut book = a_book();

        // when: its title is told
        book.apply(Change::Title("The Loom, Again".to_owned()));

        // then: it is the book's title
        assert_eq!(book.title, "The Loom, Again");
    }

    #[test]
    fn a_change_is_read_as_the_server_tells_it() {
        // when: an outline is read as told
        let read = Change::from(
            serde_json::from_str::<BookChangeDTO>(
                r#"{"kind":"outline","sections":[{"section":"morning","depth":0,"title":"morning","scenes":["harbour"]}]}"#,
            )
            .expect("a change as the server tells it"),
        );

        // then: it is the outline with its scenes
        assert_eq!(
            read,
            Change::Outline(vec![outlined("morning", &["harbour"])])
        );
    }

    #[test]
    fn a_figure_of_a_size_not_known_is_shown_full() {
        // given: a figure whose size is not one the client knows
        let told = BlockDTO::Figure {
            asset: "asset_1".to_owned(),
            size: "huge".to_owned(),
            caption: String::new(),
        };

        // when: it is read
        let read = Block::from(told);

        // then: it is shown full
        assert_eq!(
            read,
            Block::Figure {
                asset: "asset_1".to_owned(),
                size: FigureSize::Full,
                caption: String::new(),
            }
        );
    }
}
