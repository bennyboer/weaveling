use appearances_contract::PlaceDTO;
use gloo_net::http::Request;

use crate::appearances::model::Place;
use crate::http;
use crate::http::ApiError;
use crate::ideas::model::IdeaId;

const APPEARANCES: &str = "/api/appearances";
const SUBJECT: &str = "appearances";

pub async fn of(idea: &IdeaId) -> Result<Vec<Place>, ApiError> {
    let response = http::sent(Request::get(&format!("{APPEARANCES}?idea={idea}"))).await?;
    let places: Vec<PlaceDTO> = http::parsed(response, SUBJECT).await?;

    Ok(places.into_iter().map(Place::from).collect())
}
