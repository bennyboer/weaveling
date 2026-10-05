use std::fmt::{self, Display, Formatter};

use crate::ideas::model::IdeaId;

const OPENING: usize = 40;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PassageId(String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Passage {
    pub id: PassageId,
    pub title: String,
    pub ideas: Vec<IdeaId>,
    pub text: String,
}

impl Passage {
    pub fn shown_as(&self) -> String {
        if !self.title.trim().is_empty() {
            return self.title.clone();
        }

        let saying = self.text.split_whitespace().collect::<Vec<_>>().join(" ");

        match saying.char_indices().nth(OPENING) {
            None if saying.is_empty() => "Empty".to_owned(),
            None => saying,
            Some((cut, _)) => format!("{}…", saying[..cut].trim_end()),
        }
    }
}

impl PassageId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for PassageId {
    fn from(given: String) -> Self {
        Self(given)
    }
}

impl Display for PassageId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}
