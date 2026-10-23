use boards_contract::{
    BoardDTO, OpenBoardRequest, PinIdeaRequest, PositionedIdeaDTO, ReshapeIdeaRequest, SizeDTO,
    SpotDTO,
};
use gloo_net::http::Request;

use crate::boards::model::{Board, BoardId, Placement, PositionedIdea, Size, Spot};
use crate::http;
use crate::http::ApiError;
use crate::ideas::model::IdeaId;
use crate::projects::model::ProjectId;

const BOARDS: &str = "/api/boards";
const SUBJECT: &str = "board";

pub async fn open(project: &ProjectId) -> Result<Board, ApiError> {
    let payload = OpenBoardRequest {
        project: project.to_string(),
    };
    let response = http::sent_with(Request::post(BOARDS), &payload).await?;

    Ok(as_board(http::parsed(response, SUBJECT).await?))
}

pub async fn pin(board: &BoardId, idea: &IdeaId, at: Placement) -> Result<Board, ApiError> {
    let payload = PinIdeaRequest {
        idea: idea.to_string(),
        spot: to_spot_dto(at.spot),
        size: to_size_dto(at.size),
    };
    let response =
        http::sent_with(Request::post(&format!("{BOARDS}/{board}/ideas")), &payload).await?;

    Ok(as_board(http::parsed(response, SUBJECT).await?))
}

pub async fn reshape(
    board: &BoardId,
    idea: &IdeaId,
    to: Option<Spot>,
    size: Option<Size>,
) -> Result<Board, ApiError> {
    let payload = ReshapeIdeaRequest {
        spot: to.map(to_spot_dto),
        size: size.map(to_size_dto),
    };
    let response = http::sent_with(
        Request::patch(&format!("{BOARDS}/{board}/ideas/{idea}")),
        &payload,
    )
    .await?;

    Ok(as_board(http::parsed(response, SUBJECT).await?))
}

pub async fn unpin(board: &BoardId, idea: &IdeaId) -> Result<(), ApiError> {
    let response = http::sent(Request::delete(&format!("{BOARDS}/{board}/ideas/{idea}"))).await?;

    if response.ok() {
        return Ok(());
    }

    Err(ApiError::Unexpected)
}

fn as_board(dto: BoardDTO) -> Board {
    Board {
        id: BoardId::from(dto.id),
        version: dto.version,
        ideas: dto.ideas.iter().map(as_positioned).collect(),
    }
}

fn as_positioned(dto: &PositionedIdeaDTO) -> PositionedIdea {
    PositionedIdea {
        idea: IdeaId::from(dto.idea.clone()),
        spot: Spot {
            x: dto.spot.x,
            y: dto.spot.y,
        },
        size: Size {
            width: dto.size.width,
            height: dto.size.height,
        },
    }
}

fn to_spot_dto(at: Spot) -> SpotDTO {
    SpotDTO { x: at.x, y: at.y }
}

fn to_size_dto(size: Size) -> SizeDTO {
    SizeDTO {
        width: size.width,
        height: size.height,
    }
}
