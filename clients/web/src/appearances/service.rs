use appearances_contract::PlaceDTO;
use gloo_net::http::Request;

use crate::appearances::model::Place;
use crate::http::{ApiError, parsed};
use crate::ideas::model::IdeaId;
use crate::outline::model::SectionId;
use crate::passages::model::PassageId;

const APPEARANCES: &str = "/api/appearances";
const SUBJECT: &str = "appearances";

pub async fn of(idea: &IdeaId) -> Result<Vec<Place>, ApiError> {
    let response = Request::get(&format!("{APPEARANCES}?idea={idea}"))
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;
    let places: Vec<PlaceDTO> = parsed(response, SUBJECT).await?;

    Ok(places.into_iter().map(as_place).collect())
}

fn as_place(dto: PlaceDTO) -> Place {
    match dto {
        PlaceDTO::Passage { id } => Place::Passage(PassageId::from(id)),
        PlaceDTO::Section { id } => Place::Section(SectionId::from(id)),
    }
}
