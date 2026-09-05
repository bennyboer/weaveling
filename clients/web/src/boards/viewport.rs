use crate::boards::model::{Size, Spot};

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

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: i64, y: i64) -> Spot {
        Spot { x, y }
    }

    #[test]
    fn a_resting_viewport_leaves_the_board_where_it_is() {
        assert_eq!(Viewport::RESTING.on_screen(at(40, 90)), at(40, 90));
        assert_eq!(Viewport::RESTING.on_board(at(40, 90)), at(40, 90));
    }

    #[test]
    fn panning_shifts_the_board_under_the_window() {
        let shoved = Viewport::RESTING.panned(at(-120, 30));

        assert_eq!(shoved.on_screen(at(200, 0)), at(80, 30));
        assert_eq!(shoved.on_board(at(80, 30)), at(200, 0));
    }

    #[test]
    fn screen_and_board_coordinates_are_the_reverse_of_each_other() {
        let moved = Viewport::RESTING
            .panned(at(-60, 25))
            .zoomed(at(100, 100), 1.5);

        for spot in [at(0, 0), at(40, 90), at(-300, 1_200)] {
            assert_eq!(moved.on_board(moved.on_screen(spot)), spot, "{spot:?}");
        }
    }

    #[test]
    fn zooming_keeps_whatever_sits_under_the_pointer_under_the_pointer() {
        let cursor = at(300, 180);
        let before = Viewport::RESTING.panned(at(-40, -20));
        let held = before.on_board(cursor);

        let after = before.zoomed(cursor, 2.0);

        assert_eq!(after.zoom, 2.0);
        assert_eq!(
            after.on_board(cursor),
            held,
            "the point you zoom at should not slide away from the pointer"
        );
    }

    #[test]
    fn zoom_stops_before_the_board_becomes_unusable() {
        let squashed = (0..20).fold(Viewport::RESTING, |it, _| it.zoomed(at(0, 0), 0.5));
        let magnified = (0..20).fold(Viewport::RESTING, |it, _| it.zoomed(at(0, 0), 2.0));

        assert_eq!(squashed.zoom, FURTHEST);
        assert_eq!(magnified.zoom, CLOSEST);
    }

    #[test]
    fn a_drag_across_the_screen_is_a_shorter_drag_across_a_zoomed_out_board() {
        let far = Viewport::RESTING.zoomed(at(0, 0), 0.5);

        assert_eq!(far.across(at(100, 50)), at(200, 100));
    }

    #[test]
    fn a_card_is_drawn_at_the_height_the_zoom_gives_it() {
        let close = Viewport::RESTING.zoomed(at(0, 0), 2.0);

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
        let middle = at(300, 200);
        let wandered = Viewport::RESTING.panned(at(-450, -120));
        let looking = wandered.on_board(middle);

        let back = wandered.zoomed(middle, 2.5).unzoomed(middle);

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
        let middle = at(311, 207);
        let wandered = Viewport::RESTING.panned(at(-137, -49));

        let round_trip = (0..6)
            .fold(wandered, |it, _| it.zoomed(middle, 1.1))
            .unzoomed(middle);

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
        assert_eq!(Viewport::RESTING.as_percent(), "100%");
        assert_eq!(Viewport::RESTING.zoomed(at(0, 0), 0.5).as_percent(), "50%");
    }
}
