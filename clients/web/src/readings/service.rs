use gloo_net::http::Request;
use readings_contract::{ContentsDTO, ListedReadingCopyDTO, ReadingCopyDTO};

use crate::http;
use crate::http::ApiError;
use crate::projects::model::ProjectId;
use crate::readings::model::Book;
use crate::readings::reading_copies::{ListedReadingCopy, ReadingCopyId};

const LIVE: &str = "/api/readings/live";
const SUBJECT: &str = "work";
const COPIES: &str = "/api/readings/copies";
const COPY: &str = "copy";

pub async fn live_book(project: &ProjectId) -> Result<Book, ApiError> {
    let response = http::sent(Request::get(&format!("{LIVE}?project={project}"))).await?;
    let told: ContentsDTO = http::parsed(response, SUBJECT).await?;

    Ok(Book::from(told))
}

/// Every reading copy of a project, newest first.
pub async fn copies(project: &ProjectId) -> Result<Vec<ListedReadingCopy>, ApiError> {
    let response = http::sent(Request::get(&format!("{COPIES}?project={project}"))).await?;
    let listed: Vec<ListedReadingCopyDTO> = http::parsed(response, SUBJECT).await?;

    listed
        .into_iter()
        .map(ListedReadingCopy::try_from)
        .collect()
}

/// One reading copy, with the work as it was frozen.
pub async fn copy(copy: &ReadingCopyId) -> Result<(ListedReadingCopy, Book), ApiError> {
    let response = http::sent(Request::get(&format!("{COPIES}/{copy}"))).await?;
    let read: ReadingCopyDTO = http::parsed(response, COPY).await?;

    Ok((
        ListedReadingCopy::try_from(read.copy)?,
        Book::from(read.contents),
    ))
}
