use boards_contract::{BoardDTO, PositionedIdeaDTO, SizeDTO, SpotDTO};
use boards_core::{Board, PositionedIdea};
use eventsourcing::Version;

pub(crate) fn to_dto(board: &str, held: &Board, version: Version) -> BoardDTO {
    BoardDTO {
        id: board.to_owned(),
        version: version.count(),
        project: held.project().to_string(),
        ideas: held.ideas().iter().map(to_positioned_dto).collect(),
    }
}

fn to_positioned_dto(held: &PositionedIdea) -> PositionedIdeaDTO {
    PositionedIdeaDTO {
        idea: held.idea.to_string(),
        spot: SpotDTO {
            x: held.spot.x,
            y: held.spot.y,
        },
        size: SizeDTO {
            width: held.size.width,
            height: held.size.height,
        },
    }
}
