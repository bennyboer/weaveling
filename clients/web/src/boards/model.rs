use std::fmt::{self, Display, Formatter};

use boards_contract::{BoardEventDTO, SizeDTO, SpotDTO};

use crate::ideas::model::IdeaId;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BoardId(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spot {
    pub x: i64,
    pub y: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub width: i64,
    pub height: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub spot: Spot,
    pub size: Size,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PositionedIdea {
    pub idea: IdeaId,
    pub spot: Spot,
    pub size: Size,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Board {
    pub id: BoardId,
    pub version: u64,
    pub ideas: Vec<PositionedIdea>,
}

impl From<String> for BoardId {
    fn from(given: String) -> Self {
        Self(given)
    }
}

impl Display for BoardId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Board {
    pub fn holds(&self, idea: &IdeaId) -> bool {
        self.ideas.iter().any(|held| &held.idea == idea)
    }

    pub fn taken_in(mut self, event: BoardEventDTO) -> Option<Self> {
        match event {
            BoardEventDTO::IdeaPinned { idea, at, size } => {
                let idea = IdeaId::from(idea);
                if self.holds(&idea) {
                    return None;
                }
                self.ideas.push(PositionedIdea {
                    idea,
                    spot: spot_of(at),
                    size: size_of(size),
                });
            }
            BoardEventDTO::IdeaMoved { idea, to } => self.held_mut(idea)?.spot = spot_of(to),
            BoardEventDTO::IdeaResized { idea, to } => self.held_mut(idea)?.size = size_of(to),
            BoardEventDTO::IdeaRaised { idea } => {
                let at = self.place_of(idea)?;
                let raised = self.ideas.remove(at);
                self.ideas.push(raised);
            }
            BoardEventDTO::IdeaUnpinned { idea } => {
                let at = self.place_of(idea)?;
                self.ideas.remove(at);
            }
            BoardEventDTO::Started { .. } | BoardEventDTO::Discarded => return None,
        }
        self.version += 1;

        Some(self)
    }

    fn place_of(&self, idea: String) -> Option<usize> {
        let idea = IdeaId::from(idea);

        self.ideas.iter().position(|held| held.idea == idea)
    }

    fn held_mut(&mut self, idea: String) -> Option<&mut PositionedIdea> {
        let at = self.place_of(idea)?;

        self.ideas.get_mut(at)
    }
}

fn spot_of(dto: SpotDTO) -> Spot {
    Spot { x: dto.x, y: dto.y }
}

fn size_of(dto: SizeDTO) -> Size {
    Size {
        width: dto.width,
        height: dto.height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placed(idea: &str, x: i64) -> PositionedIdea {
        PositionedIdea {
            idea: IdeaId::from(idea.to_owned()),
            spot: Spot { x, y: 0 },
            size: Size {
                width: 168,
                height: 84,
            },
        }
    }

    fn a_board(ideas: Vec<PositionedIdea>) -> Board {
        Board {
            id: BoardId::from("board_1".to_owned()),
            version: 4,
            ideas,
        }
    }

    fn order(board: &Board) -> Vec<String> {
        board
            .ideas
            .iter()
            .map(|held| held.idea.to_string())
            .collect()
    }

    #[test]
    fn a_pinned_idea_joins_the_board_on_top_and_the_version_moves_on() {
        let board = a_board(vec![placed("idea_a", 0)])
            .taken_in(BoardEventDTO::IdeaPinned {
                idea: "idea_b".to_owned(),
                at: SpotDTO { x: 40, y: 80 },
                size: SizeDTO {
                    width: 200,
                    height: 100,
                },
            })
            .expect("a new idea fits");

        assert_eq!(order(&board), vec!["idea_a", "idea_b"]);
        assert_eq!(board.ideas[1].spot, Spot { x: 40, y: 80 });
        assert_eq!(
            board.ideas[1].size,
            Size {
                width: 200,
                height: 100
            }
        );
        assert_eq!(board.version, 5);
    }

    #[test]
    fn an_idea_already_on_the_board_pinned_again_does_not_fit() {
        let taken = a_board(vec![placed("idea_a", 0)]).taken_in(BoardEventDTO::IdeaPinned {
            idea: "idea_a".to_owned(),
            at: SpotDTO { x: 40, y: 80 },
            size: SizeDTO {
                width: 200,
                height: 100,
            },
        });

        assert_eq!(taken, None);
    }

    #[test]
    fn a_moved_or_resized_idea_keeps_the_rest_of_its_place() {
        let board = a_board(vec![placed("idea_a", 0), placed("idea_b", 10)])
            .taken_in(BoardEventDTO::IdeaMoved {
                idea: "idea_a".to_owned(),
                to: SpotDTO { x: 300, y: 120 },
            })
            .and_then(|board| {
                board.taken_in(BoardEventDTO::IdeaResized {
                    idea: "idea_b".to_owned(),
                    to: SizeDTO {
                        width: 220,
                        height: 90,
                    },
                })
            })
            .expect("both ideas are there");

        assert_eq!(board.ideas[0].spot, Spot { x: 300, y: 120 });
        assert_eq!(board.ideas[0].size, placed("idea_a", 0).size);
        assert_eq!(
            board.ideas[1].size,
            Size {
                width: 220,
                height: 90
            }
        );
        assert_eq!(board.ideas[1].spot, Spot { x: 10, y: 0 });
        assert_eq!(board.version, 6);
    }

    #[test]
    fn a_raised_idea_goes_on_top_and_an_unpinned_one_leaves() {
        let board = a_board(vec![
            placed("idea_a", 0),
            placed("idea_b", 10),
            placed("idea_c", 20),
        ])
        .taken_in(BoardEventDTO::IdeaRaised {
            idea: "idea_a".to_owned(),
        })
        .and_then(|board| {
            board.taken_in(BoardEventDTO::IdeaUnpinned {
                idea: "idea_b".to_owned(),
            })
        })
        .expect("both ideas are there");

        assert_eq!(order(&board), vec!["idea_c", "idea_a"]);
    }

    #[test]
    fn a_change_to_an_idea_not_on_the_board_does_not_fit() {
        let absent = || "idea_z".to_owned();
        let board = || a_board(vec![placed("idea_a", 0)]);

        assert_eq!(
            board().taken_in(BoardEventDTO::IdeaMoved {
                idea: absent(),
                to: SpotDTO { x: 1, y: 1 },
            }),
            None
        );
        assert_eq!(
            board().taken_in(BoardEventDTO::IdeaResized {
                idea: absent(),
                to: SizeDTO {
                    width: 1,
                    height: 1,
                },
            }),
            None
        );
        assert_eq!(
            board().taken_in(BoardEventDTO::IdeaRaised { idea: absent() }),
            None
        );
        assert_eq!(
            board().taken_in(BoardEventDTO::IdeaUnpinned { idea: absent() }),
            None
        );
    }

    #[test]
    fn a_board_started_again_or_discarded_is_opened_again_rather_than_guessed() {
        assert_eq!(
            a_board(Vec::new()).taken_in(BoardEventDTO::Started {
                project: "project_1".to_owned(),
            }),
            None
        );
        assert_eq!(a_board(Vec::new()).taken_in(BoardEventDTO::Discarded), None);
    }
}
