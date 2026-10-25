use boards_core::{BoardCatalog, BoardError, BoardId, BoardService, BoardServiceError, IdeaLink};
use eventsourcing::{Agent, ServiceError};
use messaging::Failure;
use time::OffsetDateTime;

pub async fn let_go_everywhere(
    boards: &BoardService,
    catalog: &dyn BoardCatalog,
    gone: &IdeaLink,
    at: OffsetDateTime,
) -> Result<(), Failure> {
    catalog.lay_tombstone(gone, at).await?;

    for board in catalog.boards_holding(gone).await? {
        let_go(boards, board, gone).await?;
    }

    Ok(())
}

pub async fn let_go_of_what_is_gone(
    boards: &BoardService,
    catalog: &dyn BoardCatalog,
    board: BoardId,
) -> Result<(), Failure> {
    for gone in catalog.held_though_tombstoned(board).await? {
        let_go(boards, board, &gone).await?;
    }

    Ok(())
}

async fn let_go(boards: &BoardService, board: BoardId, gone: &IdeaLink) -> Result<(), Failure> {
    match boards
        .unpin(&board.to_string(), gone.clone(), None, &Agent::System)
        .await
    {
        Ok(_) => Ok(()),
        Err(refused) if nothing_left_to_unpin(&refused) => Ok(()),
        Err(refused) => Err(refused.into()),
    }
}

fn nothing_left_to_unpin(refused: &BoardServiceError) -> bool {
    matches!(
        refused,
        BoardServiceError::Events(ServiceError::Refused(
            BoardError::NotPinned | BoardError::Discarded
        ))
    )
}
