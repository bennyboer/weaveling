use appearances_contract::PlaceDTO;
use appearances_core::Place;

pub(crate) fn to_dto(place: &Place) -> PlaceDTO {
    match place {
        Place::Scene(scene) => PlaceDTO::Scene {
            id: scene.to_string(),
        },
        Place::Section(section) => PlaceDTO::Section {
            id: section.to_string(),
        },
    }
}
