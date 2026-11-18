use gloo_net::http::Request;
use readings_contract::ContentsDTO;

use crate::http;
use crate::http::ApiError;
use crate::projects::model::ProjectId;
use crate::readings::model::Book;

const LIVE: &str = "/api/readings/live";
const SUBJECT: &str = "work";

pub async fn live_book(project: &ProjectId) -> Result<Book, ApiError> {
    let response = http::sent(Request::get(&format!("{LIVE}?project={project}"))).await?;
    let told: ContentsDTO = http::parsed(response, SUBJECT).await?;

    Ok(Book::from(told))
}
