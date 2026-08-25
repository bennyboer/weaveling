use std::fmt::{self, Display, Formatter};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PassageId(String);

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
