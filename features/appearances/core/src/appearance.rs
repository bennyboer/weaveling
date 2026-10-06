use crate::link::{IdeaLink, PassageLink, SectionLink};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Subject {
    Idea(IdeaLink),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Place {
    Passage(PassageLink),
    Section(SectionLink),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_of_different_types_never_coincide() {
        assert_ne!(
            Place::Section(SectionLink::from("same_1")),
            Place::Passage(PassageLink::from("same_1")),
            "the type is part of the identity, or forgetting a passage could take a section with it"
        );
    }
}
