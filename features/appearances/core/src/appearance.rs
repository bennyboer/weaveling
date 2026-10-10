use crate::link::{IdeaLink, SceneLink, SectionLink};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Subject {
    Idea(IdeaLink),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Place {
    Scene(SceneLink),
    Section(SectionLink),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_of_different_types_never_coincide() {
        assert_ne!(
            Place::Section(SectionLink::from("same_1")),
            Place::Scene(SceneLink::from("same_1")),
            "the type is part of the identity, or forgetting a scene could take a section with it"
        );
    }
}
