use std::fmt::{self, Debug, Display, Formatter};

/// A setting's value and its source: an environment variable or a key in the settings file.
/// So when a value cannot be read, the error says where to fix it.
#[derive(Clone, PartialEq, Eq)]
pub struct Setting {
    pub value: String,
    pub source: String,
    secret: bool,
}

impl Setting {
    pub fn new(source: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            source: source.into(),
            secret: false,
        }
    }

    /// A setting whose value is never shown, in a refusal or a log.
    pub fn secret(source: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            secret: true,
            ..Self::new(source, value)
        }
    }

    fn shown(&self) -> &str {
        if self.secret { "…" } else { &self.value }
    }
}

impl Display for Setting {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{} is '{}'", self.source, self.shown())
    }
}

impl Debug for Setting {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Setting")
            .field("value", &self.shown())
            .field("source", &self.source)
            .finish()
    }
}
