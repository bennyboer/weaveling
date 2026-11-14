use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;
use image::codecs::webp::WebPEncoder;
use image::{ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};

use super::*;

fn like_a_photo(width: u32, height: u32) -> RgbImage {
    RgbImage::from_fn(width, height, |x, y| {
        let (x, y) = (f64::from(x), f64::from(y));
        let wave = ((x / 37.0).sin() * (y / 53.0).cos() * 60.0) as i32;
        Rgb([
            (90 + (x * 120.0 / f64::from(width)) as i32 + wave).clamp(0, 255) as u8,
            (60 + (y * 150.0 / f64::from(height)) as i32 - wave / 2).clamp(0, 255) as u8,
            (140 + wave).clamp(0, 255) as u8,
        ])
    })
}

fn written(image: &DynamicImage, format: ImageFormat) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, format)
        .expect("an image of the tests is written");

    bytes.into_inner()
}

fn jpeg(image: &RgbImage) -> Vec<u8> {
    let mut bytes = Vec::new();
    image
        .write_with_encoder(JpegEncoder::new_with_quality(&mut bytes, 90))
        .expect("a JPEG is written");

    bytes
}

/// Returns the JPEG with a tag saying it was taken turned by a quarter, as a phone's camera does.
/// Such an image has to be shown turned back.
fn turned_by_its_camera(jpeg: &[u8]) -> Vec<u8> {
    let exif = [
        b"Exif\0\0".as_slice(),
        b"MM\0*\0\0\0\x08",
        &[0, 1],
        &[0x01, 0x12, 0, 3, 0, 0, 0, 1, 0, 6, 0, 0],
        &[0, 0, 0, 0],
    ]
    .concat();
    let length = u16::try_from(exif.len() + 2).expect("a short segment");

    [
        &jpeg[..2],
        &[0xFF, 0xE1],
        &length.to_be_bytes(),
        &exif,
        &jpeg[2..],
    ]
    .concat()
}

fn crc(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFF_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }

    !crc
}

fn chunk(kind: &[u8], data: &[u8]) -> Vec<u8> {
    let length = u32::try_from(data.len()).expect("a short chunk");
    let checked = [kind, data].concat();

    [
        &length.to_be_bytes(),
        checked.as_slice(),
        &crc(&checked).to_be_bytes(),
    ]
    .concat()
}

/// Returns a PNG whose header claims the given size.
/// Only a token amount of data follows the header.
fn a_png_claiming(width: u32, height: u32) -> Vec<u8> {
    let header = [
        &width.to_be_bytes()[..],
        &height.to_be_bytes(),
        &[8, 2, 0, 0, 0],
    ]
    .concat();

    [
        b"\x89PNG\r\n\x1a\n".to_vec(),
        chunk(b"IHDR", &header),
        chunk(b"IDAT", &[0x78, 0x9c, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01]),
        chunk(b"IEND", &[]),
    ]
    .concat()
}

fn read_back(variant: &[u8]) -> DynamicImage {
    image::load_from_memory_with_format(variant, ImageFormat::WebP)
        .expect("each variant is a WebP that reads back")
}

fn fidelity(made: &RgbImage, of: &RgbImage) -> f64 {
    let squared: f64 = made
        .pixels()
        .zip(of.pixels())
        .flat_map(|(a, b)| (0..3).map(move |c| (f64::from(a[c]) - f64::from(b[c])).powi(2)))
        .sum();
    let mean = squared / (f64::from(made.width() * made.height()) * 3.0);

    10.0 * (255.0 * 255.0 / mean).log10()
}

#[test]
fn a_photo_is_made_into_each_width_narrower_than_it_and_its_own_as_webp_close_to_it() {
    // given: a photo of two thousand pixels by fifteen hundred
    let photo = like_a_photo(2_000, 1_500);
    let uploaded = jpeg(&photo);

    // when: it is prepared
    let prepared = WebpImages.prepared(&uploaded).expect("a photo is taken");

    // then: it is told a JPEG as wide and high as it is, as large as it was uploaded
    assert_eq!(prepared.picture.format, Format::Jpeg);
    assert_eq!(
        (prepared.picture.width, prepared.picture.height),
        (2_000, 1_500)
    );
    assert_eq!(prepared.picture.original_size, uploaded.len() as u64);
    // and: made into each width narrower than it and its own, narrowest first
    assert_eq!(prepared.picture.variants, [96, 192, 384, 768, 1_536, 2_000]);
    assert_eq!(
        prepared
            .variants
            .iter()
            .map(|(width, _)| *width)
            .collect::<Vec<_>>(),
        prepared.picture.variants
    );
    // and: each a WebP of that width, as high as the photo is for it, close to the photo
    for (width, variant) in &prepared.variants {
        let back = read_back(variant);
        let height = (1_500 * width + 1_000) / 2_000;
        assert_eq!((back.width(), back.height()), (*width, height));
        let expected = DynamicImage::ImageRgb8(photo.clone())
            .resize_exact(*width, height, FilterType::Lanczos3)
            .to_rgb8();
        let close = fidelity(&back.to_rgb8(), &expected);
        assert!(
            close > 30.0,
            "the {width} wide variant is {close:.1} dB from the photo; an encoder writing noise is caught here"
        );
    }
    // and: what is kept is the original and every variant
    assert_eq!(
        prepared.total_size(),
        uploaded.len() as u64
            + prepared
                .variants
                .iter()
                .map(|(_, bytes)| bytes.len() as u64)
                .sum::<u64>()
    );
}

#[test]
fn a_photo_its_camera_took_turned_is_set_upright() {
    // given: a photo taken lying on its side, four hundred by two hundred, its camera telling it is to be turned
    let uploaded = turned_by_its_camera(&jpeg(&like_a_photo(400, 200)));

    // when: it is prepared
    let prepared = WebpImages
        .prepared(&uploaded)
        .expect("a turned photo is taken");

    // then: it stands upright, two hundred by four hundred, and so do its variants
    assert_eq!(
        (prepared.picture.width, prepared.picture.height),
        (200, 400)
    );
    let (width, variant) = prepared.variants.last().expect("its own width is made");
    assert_eq!(*width, 200);
    assert_eq!(
        (read_back(variant).width(), read_back(variant).height()),
        (200, 400)
    );
}

#[test]
fn transparency_is_kept_where_there_is_any() {
    // given: a PNG, its left half see-through and its right half not
    let cut_out = RgbaImage::from_fn(300, 200, |x, _| {
        if x < 150 {
            Rgba([255, 0, 0, 0])
        } else {
            Rgba([0, 0, 255, 255])
        }
    });
    let uploaded = written(&DynamicImage::ImageRgba8(cut_out), ImageFormat::Png);

    // when: it is prepared
    let prepared = WebpImages.prepared(&uploaded).expect("a PNG is taken");

    // then: its variant is see-through on the left and not on the right
    let back = read_back(&prepared.variants.last().expect("its own width is made").1).to_rgba8();
    assert_eq!(back.get_pixel(20, 100)[3], 0);
    assert_eq!(back.get_pixel(280, 100)[3], 255);
    assert_eq!(prepared.picture.format, Format::Png);
}

#[test]
fn a_gif_and_a_webp_are_taken_too() {
    // given: the same picture as a GIF and as a WebP
    let picture = DynamicImage::ImageRgb8(like_a_photo(500, 300));
    let gif = written(&picture.to_rgba8().into(), ImageFormat::Gif);
    let mut webp = Vec::new();
    picture
        .to_rgba8()
        .write_with_encoder(WebPEncoder::new_lossless(&mut webp))
        .expect("a WebP is written");

    // expect: each is prepared, told by its own format
    for (uploaded, format) in [(gif, Format::Gif), (webp, Format::Webp)] {
        let prepared = WebpImages.prepared(&uploaded).expect("it is taken");
        assert_eq!(prepared.picture.format, format);
        assert_eq!(prepared.picture.variants, [96, 192, 384, 500]);
    }
}

#[test]
fn what_is_no_image_the_instance_takes_is_refused_as_such() {
    // expect: nothing, text, an SVG and a BMP are no image it takes
    for uploaded in [
        Vec::new(),
        b"a letter, not a picture".to_vec(),
        b"<svg xmlns=\"http://www.w3.org/2000/svg\"><script>alert(1)</script></svg>".to_vec(),
        b"BM\x3a\0\0\0\0\0\0\0\x36\0\0\0".to_vec(),
    ] {
        assert_eq!(
            WebpImages.prepared(&uploaded),
            Err(ImageRefused::NotAnImage),
            "{:?}",
            String::from_utf8_lossy(&uploaded)
        );
    }
}

#[test]
fn an_image_cut_short_is_refused_as_unreadable() {
    // given: a PNG cut off half way
    let whole = written(
        &DynamicImage::ImageRgb8(like_a_photo(300, 300)),
        ImageFormat::Png,
    );

    // expect: it cannot be read
    assert_eq!(
        WebpImages.prepared(&whole[..whole.len() / 2]),
        Err(ImageRefused::Unreadable)
    );
}

#[test]
fn an_image_larger_than_the_instance_takes_is_refused_before_it_is_read() {
    // expect: one wider than sixteen thousand pixels is refused, though it holds nothing
    assert_eq!(
        WebpImages.prepared(&a_png_claiming(20_000, 10)),
        Err(ImageRefused::TooLarge)
    );
    // and: so is one of more than fifty million pixels, each side within bounds
    assert_eq!(
        WebpImages.prepared(&a_png_claiming(8_000, 8_000)),
        Err(ImageRefused::TooLarge),
        "so that a small file claiming to be a huge picture takes no memory"
    );
}
