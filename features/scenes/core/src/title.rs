use std::fmt::{self, Display, Formatter};

use words::{TextRules, Unfit};

pub type InvalidSceneTitle = Unfit;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct SceneTitle(String);

impl SceneTitle {
    pub const MAX_CHARS: usize = 2048;

    const RULES: TextRules = TextRules {
        max_chars: Self::MAX_CHARS,
        may_be_blank: true,
        may_break_lines: false,
    };

    pub fn new(raw: &str) -> Result<Self, InvalidSceneTitle> {
        Self::RULES.fitted(raw).map(Self)
    }

    pub fn untitled() -> Self {
        Self(String::new())
    }

    pub fn is_untitled(&self) -> bool {
        self.0.is_empty()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for SceneTitle {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scene_may_be_written_before_it_is_titled() {
        // when: a blank title is given
        let title = SceneTitle::new("  ").expect("a scene may be untitled");

        // then: the scene is untitled
        assert_eq!(title, SceneTitle::untitled());
        // and: knows it
        assert!(title.is_untitled());
    }

    #[test]
    fn a_title_is_one_line_up_to_its_limit() {
        // then: a title of two lines is refused
        assert_eq!(
            SceneTitle::new("The\nLoom"),
            Err(InvalidSceneTitle::ControlCharacter)
        );
        // and: a title at its limit is fine
        assert!(SceneTitle::new(&"a".repeat(SceneTitle::MAX_CHARS)).is_ok());
        // and: a title past its limit is refused
        assert_eq!(
            SceneTitle::new(&"a".repeat(SceneTitle::MAX_CHARS + 1)),
            Err(InvalidSceneTitle::TooLong {
                max: SceneTitle::MAX_CHARS,
                actual: SceneTitle::MAX_CHARS + 1,
            })
        );
    }
}
