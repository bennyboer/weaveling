use std::fmt::{self, Display, Formatter};

macro_rules! link {
    ($named:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $named(String);

        impl $named {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $named {
            fn from(given: String) -> Self {
                Self(given)
            }
        }

        impl From<&str> for $named {
            fn from(given: &str) -> Self {
                Self(given.to_owned())
            }
        }

        impl Display for $named {
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                Display::fmt(&self.0, f)
            }
        }
    };
}

link!(IdeaLink);
link!(SectionLink);
link!(SceneLink);
