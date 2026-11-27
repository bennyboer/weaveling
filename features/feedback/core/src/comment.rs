use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// What someone says in a piece of feedback: paragraphs, lists and quotes of text,
/// in italics, in bold or linked, drawn as a reading copy's text is.
/// What is quoted says nothing of its own, so a comment must say something besides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment(Vec<CommentBlock>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommentBlock {
    Paragraph(Vec<CommentRun>),
    /// A list, each item one line of runs.
    List {
        ordered: bool,
        items: Vec<Vec<CommentRun>>,
    },
    /// A line quoted, as a reply quotes what it answers.
    Quote(Vec<CommentRun>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommentRun {
    Text {
        text: String,
        emphasis: bool,
        strong: bool,
        /// Where it leads: a page on the web or an address to mail.
        link: Option<String>,
    },
    Break,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidComment {
    Blank,
    /// A line is broken by a run of its own, never inside the text.
    ControlCharacter,
    TooLong {
        max: usize,
        actual: usize,
    },
    /// A link leads somewhere other than a page on the web or an address to mail.
    LinkUnsafe,
}

const LINKS_LEAD_TO: [&str; 3] = ["http://", "https://", "mailto:"];

impl Comment {
    pub const MAX_CHARS: usize = 10_000;

    pub fn new(blocks: Vec<CommentBlock>) -> Result<Self, InvalidComment> {
        let runs: Vec<(&CommentRun, bool)> = blocks
            .iter()
            .flat_map(|block| {
                let own = !matches!(block, CommentBlock::Quote(_));
                block.runs().map(move |run| (run, own))
            })
            .collect();
        let texts = runs.iter().filter_map(|(run, own)| match run {
            CommentRun::Text { text, link, .. } => Some((text, link, *own)),
            CommentRun::Break => None,
        });

        let mut said = 0;
        let mut anything = false;
        for (text, link, own) in texts {
            if text.chars().any(char::is_control) {
                return Err(InvalidComment::ControlCharacter);
            }
            if link
                .as_deref()
                .is_some_and(|link| !leads_somewhere_safe(link))
            {
                return Err(InvalidComment::LinkUnsafe);
            }
            said += text.chars().count();
            anything |= own && !text.trim().is_empty();
        }
        if !anything {
            return Err(InvalidComment::Blank);
        }
        if said > Self::MAX_CHARS {
            return Err(InvalidComment::TooLong {
                max: Self::MAX_CHARS,
                actual: said,
            });
        }

        Ok(Self(blocks))
    }

    pub fn blocks(&self) -> &[CommentBlock] {
        &self.0
    }
}

impl CommentBlock {
    fn runs(&self) -> Box<dyn Iterator<Item = &CommentRun> + '_> {
        match self {
            Self::Paragraph(runs) | Self::Quote(runs) => Box::new(runs.iter()),
            Self::List { items, .. } => Box::new(items.iter().flatten()),
        }
    }
}

fn leads_somewhere_safe(link: &str) -> bool {
    let link = link.to_ascii_lowercase();

    !link.chars().any(char::is_whitespace)
        && LINKS_LEAD_TO.iter().any(|scheme| {
            link.strip_prefix(scheme)
                .is_some_and(|rest| !rest.is_empty())
        })
}

impl Display for InvalidComment {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Blank => write!(f, "a comment must say something"),
            Self::ControlCharacter => write!(f, "a comment must not contain control characters"),
            Self::TooLong { max, actual } => {
                write!(
                    f,
                    "a comment must be at most {max} characters, got {actual}"
                )
            }
            Self::LinkUnsafe => {
                write!(
                    f,
                    "a link must lead to a page on the web or an address to mail"
                )
            }
        }
    }
}

impl Error for InvalidComment {}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(text: &str) -> CommentRun {
        CommentRun::Text {
            text: text.to_owned(),
            emphasis: false,
            strong: false,
            link: None,
        }
    }

    fn linked(text: &str, to: &str) -> CommentRun {
        CommentRun::Text {
            text: text.to_owned(),
            emphasis: false,
            strong: false,
            link: Some(to.to_owned()),
        }
    }

    fn saying(runs: Vec<CommentRun>) -> Result<Comment, InvalidComment> {
        Comment::new(vec![CommentBlock::Paragraph(runs)])
    }

    #[test]
    fn a_comment_holds_paragraphs_and_lists_of_formatted_text_as_written() {
        // given: a paragraph in bold and italics with a break, and a list
        let blocks = vec![
            CommentBlock::Paragraph(vec![
                CommentRun::Text {
                    text: "This line ".to_owned(),
                    emphasis: true,
                    strong: true,
                    link: None,
                },
                CommentRun::Break,
                plain(" stopped me. "),
            ]),
            CommentBlock::List {
                ordered: true,
                items: vec![vec![plain("keep it")], vec![]],
            },
        ];

        // expect: it is kept as written, spaces between runs included
        assert_eq!(
            Comment::new(blocks.clone()).map(|comment| comment.blocks().to_vec()),
            Ok(blocks)
        );
    }

    #[test]
    fn a_comment_says_something_and_not_too_much() {
        // expect: nothing but blanks, breaks and empty lists say nothing
        assert_eq!(Comment::new(vec![]), Err(InvalidComment::Blank));
        assert_eq!(
            saying(vec![plain("  "), CommentRun::Break]),
            Err(InvalidComment::Blank)
        );
        assert_eq!(
            Comment::new(vec![CommentBlock::List {
                ordered: false,
                items: vec![vec![]],
            }]),
            Err(InvalidComment::Blank)
        );
        // and: a quote alone says nothing of its own
        assert_eq!(
            Comment::new(vec![
                CommentBlock::Quote(vec![plain("Which boats?")]),
                CommentBlock::Paragraph(vec![plain(" ")]),
            ]),
            Err(InvalidComment::Blank)
        );
        // and: more than ten thousand characters, counted over every run, are too many
        assert_eq!(
            saying(vec![plain(&"a".repeat(6_000)), plain(&"b".repeat(4_001))]),
            Err(InvalidComment::TooLong {
                max: Comment::MAX_CHARS,
                actual: 10_001
            })
        );
    }

    #[test]
    fn a_line_is_broken_by_a_run_of_its_own_and_never_inside_the_text() {
        // expect: a line break or another control character in a run is refused
        for broken in ["one\ntwo", "bell\u{7}"] {
            assert_eq!(
                saying(vec![plain(broken)]),
                Err(InvalidComment::ControlCharacter),
                "{broken:?}"
            );
        }
    }

    #[test]
    fn a_link_leads_to_a_page_on_the_web_or_an_address_to_mail() {
        // expect: the web and mail are linked to
        for safe in [
            "https://example.org/page",
            "HTTP://example.org",
            "mailto:lena@example.org",
        ] {
            assert!(saying(vec![linked("here", safe)]).is_ok(), "{safe}");
        }
        // and: scripts, data, files, nothing after the scheme or spaces are refused
        for unsafe_link in [
            "javascript:alert(1)",
            "data:text/html,hi",
            "file:///etc/passwd",
            "https://",
            "https://exa mple.org",
            "example.org",
        ] {
            assert_eq!(
                saying(vec![linked("here", unsafe_link)]),
                Err(InvalidComment::LinkUnsafe),
                "{unsafe_link}"
            );
        }
    }
}
