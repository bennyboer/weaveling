use std::io::Cursor;

use assets_core::{Format, ImageRefused, Images, Picture, Prepared, widths_for};
use gamut_core::{Dimensions, EncodeImage, ImageRef, Rgb8, Rgba8};
use gamut_webp::WebpEncoder;
use image::imageops::FilterType;
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageError, ImageFormat, ImageReader, Limits};

const QUALITY: u8 = 75;
const LONGEST_SIDE: u32 = 16_384;
const MOST_PIXELS: u64 = 50_000_000;
const MOST_MEMORY: u64 = 1024 * 1024 * 1024;

/// Reads JPEG, PNG, GIF and WebP by `image`, a GIF by its first frame, sets it upright by what its camera told, and makes its variants as WebP by `gamut-webp`, keeping transparency where there is any. Whatever a camera wrote beside the pixels, where it was taken among it, stays with the original, which is never handed out.
#[derive(Debug, Default, Clone, Copy)]
pub struct WebpImages;

impl Images for WebpImages {
    fn prepared(&self, uploaded: &[u8]) -> Result<Prepared, ImageRefused> {
        let format = format_of(uploaded)?;
        let upright = read_upright(uploaded, format)?;
        let (width, height) = (upright.width(), upright.height());
        let widths = widths_for(width);

        let mut variants = Vec::with_capacity(widths.len());
        let mut narrowed = upright;
        for each in widths.iter().rev().copied() {
            if each < narrowed.width() {
                narrowed = narrowed.resize_exact(
                    each,
                    height_at(each, width, height),
                    FilterType::Lanczos3,
                );
            }
            variants.push((each, encoded(&narrowed)?));
        }
        variants.reverse();

        Ok(Prepared {
            picture: Picture {
                format,
                width,
                height,
                original_size: uploaded.len() as u64,
                variants: widths,
            },
            variants,
        })
    }
}

fn format_of(uploaded: &[u8]) -> Result<Format, ImageRefused> {
    match image::guess_format(uploaded) {
        Ok(ImageFormat::Jpeg) => Ok(Format::Jpeg),
        Ok(ImageFormat::Png) => Ok(Format::Png),
        Ok(ImageFormat::Gif) => Ok(Format::Gif),
        Ok(ImageFormat::WebP) => Ok(Format::Webp),
        _ => Err(ImageRefused::NotAnImage),
    }
}

fn read_upright(uploaded: &[u8], format: Format) -> Result<DynamicImage, ImageRefused> {
    let mut reader = ImageReader::with_format(Cursor::new(uploaded), image_format(format));
    let mut limits = Limits::default();
    limits.max_image_width = Some(LONGEST_SIDE);
    limits.max_image_height = Some(LONGEST_SIDE);
    limits.max_alloc = Some(MOST_MEMORY);
    reader.limits(limits);

    let mut decoder = reader.into_decoder().map_err(refused)?;
    let (width, height) = decoder.dimensions();
    if u64::from(width) * u64::from(height) > MOST_PIXELS {
        return Err(ImageRefused::TooLarge);
    }
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut image = DynamicImage::from_decoder(decoder).map_err(refused)?;
    image.apply_orientation(orientation);

    Ok(image)
}

fn image_format(format: Format) -> ImageFormat {
    match format {
        Format::Jpeg => ImageFormat::Jpeg,
        Format::Png => ImageFormat::Png,
        Format::Gif => ImageFormat::Gif,
        Format::Webp => ImageFormat::WebP,
    }
}

fn refused(failure: ImageError) -> ImageRefused {
    match failure {
        ImageError::Limits(_) => ImageRefused::TooLarge,
        _ => ImageRefused::Unreadable,
    }
}

fn height_at(width: u32, of_width: u32, of_height: u32) -> u32 {
    let scaled =
        (u64::from(of_height) * u64::from(width) + u64::from(of_width) / 2) / u64::from(of_width);

    u32::try_from(scaled.max(1)).unwrap_or(u32::MAX)
}

fn encoded(image: &DynamicImage) -> Result<Vec<u8>, ImageRefused> {
    let dimensions =
        Dimensions::new(image.width(), image.height()).map_err(|_| ImageRefused::Unreadable)?;
    let encoder = WebpEncoder::lossy(QUALITY);
    let mut written = Vec::new();
    let encoding = if image.color().has_alpha() {
        let pixels = image.to_rgba8();
        ImageRef::<Rgba8>::new(pixels.as_raw(), dimensions)
            .and_then(|pixels| encoder.encode_image(pixels, &mut written))
    } else {
        let pixels = image.to_rgb8();
        ImageRef::<Rgb8>::new(pixels.as_raw(), dimensions)
            .and_then(|pixels| encoder.encode_image(pixels, &mut written))
    };
    encoding.map_err(|_| ImageRefused::Unreadable)?;

    Ok(written)
}

#[cfg(test)]
mod tests;
