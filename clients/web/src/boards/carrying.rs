use crate::boards::model::{Placement, Size, Spot};
use crate::pieces::model::PieceId;

pub const GRID: i64 = 5;
const LEAP: i64 = 40;
const DRAG_BEGINS: i64 = 4;
const SMALLEST: Size = Size {
    width: 80,
    height: 40,
};

pub const EVERY_HANDLE: [Held; 8] = [
    Held::Top,
    Held::Bottom,
    Held::Left,
    Held::Right,
    Held::TopLeft,
    Held::TopRight,
    Held::BottomLeft,
    Held::BottomRight,
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Held {
    Whole,
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Carrying {
    pub piece: PieceId,
    pub held: Held,
    pub from: Placement,
    pub by: Spot,
}

impl Held {
    pub fn side(&self) -> &'static str {
        match self {
            Self::Whole => "whole",
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::Left => "left",
            Self::Right => "right",
            Self::TopLeft => "top-left",
            Self::TopRight => "top-right",
            Self::BottomLeft => "bottom-left",
            Self::BottomRight => "bottom-right",
        }
    }

    fn pulls_left(&self) -> bool {
        matches!(self, Self::Left | Self::TopLeft | Self::BottomLeft)
    }

    fn pulls_right(&self) -> bool {
        matches!(self, Self::Right | Self::TopRight | Self::BottomRight)
    }

    fn pulls_top(&self) -> bool {
        matches!(self, Self::Top | Self::TopLeft | Self::TopRight)
    }

    fn pulls_bottom(&self) -> bool {
        matches!(self, Self::Bottom | Self::BottomLeft | Self::BottomRight)
    }
}

impl Carrying {
    pub fn dragging(&self) -> bool {
        self.by.x.abs().max(self.by.y.abs()) >= DRAG_BEGINS
    }

    pub fn landing(&self) -> Placement {
        if !self.dragging() {
            return self.from;
        }

        if self.held == Held::Whole {
            return Placement {
                spot: snapped(Spot {
                    x: self.from.spot.x + self.by.x,
                    y: self.from.spot.y + self.by.y,
                }),
                size: self.from.size,
            };
        }

        let far = Spot {
            x: self.from.spot.x + self.from.size.width,
            y: self.from.spot.y + self.from.size.height,
        };
        let left = match self.held.pulls_left() {
            true => onto_grid(self.from.spot.x + self.by.x).min(far.x - SMALLEST.width),
            false => self.from.spot.x,
        };
        let top = match self.held.pulls_top() {
            true => onto_grid(self.from.spot.y + self.by.y).min(far.y - SMALLEST.height),
            false => self.from.spot.y,
        };
        let right = match self.held.pulls_right() {
            true => onto_grid(far.x + self.by.x).max(left + SMALLEST.width),
            false => far.x,
        };
        let bottom = match self.held.pulls_bottom() {
            true => onto_grid(far.y + self.by.y).max(top + SMALLEST.height),
            false => far.y,
        };

        Placement {
            spot: Spot { x: left, y: top },
            size: Size {
                width: right - left,
                height: bottom - top,
            },
        }
    }
}

pub fn nudge(key: &str, leaping: bool, from: Spot) -> Option<Spot> {
    let by = if leaping { LEAP } else { GRID };
    let (x, y) = match key {
        "ArrowLeft" => (-by, 0),
        "ArrowRight" => (by, 0),
        "ArrowUp" => (0, -by),
        "ArrowDown" => (0, by),
        _ => return None,
    };

    Some(snapped(Spot {
        x: from.x + x,
        y: from.y + y,
    }))
}

pub fn snapped(loose: Spot) -> Spot {
    Spot {
        x: onto_grid(loose.x),
        y: onto_grid(loose.y),
    }
}

fn onto_grid(loose: i64) -> i64 {
    (loose + GRID / 2).div_euclid(GRID) * GRID
}

#[cfg(test)]
mod tests {
    use super::*;

    const CARD: Placement = Placement {
        spot: Spot { x: 100, y: 100 },
        size: Size {
            width: 200,
            height: 100,
        },
    };

    fn a_piece() -> PieceId {
        PieceId::from("piece_1".to_owned())
    }

    fn carrying(held: Held, by: Spot) -> Carrying {
        Carrying {
            piece: a_piece(),
            held,
            from: CARD,
            by,
        }
    }

    fn at(x: i64, y: i64) -> Spot {
        Spot { x, y }
    }

    fn of(width: i64, height: i64) -> Size {
        Size { width, height }
    }

    #[test]
    fn a_slip_too_small_to_be_a_drag_leaves_the_card_alone() {
        for slip in [at(0, 0), at(3, 3), at(-3, 0), at(0, 3)] {
            assert_eq!(
                carrying(Held::Whole, slip).landing(),
                CARD,
                "{slip:?} is a wobble, not a drag"
            );
        }
    }

    #[test]
    fn a_drag_becomes_one_the_moment_it_passes_the_threshold() {
        assert!(!carrying(Held::Whole, at(3, 3)).dragging());
        assert!(carrying(Held::Whole, at(4, 0)).dragging());
        assert!(carrying(Held::Whole, at(0, -4)).dragging());
    }

    #[test]
    fn dragging_the_body_moves_the_card_without_reshaping_it() {
        let landed = carrying(Held::Whole, at(43, -27)).landing();

        assert_eq!(landed.spot, at(145, 75), "100+43 and 100-27, both snapped");
        assert_eq!(landed.size, CARD.size);
    }

    #[test]
    fn every_landing_sits_on_the_grid() {
        for held in EVERY_HANDLE {
            let landed = carrying(held, at(37, -23)).landing();

            assert_eq!(landed.spot.x % GRID, 0, "{} left", held.side());
            assert_eq!(landed.spot.y % GRID, 0, "{} top", held.side());
            assert_eq!(
                (landed.spot.x + landed.size.width) % GRID,
                0,
                "{} right",
                held.side()
            );
            assert_eq!(
                (landed.spot.y + landed.size.height) % GRID,
                0,
                "{} bottom",
                held.side()
            );
        }
    }

    #[test]
    fn pulling_a_side_moves_only_that_edge() {
        for (held, by, spot, size) in [
            (Held::Right, at(50, 0), CARD.spot, of(250, 100)),
            (Held::Bottom, at(0, 50), CARD.spot, of(200, 150)),
            (Held::Left, at(-50, 0), at(50, 100), of(250, 100)),
            (Held::Top, at(0, -50), at(100, 50), of(200, 150)),
        ] {
            let landed = carrying(held, by).landing();

            assert_eq!(landed.spot, spot, "{} spot", held.side());
            assert_eq!(landed.size, size, "{} size", held.side());
        }
    }

    #[test]
    fn pulling_one_side_leaves_the_opposite_one_where_it_was() {
        for (held, by) in [
            (Held::Right, at(70, 0)),
            (Held::Left, at(-70, 0)),
            (Held::Bottom, at(0, 70)),
            (Held::Top, at(0, -70)),
        ] {
            let landed = carrying(held, by).landing();
            let stayed = match held {
                Held::Left => landed.spot.x + landed.size.width == 300,
                Held::Right => landed.spot.x == 100,
                Held::Top => landed.spot.y + landed.size.height == 200,
                _ => landed.spot.y == 100,
            };

            assert!(
                stayed,
                "pulling the {} edge shifted the other one",
                held.side()
            );
        }
    }

    #[test]
    fn pulling_a_corner_moves_both_its_edges() {
        let landed = carrying(Held::TopLeft, at(-40, -20)).landing();

        assert_eq!(landed.spot, at(60, 80));
        assert_eq!(
            landed.size,
            of(240, 120),
            "the corner opposite the one you grabbed must stay put"
        );
    }

    #[test]
    fn a_card_cannot_be_pulled_smaller_than_it_is_allowed_to_be() {
        for held in [Held::Right, Held::Left, Held::Top, Held::Bottom] {
            let landed = carrying(held, at(-500, -500)).landing();

            assert!(
                landed.size.width >= SMALLEST.width && landed.size.height >= SMALLEST.height,
                "{} shrank to {:?}",
                held.side(),
                landed.size
            );
        }
    }

    #[test]
    fn shrinking_from_a_leading_edge_stops_without_running_past_the_far_one() {
        let landed = carrying(Held::Left, at(500, 0)).landing();

        assert_eq!(
            landed.spot.x + landed.size.width,
            300,
            "the right edge was never grabbed, so it must not move"
        );
        assert_eq!(landed.size.width, SMALLEST.width);
    }

    #[test]
    fn a_card_may_be_dragged_behind_the_origin() {
        let landed = carrying(Held::Whole, at(-195, -170)).landing();

        assert_eq!(
            landed.spot,
            at(-95, -70),
            "the board runs in every direction"
        );
    }

    #[test]
    fn the_arrow_keys_step_one_cell_and_leap_eight() {
        assert_eq!(nudge("ArrowRight", false, at(40, 40)), Some(at(45, 40)));
        assert_eq!(nudge("ArrowLeft", false, at(40, 40)), Some(at(35, 40)));
        assert_eq!(nudge("ArrowUp", false, at(40, 40)), Some(at(40, 35)));
        assert_eq!(nudge("ArrowDown", true, at(40, 40)), Some(at(40, 80)));
    }

    #[test]
    fn keys_that_are_not_arrows_move_nothing() {
        for key in ["Enter", "Escape", "a", "Tab"] {
            assert_eq!(nudge(key, false, at(40, 40)), None, "{key} is not a nudge");
        }
    }

    #[test]
    fn snapping_rounds_to_the_nearest_cell_in_both_directions() {
        assert_eq!(snapped(at(12, 13)), at(10, 15));
        assert_eq!(snapped(at(-12, -13)), at(-10, -15));
        assert_eq!(snapped(at(40, 40)), at(40, 40));
    }
}
