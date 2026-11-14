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

/// How far a picture can be zoomed in.
/// At the most, the square shows a quarter of the image's shorter side.
pub const MOST_ZOOM: f64 = 4.0;

/// Why a file could not be squared: the browser could not read it as an image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unreadable;

/// A square in an image: its left and top edge, and its side, in the image's pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Square {
    pub left: f64,
    pub top: f64,
    pub side: f64,
}

/// Which square of an image the account chose for its picture.
/// It is kept as the middle of the square and how far it is zoomed in.
/// At zoom 1 the square is as large as the image's shorter side.
/// The square never reaches past the image's edges.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Framing {
    width: f64,
    height: f64,
    middle_x: f64,
    middle_y: f64,
    zoom: f64,
}

impl Framing {
    /// Frames the largest square around the middle of an image this wide and high.
    pub fn of(width: u32, height: u32) -> Self {
        let (width, height) = (f64::from(width), f64::from(height));

        Self {
            width,
            height,
            middle_x: width / 2.0,
            middle_y: height / 2.0,
            zoom: 1.0,
        }
    }

    pub fn zoom(self) -> f64 {
        self.zoom
    }

    /// Returns the square the framing shows.
    pub fn square(self) -> Square {
        let side = self.width.min(self.height) / self.zoom;

        Square {
            left: self.middle_x - side / 2.0,
            top: self.middle_y - side / 2.0,
            side,
        }
    }

    /// Moves the picture by what the pointer moved, on a view this many pixels wide.
    /// Dragging the picture to the right shows more of its left side, so the square moves the other way.
    pub fn dragged(self, by_x: f64, by_y: f64, view: f64) -> Self {
        let scale = self.square().side / view;

        Self {
            middle_x: self.middle_x - by_x * scale,
            middle_y: self.middle_y - by_y * scale,
            ..self
        }
        .kept_inside()
    }

    /// Zooms to the given amount, between 1 and `MOST_ZOOM`, around the middle of the square.
    pub fn zoomed(self, zoom: f64) -> Self {
        Self {
            zoom: zoom.clamp(1.0, MOST_ZOOM),
            ..self
        }
        .kept_inside()
    }

    /// Moves the square back inside the image where it reaches past an edge.
    fn kept_inside(self) -> Self {
        let half = self.width.min(self.height) / self.zoom / 2.0;

        Self {
            middle_x: self.middle_x.clamp(half, self.width - half),
            middle_y: self.middle_y.clamp(half, self.height - half),
            ..self
        }
    }
}

/// Decodes an image file into a bitmap the canvas can draw.
///
/// It uses `createImageBitmap`, which follows the orientation a camera wrote into a photo.
/// It needs no address for the image, so the page's content security policy can keep refusing `blob:` images.
pub async fn decoded(file: &File) -> Result<ImageBitmap, Unreadable> {
    let decoding = web_sys::window()
        .ok_or(Unreadable)?
        .create_image_bitmap_with_blob(file)
        .map_err(|_| Unreadable)?;
    let image = JsFuture::from(decoding)
        .await
        .ok()
        .and_then(|bitmap| bitmap.dyn_into::<ImageBitmap>().ok())
        .ok_or(Unreadable)?;
    if image.width() == 0 || image.height() == 0 {
        return Err(Unreadable);
    }

    Ok(image)
}

/// Draws a square of an image onto the whole of a canvas.
pub fn drawn(
    image: &ImageBitmap,
    square: Square,
    canvas: &HtmlCanvasElement,
) -> Result<(), Unreadable> {
    let drawing: CanvasRenderingContext2d = canvas
        .get_context("2d")
        .ok()
        .flatten()
        .and_then(|context| context.dyn_into().ok())
        .ok_or(Unreadable)?;
    drawing.clear_rect(
        0.0,
        0.0,
        f64::from(canvas.width()),
        f64::from(canvas.height()),
    );

    drawing
        .draw_image_with_image_bitmap_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
            image,
            square.left,
            square.top,
            square.side,
            square.side,
            0.0,
            0.0,
            f64::from(canvas.width()),
            f64::from(canvas.height()),
        )
        .map_err(|_| Unreadable)
}

/// Cuts a square from an image and returns it as a JPEG.
/// The square is scaled down to `LONGEST` pixels if it is larger.
pub async fn cut(image: &ImageBitmap, square: Square) -> Result<Vec<u8>, Unreadable> {
    let out = (square.side.round() as u32).clamp(1, LONGEST);
    let canvas: HtmlCanvasElement = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.create_element("canvas").ok())
        .and_then(|element| element.dyn_into().ok())
        .ok_or(Unreadable)?;
    canvas.set_width(out);
    canvas.set_height(out);
    drawn(image, square, &canvas)?;

    let blob = written(&canvas).await?;
    let buffer = JsFuture::from(blob.array_buffer())
        .await
        .map_err(|_| Unreadable)?;

    Ok(Uint8Array::new(&buffer).to_vec())
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

    fn square(left: f64, top: f64, side: f64) -> Square {
        Square { left, top, side }
    }

    #[test]
    fn a_wide_image_is_first_framed_by_the_square_in_its_middle() {
        // expect: the sides cut away evenly
        assert_eq!(Framing::of(1_600, 900).square(), square(350.0, 0.0, 900.0));
    }

    #[test]
    fn a_tall_image_is_first_framed_by_the_square_in_its_middle() {
        // expect: the top and bottom cut away evenly
        assert_eq!(Framing::of(600, 1_000).square(), square(0.0, 200.0, 600.0));
    }

    #[test]
    fn a_square_image_is_first_framed_whole() {
        // expect: nothing cut away
        assert_eq!(Framing::of(500, 500).square(), square(0.0, 0.0, 500.0));
    }

    #[test]
    fn dragging_the_picture_moves_the_square_the_other_way_at_the_view_s_scale() {
        // given: a wide image, its square of 900 pixels shown on a view 300 pixels wide
        let framing = Framing::of(1_600, 900);

        // when: the picture is dragged 50 pixels to the right
        let dragged = framing.dragged(50.0, 0.0, 300.0);

        // then: the square moves 150 image pixels to the left
        assert_eq!(dragged.square(), square(200.0, 0.0, 900.0));
    }

    #[test]
    fn the_square_stops_at_the_image_s_edges() {
        // given: a wide image, framed in its middle
        let framing = Framing::of(1_600, 900);

        // when: the picture is dragged far to the right and down
        let dragged = framing.dragged(10_000.0, 10_000.0, 300.0);

        // then: the square stops at the left and top edge
        assert_eq!(dragged.square(), square(0.0, 0.0, 900.0));
        // and: dragged far the other way, it stops at the right and bottom edge
        assert_eq!(
            framing.dragged(-10_000.0, -10_000.0, 300.0).square(),
            square(700.0, 0.0, 900.0)
        );
    }

    #[test]
    fn zooming_in_shrinks_the_square_around_its_middle() {
        // given: a square image of 800 pixels
        let framing = Framing::of(800, 800);

        // when: it is zoomed in twice as far
        let zoomed = framing.zoomed(2.0);

        // then: the square is half as large, around the same middle
        assert_eq!(zoomed.square(), square(200.0, 200.0, 400.0));
    }

    #[test]
    fn zooming_out_near_an_edge_moves_the_square_back_inside() {
        // given: a square image, zoomed in and dragged to its top left corner
        let framing = Framing::of(800, 800)
            .zoomed(4.0)
            .dragged(10_000.0, 10_000.0, 200.0);
        assert_eq!(framing.square(), square(0.0, 0.0, 200.0));

        // when: it is zoomed out again
        let zoomed = framing.zoomed(1.0);

        // then: the square covers the whole image again
        assert_eq!(zoomed.square(), square(0.0, 0.0, 800.0));
    }

    #[test]
    fn the_zoom_stays_between_whole_and_the_most() {
        // expect: zooming below 1 or past the most stops there
        assert_eq!(Framing::of(800, 800).zoomed(0.2).zoom(), 1.0);
        assert_eq!(Framing::of(800, 800).zoomed(40.0).zoom(), MOST_ZOOM);
    }
}
