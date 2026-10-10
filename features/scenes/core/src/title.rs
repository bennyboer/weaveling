use std::fmt::{self, Display, Formatter};

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct SceneTitle(String);

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InvalidSceneTitle {
    #[error("a scene title must not contain control characters")]
    ControlCharacter,
    #[error("a scene title must be at most {max} characters, got {actual}")]
    TooLong { max: usize, actual: usize },
}

impl SceneTitle {
    pub const MAX_CHARS: usize = 2048;

    pub fn new(raw: &str) -> Result<Self, InvalidSceneTitle> {
        let trimmed = raw.trim();

        if trimmed.chars().any(char::is_control) {
            return Err(InvalidSceneTitle::ControlCharacter);
        }

        let actual = trimmed.chars().count();
        if actual > Self::MAX_CHARS {
            return Err(InvalidSceneTitle::TooLong {
                max: Self::MAX_CHARS,
                actual,
            });
        }

        Ok(Self(trimmed.to_owned()))
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
    fn a_title_is_trimmed() {
        assert_eq!(
            SceneTitle::new("  The loom  ").unwrap().as_str(),
            "The loom"
        );
    }

    #[test]
    fn a_scene_may_go_untitled_so_the_outline_falls_back_to_its_opening_words() {
        assert!(SceneTitle::new("   ").unwrap().is_untitled());
        assert!(SceneTitle::untitled().is_untitled());
    }

    #[test]
    fn control_characters_are_refused() {
        assert_eq!(
            SceneTitle::new("The\u{7}loom"),
            Err(InvalidSceneTitle::ControlCharacter)
        );
    }

    #[test]
    fn a_title_longer_than_the_cap_is_refused() {
        let far_too_long = "a".repeat(SceneTitle::MAX_CHARS + 1);

        assert_eq!(
            SceneTitle::new(&far_too_long),
            Err(InvalidSceneTitle::TooLong {
                max: SceneTitle::MAX_CHARS,
                actual: SceneTitle::MAX_CHARS + 1,
            })
        );
    }
}
