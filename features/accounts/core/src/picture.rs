use std::fmt::{self, Display, Formatter};

use thiserror::Error;

/// The id of the image an account shows as its picture.
///
/// The image is an asset, kept by the assets feature, which this feature does not know.
/// So the id is only checked to look like an id: letters, digits and underscores, at most 64 of them.
/// Whether such an asset exists, and is the account's own, is for the assets feature to tell.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PictureId(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("a picture is named by an id of letters, digits and underscores, at most 64 of them")]
pub struct InvalidPictureId;

impl PictureId {
    pub const MAX_CHARS: usize = 64;

    pub fn new(raw: &str) -> Result<Self, InvalidPictureId> {
        let fits = !raw.is_empty()
            && raw.len() <= Self::MAX_CHARS
            && raw
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_');

        if fits {
            Ok(Self(raw.to_owned()))
        } else {
            Err(InvalidPictureId)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for PictureId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_asset_s_id_names_a_picture() {
        // expect: an id as the assets feature makes them is taken
        assert!(PictureId::new("asset_0F8vXk2cQ9rTzL1mN4pB7a").is_ok());
    }

    #[test]
    fn what_does_not_look_like_an_id_is_refused() {
        // expect: nothing, spaces, signs of an address and an id too long are refused
        for raw in [
            "",
            "asset 1",
            "../asset_1",
            "asset_1?width=96",
            &"a".repeat(65),
        ] {
            assert_eq!(PictureId::new(raw), Err(InvalidPictureId), "{raw}");
        }
    }
}
