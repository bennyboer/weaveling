use crate::outline::model::SectionId;
use crate::passages::model::PassageId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    Passage(PassageId),
    Section(SectionId),
}
