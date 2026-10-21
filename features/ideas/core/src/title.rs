use std::fmt::{self, Display, Formatter};

use words::{TextRules, Unfit};

pub type InvalidIdeaTitle = Unfit;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct IdeaTitle(String);

impl IdeaTitle {
    pub const MAX_CHARS: usize = 2048;

    const RULES: TextRules = TextRules {
        max_chars: Self::MAX_CHARS,
        may_be_blank: true,
        may_break_lines: false,
    };

    pub fn new(raw: &str) -> Result<Self, InvalidIdeaTitle> {
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

impl Display for IdeaTitle {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_idea_may_arrive_before_its_title() {
        let title = IdeaTitle::new("  ").expect("an idea may be untitled");

        assert_eq!(title, IdeaTitle::untitled());
        assert!(title.is_untitled());
    }

    #[test]
    fn a_title_is_one_line_up_to_its_limit() {
        assert_eq!(
            IdeaTitle::new("The\nLoom"),
            Err(InvalidIdeaTitle::ControlCharacter)
        );
        assert!(IdeaTitle::new(&"a".repeat(IdeaTitle::MAX_CHARS)).is_ok());
        assert_eq!(
            IdeaTitle::new(&"a".repeat(IdeaTitle::MAX_CHARS + 1)),
            Err(InvalidIdeaTitle::TooLong {
                max: IdeaTitle::MAX_CHARS,
                actual: IdeaTitle::MAX_CHARS + 1,
            })
        );
    }
}
