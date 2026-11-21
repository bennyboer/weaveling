use crate::block_path::BlockPath;
use crate::link::{ReadingCopyLink, SceneLink, SectionLink};

/// What a piece of feedback is about: the thing it is given on, and where in it.
/// Each kind of thing brings its own places, so a place always belongs to the kind it is in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum About {
    ReadingCopy {
        reading_copy: ReadingCopyLink,
        place: ReadingCopyPlace,
    },
}

/// The thing feedback is given on, whatever place in it a piece is about.
/// Who may see feedback, and until when it may be given, follows from its subject.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Subject {
    ReadingCopy(ReadingCopyLink),
}

/// Where in a reading copy a piece of feedback is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadingCopyPlace {
    /// A stretch of one block's text: the scene, the path to the block in it,
    /// the stretch as offsets in characters, and the text it quotes.
    Passage {
        scene: SceneLink,
        block_path: BlockPath,
        from: u32,
        to: u32,
        quote: String,
    },
    Scene(SceneLink),
    Section(SectionLink),
    Whole,
}

impl About {
    pub fn subject(&self) -> Subject {
        match self {
            Self::ReadingCopy { reading_copy, .. } => Subject::ReadingCopy(reading_copy.clone()),
        }
    }

    /// The scene it is about, on the scene itself or a passage in it; none for a section or the whole.
    pub fn in_scene(&self) -> Option<&SceneLink> {
        match self {
            Self::ReadingCopy { place, .. } => match place {
                ReadingCopyPlace::Passage { scene, .. } | ReadingCopyPlace::Scene(scene) => {
                    Some(scene)
                }
                ReadingCopyPlace::Section(_) | ReadingCopyPlace::Whole => None,
            },
        }
    }

    /// The section it is about itself; none for anything else, as a scene in it is told by `in_scene`.
    pub fn on_section(&self) -> Option<&SectionLink> {
        match self {
            Self::ReadingCopy {
                place: ReadingCopyPlace::Section(section),
                ..
            } => Some(section),
            Self::ReadingCopy { .. } => None,
        }
    }

    /// Whether it is about something: a passage lies in a block, holds at least one character, and quotes it.
    pub fn holds_something(&self) -> bool {
        match self {
            Self::ReadingCopy { place, .. } => place.holds_something(),
        }
    }
}

impl ReadingCopyPlace {
    fn holds_something(&self) -> bool {
        match self {
            Self::Passage {
                block_path,
                from,
                to,
                quote,
                ..
            } => !block_path.is_empty() && from < to && !quote.trim().is_empty(),
            Self::Scene(_) | Self::Section(_) | Self::Whole => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn passage(block_path: Vec<u32>, from: u32, to: u32, quote: &str) -> ReadingCopyPlace {
        ReadingCopyPlace::Passage {
            scene: SceneLink::from("scene_harbour"),
            block_path: BlockPath::new(block_path),
            from,
            to,
            quote: quote.to_owned(),
        }
    }

    fn in_a_copy(place: ReadingCopyPlace) -> About {
        About::ReadingCopy {
            reading_copy: ReadingCopyLink::from("copy_loom"),
            place,
        }
    }

    #[test]
    fn a_passage_holds_at_least_one_character_of_a_block_and_quotes_it() {
        // expect: a stretch of a block, quoted, holds something
        assert!(in_a_copy(passage(vec![0], 4, 9, "boats")).holds_something());
        assert!(in_a_copy(passage(vec![2, 0], 0, 1, "F")).holds_something());
        // and: no block, no characters, a stretch backwards or nothing quoted does not
        for empty in [
            passage(vec![], 4, 9, "boats"),
            passage(vec![0], 4, 4, "boats"),
            passage(vec![0], 9, 4, "boats"),
            passage(vec![0], 4, 9, "  "),
        ] {
            assert!(!in_a_copy(empty.clone()).holds_something(), "{empty:?}");
        }
        // and: a scene, a section or the whole copy always does
        for whole in [
            ReadingCopyPlace::Scene(SceneLink::from("scene_harbour")),
            ReadingCopyPlace::Section(SectionLink::from("section_morning")),
            ReadingCopyPlace::Whole,
        ] {
            assert!(in_a_copy(whole.clone()).holds_something(), "{whole:?}");
        }
    }

    #[test]
    fn feedback_anywhere_in_a_copy_is_given_on_that_copy() {
        // expect: the subject is the copy, wherever in it
        assert_eq!(
            in_a_copy(ReadingCopyPlace::Whole).subject(),
            Subject::ReadingCopy(ReadingCopyLink::from("copy_loom"))
        );
    }
}
