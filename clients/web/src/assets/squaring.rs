use js_sys::{Promise, Uint8Array};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Blob, CanvasRenderingContext2d, File, HtmlCanvasElement, ImageBitmap};

/// The longest side of a squared picture, in pixels.
/// Larger images are scaled down to it, so a photo from a camera does not upload its full size.
const LONGEST: u32 = 1_024;

/// The format and quality a squared picture is written in.
const WRITTEN_AS: &str = "image/jpeg";
const QUALITY: f64 = 0.9;

/// Why a file could not be squared: the browser could not read it as an image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unreadable;

/// The square cut from the middle of an image: its left and top edge, and its side, in the image's pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Square {
    left: u32,
    top: u32,
    side: u32,
}

/// Returns the largest square around the middle of an image this wide and high.
fn middle_square(width: u32, height: u32) -> Square {
    let side = width.min(height);

    Square {
        left: (width - side) / 2,
        top: (height - side) / 2,
        side,
    }
}

/// Cuts the largest square from the middle of an image file and returns it as a JPEG.
/// The square is scaled down to `LONGEST` pixels if it is larger.
///
/// The file is decoded with `createImageBitmap`, which follows the orientation a camera wrote into a photo.
/// It needs no address for the image, so the page's content security policy can keep refusing `blob:` images.
pub async fn squared(file: &File) -> Result<Vec<u8>, Unreadable> {
    let image = decoded(file).await?;
    let square = middle_square(image.width(), image.height());
    if square.side == 0 {
        return Err(Unreadable);
    }
    let out = square.side.min(LONGEST);
    let canvas: HtmlCanvasElement = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.create_element("canvas").ok())
        .and_then(|element| element.dyn_into().ok())
        .ok_or(Unreadable)?;
    canvas.set_width(out);
    canvas.set_height(out);
    let drawing: CanvasRenderingContext2d = canvas
        .get_context("2d")
        .ok()
        .flatten()
        .and_then(|context| context.dyn_into().ok())
        .ok_or(Unreadable)?;
    drawing
        .draw_image_with_image_bitmap_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
            &image,
            f64::from(square.left),
            f64::from(square.top),
            f64::from(square.side),
            f64::from(square.side),
            0.0,
            0.0,
            f64::from(out),
            f64::from(out),
        )
        .map_err(|_| Unreadable)?;
    image.close();

    let blob = written(&canvas).await?;
    let buffer = JsFuture::from(blob.array_buffer())
        .await
        .map_err(|_| Unreadable)?;

    Ok(Uint8Array::new(&buffer).to_vec())
}

/// Decodes an image file into a bitmap the canvas can draw.
async fn decoded(file: &File) -> Result<ImageBitmap, Unreadable> {
    let decoding = web_sys::window()
        .ok_or(Unreadable)?
        .create_image_bitmap_with_blob(file)
        .map_err(|_| Unreadable)?;

    JsFuture::from(decoding)
        .await
        .ok()
        .and_then(|bitmap| bitmap.dyn_into::<ImageBitmap>().ok())
        .ok_or(Unreadable)
}

/// Writes what the canvas shows as a JPEG blob.
async fn written(canvas: &HtmlCanvasElement) -> Result<Blob, Unreadable> {
    let writing = Promise::new(&mut |resolve, _| {
        let told = Closure::once_into_js(move |blob: JsValue| {
            let _called = resolve.call1(&JsValue::NULL, &blob);
        });
        let _asked = canvas.to_blob_with_type_and_encoder_options(
            told.unchecked_ref(),
            WRITTEN_AS,
            &JsValue::from_f64(QUALITY),
        );
    });

    JsFuture::from(writing)
        .await
        .ok()
        .and_then(|blob| blob.dyn_into::<Blob>().ok())
        .ok_or(Unreadable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wide_image_is_cut_to_the_square_in_its_middle() {
        // expect: the sides cut away evenly
        assert_eq!(
            middle_square(1_600, 900),
            Square {
                left: 350,
                top: 0,
                side: 900,
            }
        );
    }

    #[test]
    fn a_tall_image_is_cut_to_the_square_in_its_middle() {
        // expect: the top and bottom cut away evenly
        assert_eq!(
            middle_square(600, 1_000),
            Square {
                left: 0,
                top: 200,
                side: 600,
            }
        );
    }

    #[test]
    fn a_square_image_stays_whole() {
        // expect: nothing cut away
        assert_eq!(
            middle_square(500, 500),
            Square {
                left: 0,
                top: 0,
                side: 500,
            }
        );
    }
}
