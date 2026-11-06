use std::fmt::{self, Display, Formatter};

/// A setting's value and its source, a variable or a key in the settings file, so a value that cannot be read says where to mend it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setting {
    pub value: String,
    pub source: String,
}

impl Setting {
    pub fn new(source: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            source: source.into(),
        }
    }
}

impl Display for Setting {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{} is '{}'", self.source, self.value)
    }
}
