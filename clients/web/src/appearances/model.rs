use crate::outline::model::SectionId;
use crate::scenes::model::SceneId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    Scene(SceneId),
    Section(SectionId),
}
