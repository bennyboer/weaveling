use thiserror::Error;

use crate::asset::Picture;

/// The widths an image is made into.
/// An image gets those narrower than itself, plus its own width if it is narrower than the widest.
pub const WIDTHS: [u32; 4] = [384, 768, 1_536, 3_072];

/// Why an upload is not accepted as an image.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ImageRefused {
    #[error("this is no image the instance takes: JPEG, PNG, GIF or WebP")]
    NotAnImage,
    #[error("this image cannot be read")]
    Unreadable,
    #[error("this image is larger than the instance takes")]
    TooLarge,
}

/// An upload prepared for storing.
/// It holds the image as it was uploaded, read and checked, and the variants made from it.
/// The variants are listed by width, narrowest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prepared {
    pub picture: Picture,
    pub variants: Vec<(u32, Vec<u8>)>,
}

/// Reads an upload as an image, turns it upright, and makes its variants.
/// This is slow, so it runs outside the async runtime, only a few at once.
pub trait Images: Send + Sync {
    fn prepared(&self, uploaded: &[u8]) -> Result<Prepared, ImageRefused>;
}

impl Prepared {
    /// Returns all bytes stored for the image: the original and its variants.
    pub fn total_size(&self) -> u64 {
        self.picture.original_size
            + self
                .variants
                .iter()
                .map(|(_, bytes)| bytes.len() as u64)
                .sum::<u64>()
    }
}

/// Returns the widths an image of this width is made into.
/// These are the widths in `WIDTHS` narrower than the image, plus the image's own width if it is narrower than the widest of them.
/// So no variant is wider than the image.
pub fn widths_for(width: u32) -> Vec<u32> {
    let mut widths: Vec<u32> = WIDTHS.into_iter().filter(|each| *each < width).collect();
    let widest = WIDTHS[WIDTHS.len() - 1];
    if width <= widest {
        widths.push(width);
    }

    widths
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::Format;

    #[test]
    fn an_image_is_made_into_the_widths_narrower_than_it_and_its_own() {
        // expect: a narrow image only its own width
        assert_eq!(widths_for(200), [200]);
        // and: one between them those below and its own
        assert_eq!(widths_for(1_280), [384, 768, 1_280]);
        // and: one as wide as a width that one once
        assert_eq!(widths_for(768), [384, 768]);
        // and: one wider than the widest every width, and not its own
        assert_eq!(
            widths_for(6_000),
            [384, 768, 1_536, 3_072],
            "the original is never handed out, and a variant as wide as a camera's photo would be as heavy"
        );
    }

    #[test]
    fn what_is_kept_is_the_original_and_every_variant() {
        // given: an image of ten bytes made into two variants
        let prepared = Prepared {
            picture: Picture {
                format: Format::Png,
                width: 500,
                height: 400,
                original_size: 10,
                variants: vec![384, 500],
            },
            variants: vec![(384, vec![0; 3]), (500, vec![0; 4])],
        };

        // expect: seventeen bytes kept
        assert_eq!(prepared.total_size(), 17);
    }
}
