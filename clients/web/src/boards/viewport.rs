use crate::boards::model::{Size, Spot};

pub const NEARER: f64 = 1.1;
const CLOSEST: f64 = 3.0;
const FURTHEST: f64 = 0.2;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Exact {
    x: f64,
    y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pan: Exact,
    pub zoom: f64,
}

impl Viewport {
    pub const RESTING: Self = Self {
        pan: Exact { x: 0.0, y: 0.0 },
        zoom: 1.0,
    };

    pub fn on_screen(&self, at: Spot) -> Spot {
        Spot {
            x: (at.x as f64 * self.zoom + self.pan.x).round() as i64,
            y: (at.y as f64 * self.zoom + self.pan.y).round() as i64,
        }
    }

    pub fn on_board(&self, at: Spot) -> Spot {
        let held = self.exactly(at);

        Spot {
            x: held.x.round() as i64,
            y: held.y.round() as i64,
        }
    }

    pub fn across(&self, by: Spot) -> Spot {
        Spot {
            x: (by.x as f64 / self.zoom).round() as i64,
            y: (by.y as f64 / self.zoom).round() as i64,
        }
    }

    pub fn tall(&self, size: Size) -> i64 {
        (size.height as f64 * self.zoom).round() as i64
    }

    pub fn panned(&self, by: Spot) -> Self {
        Self {
            pan: Exact {
                x: self.pan.x + by.x as f64,
                y: self.pan.y + by.y as f64,
            },
            zoom: self.zoom,
        }
    }

    pub fn zoomed(&self, towards: Spot, by: f64) -> Self {
        let zoom = (self.zoom * by).clamp(FURTHEST, CLOSEST);
        let held = self.exactly(towards);

        Self {
            pan: Exact {
                x: towards.x as f64 - held.x * zoom,
                y: towards.y as f64 - held.y * zoom,
            },
            zoom,
        }
    }

    pub fn pinched(&self, from: (Spot, Spot), to: (Spot, Spot)) -> Self {
        let (was, now) = (middle(from), middle(to));
        let moved = self.panned(Spot {
            x: now.x - was.x,
            y: now.y - was.y,
        });
        let (apart, apart_now) = (gap(from), gap(to));

        if apart < 1.0 || apart_now < 1.0 {
            return moved;
        }

        moved.zoomed(now, apart_now / apart)
    }

    fn exactly(&self, at: Spot) -> Exact {
        Exact {
            x: (at.x as f64 - self.pan.x) / self.zoom,
            y: (at.y as f64 - self.pan.y) / self.zoom,
        }
    }

    pub fn unzoomed(&self, around: Spot) -> Self {
        self.zoomed(around, 1.0 / self.zoom)
    }

    pub fn surface(&self) -> String {
        format!(
            "transform: translate({}px, {}px) scale({});",
            self.pan.x, self.pan.y, self.zoom
        )
    }

    pub fn grid(&self, apart: i64) -> String {
        format!(
            "background-position: {}px {}px; background-size: {}px {}px;",
            self.pan.x,
            self.pan.y,
            apart as f64 * self.zoom,
            apart as f64 * self.zoom
        )
    }

    pub fn as_percent(&self) -> String {
        format!("{}%", (self.zoom * 100.0).round())
    }
}

fn middle((one, other): (Spot, Spot)) -> Spot {
    Spot {
        x: (one.x + other.x) / 2,
        y: (one.y + other.y) / 2,
    }
}

fn gap((one, other): (Spot, Spot)) -> f64 {
    ((one.x - other.x) as f64).hypot((one.y - other.y) as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: i64, y: i64) -> Spot {
        Spot { x, y }
    }

    #[test]
    fn a_resting_viewport_leaves_the_board_where_it_is() {
        // expect: screen and board spots are the same
        assert_eq!(Viewport::RESTING.on_screen(at(40, 90)), at(40, 90));
        assert_eq!(Viewport::RESTING.on_board(at(40, 90)), at(40, 90));
    }

    #[test]
    fn panning_shifts_the_board_under_the_window() {
        // when: the viewport is panned
        let shoved = Viewport::RESTING.panned(at(-120, 30));

        // then: the board shows shifted by as much, both ways round
        assert_eq!(shoved.on_screen(at(200, 0)), at(80, 30));
        assert_eq!(shoved.on_board(at(80, 30)), at(200, 0));
    }

    #[test]
    fn screen_and_board_coordinates_are_the_reverse_of_each_other() {
        // given: a panned and zoomed viewport
        let moved = Viewport::RESTING
            .panned(at(-60, 25))
            .zoomed(at(100, 100), 1.5);

        // then: any spot taken to the screen and back is where it was
        for spot in [at(0, 0), at(40, 90), at(-300, 1_200)] {
            assert_eq!(moved.on_board(moved.on_screen(spot)), spot, "{spot:?}");
        }
    }

    #[test]
    fn zooming_keeps_whatever_sits_under_the_pointer_under_the_pointer() {
        // given: a panned viewport and the board spot under the pointer
        let cursor = at(300, 180);
        let before = Viewport::RESTING.panned(at(-40, -20));
        let held = before.on_board(cursor);

        // when: it is zoomed in at the pointer
        let after = before.zoomed(cursor, 2.0);

        // then: the same spot is under the pointer
        assert_eq!(after.zoom, 2.0);
        assert_eq!(
            after.on_board(cursor),
            held,
            "the point you zoom at should not slide away from the pointer"
        );
    }

    #[test]
    fn zoom_stops_before_the_board_becomes_unusable() {
        // when: the viewport is zoomed out and in far too often
        let squashed = (0..20).fold(Viewport::RESTING, |it, _| it.zoomed(at(0, 0), 0.5));
        let magnified = (0..20).fold(Viewport::RESTING, |it, _| it.zoomed(at(0, 0), 2.0));

        // then: the zoom stops at the furthest and the closest
        assert_eq!(squashed.zoom, FURTHEST);
        assert_eq!(magnified.zoom, CLOSEST);
    }

    #[test]
    fn a_drag_across_the_screen_is_a_shorter_drag_across_a_zoomed_out_board() {
        // given: a viewport zoomed out to half
        let far = Viewport::RESTING.zoomed(at(0, 0), 0.5);

        // then: a drag across the screen covers twice as much board
        assert_eq!(far.across(at(100, 50)), at(200, 100));
    }

    #[test]
    fn a_card_is_drawn_at_the_height_the_zoom_gives_it() {
        // given: a viewport zoomed in to double
        let close = Viewport::RESTING.zoomed(at(0, 0), 2.0);

        // then: a card is drawn twice as tall
        assert_eq!(
            close.tall(Size {
                width: 168,
                height: 84
            }),
            168
        );
    }

    #[test]
    fn undoing_the_zoom_leaves_the_board_where_the_author_panned_it() {
        // given: a viewport panned away from home and the board spot in its middle
        let middle = at(300, 200);
        let wandered = Viewport::RESTING.panned(at(-450, -120));
        let looking = wandered.on_board(middle);

        // when: it is zoomed in and the zoom undone
        let back = wandered.zoomed(middle, 2.5).unzoomed(middle);

        // then: it is back at full size over the same spot, not at home
        assert_eq!(back.zoom, 1.0);
        assert_eq!(
            back.on_board(middle),
            looking,
            "resetting the zoom is not the same as going home"
        );
        assert_ne!(back.pan, Viewport::RESTING.pan);
    }

    #[test]
    fn zooming_in_and_back_out_leaves_the_board_exactly_where_it_was() {
        // given: a panned viewport
        let middle = at(311, 207);
        let wandered = Viewport::RESTING.panned(at(-137, -49));

        // when: it is zoomed in step by step and the zoom undone
        let round_trip = (0..6)
            .fold(wandered, |it, _| it.zoomed(middle, 1.1))
            .unzoomed(middle);

        // then: every spot is drawn to the pixel where it was
        assert_eq!(round_trip.zoom, wandered.zoom);
        for spot in [at(0, 0), at(400, 250), at(-90, 3_000)] {
            assert_eq!(
                round_trip.on_screen(spot),
                wandered.on_screen(spot),
                "a viewport that drifts a pixel per zoom creeps away over an afternoon"
            );
        }
    }

    #[test]
    fn the_zoom_reads_as_a_percentage() {
        // expect: full size reads 100% and half 50%
        assert_eq!(Viewport::RESTING.as_percent(), "100%");
        assert_eq!(Viewport::RESTING.zoomed(at(0, 0), 0.5).as_percent(), "50%");
    }

    #[test]
    fn fingers_spread_twice_as_far_apart_zoom_twice_as_close() {
        // when: two fingers spread to twice their gap
        let pinched =
            Viewport::RESTING.pinched((at(100, 200), at(200, 200)), (at(50, 200), at(250, 200)));

        // then: the zoom doubles
        assert_eq!(pinched.zoom, 2.0);
    }

    #[test]
    fn what_lies_between_the_fingers_stays_between_them() {
        // given: a panned viewport and the board spot between two fingers
        let start = Viewport::RESTING.panned(at(30, -20));
        let between = start.on_board(at(150, 200));

        // when: the fingers move and spread
        let pinched = start.pinched((at(100, 200), at(200, 200)), (at(80, 260), at(280, 260)));

        // then: the spot is between them still
        assert_eq!(
            pinched.on_screen(between),
            at(180, 260),
            "the spot under the fingers' middle follows the middle as they move and spread"
        );
    }

    #[test]
    fn fingers_moving_together_pan_without_zooming() {
        // when: two fingers move together without spreading
        let pinched =
            Viewport::RESTING.pinched((at(100, 200), at(200, 200)), (at(130, 160), at(230, 160)));

        // then: the board pans as far as they moved, its zoom unchanged
        assert_eq!(pinched.zoom, 1.0);
        assert_eq!(pinched.on_screen(at(0, 0)), at(30, -40));
    }

    #[test]
    fn a_pinch_stops_where_the_wheel_stops() {
        // when: two fingers spread a hundredfold
        let pinched =
            Viewport::RESTING.pinched((at(100, 200), at(110, 200)), (at(0, 200), at(1_000, 200)));

        // then: the zoom stops at the closest
        assert_eq!(pinched.zoom, CLOSEST);
    }

    #[test]
    fn fingers_on_one_spot_only_pan() {
        // when: two fingers that started on one spot move apart
        let pinched =
            Viewport::RESTING.pinched((at(100, 200), at(100, 200)), (at(120, 200), at(160, 200)));

        // then: the board pans without zooming
        assert_eq!(pinched.zoom, 1.0);
        assert!(
            pinched.on_screen(at(0, 0)).x.abs() < 1_000,
            "no division by a gap of nothing"
        );
    }
}
