use appearances_contract::PlaceDTO;

use crate::outline::model::SectionId;
use crate::scenes::model::SceneId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    Scene(SceneId),
    Section(SectionId),
}

impl From<PlaceDTO> for Place {
    fn from(dto: PlaceDTO) -> Self {
        match dto {
            PlaceDTO::Scene { id } => Self::Scene(SceneId::from(id)),
            PlaceDTO::Section { id } => Self::Section(SectionId::from(id)),
        }
    }
}
