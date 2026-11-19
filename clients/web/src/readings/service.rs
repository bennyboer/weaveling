use gloo_net::http::Request;
use readings_contract::{
    ContentsDTO, ExtendReadingCopyRequest, ListedReadingCopyDTO, MakeReadingCopyRequest,
    ReadingCopyDTO, RenameReadingCopyRequest, SharedDTO,
};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

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

/// Makes a reading copy of the work as it is now, shared under a name.
pub async fn make_copy(
    project: &ProjectId,
    name: &str,
    shared: SharedDTO,
    open_until: Option<OffsetDateTime>,
    deleted_on: Option<OffsetDateTime>,
) -> Result<ListedReadingCopy, ApiError> {
    let payload = MakeReadingCopyRequest {
        project: project.to_string(),
        name: name.to_owned(),
        shared,
        open_until: open_until.map(written),
        deleted_on: deleted_on.map(written),
    };
    let response = http::sent_with(Request::post(COPIES), &payload).await?;
    let made: ListedReadingCopyDTO = http::parsed(response, COPY).await?;

    ListedReadingCopy::try_from(made)
}

fn written(moment: OffsetDateTime) -> String {
    moment
        .format(&Rfc3339)
        .expect("a moment chosen in the dialog is always representable as RFC 3339")
}

/// Renames a reading copy.
pub async fn rename_copy(copy: &ReadingCopyId, name: &str) -> Result<ListedReadingCopy, ApiError> {
    let payload = RenameReadingCopyRequest {
        name: name.to_owned(),
    };
    let response =
        http::sent_with(Request::put(&format!("{COPIES}/{copy}/name")), &payload).await?;
    let renamed: ListedReadingCopyDTO = http::parsed(response, COPY).await?;

    ListedReadingCopy::try_from(renamed)
}

/// Deletes a reading copy for good, with its feedback.
pub async fn delete_copy(copy: &ReadingCopyId) -> Result<(), ApiError> {
    let response = http::sent(Request::delete(&format!("{COPIES}/{copy}"))).await?;

    http::checked(response, COPY).await?;

    Ok(())
}

/// Moves a reading copy's times later.
pub async fn extend_copy(
    copy: &ReadingCopyId,
    open_until: Option<OffsetDateTime>,
    deleted_on: Option<OffsetDateTime>,
) -> Result<ListedReadingCopy, ApiError> {
    let payload = ExtendReadingCopyRequest {
        open_until: open_until.map(written),
        deleted_on: deleted_on.map(written),
    };
    let response =
        http::sent_with(Request::put(&format!("{COPIES}/{copy}/times")), &payload).await?;
    let extended: ListedReadingCopyDTO = http::parsed(response, COPY).await?;

    ListedReadingCopy::try_from(extended)
}
