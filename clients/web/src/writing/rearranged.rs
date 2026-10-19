use std::collections::HashMap;

use crate::outline::model::{Outline, SectionId};
use crate::scenes::model::SceneId;

pub struct Placement {
    parent: Option<SectionId>,
    siblings: Vec<SectionId>,
}

pub fn placements(book: &Outline) -> HashMap<SectionId, Placement> {
    let read = book.in_reading_order();
    let mut under: HashMap<Option<SectionId>, Vec<SectionId>> = HashMap::new();
    for (_, section) in &read {
        under
            .entry(section.parent.clone())
            .or_default()
            .push(section.id.clone());
    }

    read.into_iter()
        .map(|(_, section)| {
            let siblings = under.get(&section.parent).cloned().unwrap_or_default();

            (
                section.id.clone(),
                Placement {
                    parent: section.parent.clone(),
                    siblings,
                },
            )
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Removed {
    pub section: SectionId,
    pub title: String,
    pub scenes: Vec<SceneId>,
}

pub fn removed(before: &Outline, after: &Outline) -> Vec<Removed> {
    before
        .sections
        .iter()
        .filter(|was| after.sections.iter().all(|now| now.id != was.id))
        .map(|was| Removed {
            section: was.id.clone(),
            title: was.title.clone(),
            scenes: was.scenes(),
        })
        .collect()
}

pub fn rearranged(
    before: &HashMap<SectionId, Placement>,
    after: &HashMap<SectionId, Placement>,
) -> Vec<SectionId> {
    after
        .iter()
        .filter(|(id, now)| {
            let Some(was) = before.get(*id) else {
                return false;
            };
            if was.parent != now.parent {
                return true;
            }

            rank_among(id, &was.siblings, &now.siblings)
                != rank_among(id, &now.siblings, &was.siblings)
        })
        .map(|(id, _)| id.clone())
        .collect()
}

fn rank_among(id: &SectionId, these: &[SectionId], kept_in: &[SectionId]) -> Option<usize> {
    these
        .iter()
        .filter(|sibling| kept_in.contains(sibling))
        .position(|sibling| sibling == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outline::model::{OutlineId, Section};

    fn section(id: &str, parent: Option<&str>) -> Section {
        Section {
            id: SectionId::from(id.to_owned()),
            parent: parent.map(|parent| SectionId::from(parent.to_owned())),
            title: id.to_owned(),
            attachments: Vec::new(),
        }
    }

    fn book(sections: Vec<Section>) -> HashMap<SectionId, Placement> {
        placements(&Outline {
            id: OutlineId::from("outline_1".to_owned()),
            version: 1,
            sections,
        })
    }

    fn named(found: Vec<SectionId>) -> Vec<String> {
        let mut found: Vec<String> = found.into_iter().map(|id| id.to_string()).collect();
        found.sort();
        found
    }

    #[test]
    fn a_section_gone_from_the_book_is_removed_with_its_title_and_scenes() {
        let mut part = section("two", None);
        part.title = "Part Two".to_owned();
        part.attachments = vec![crate::outline::model::Attachment::Scene(SceneId::from(
            "scene_1".to_owned(),
        ))];
        let before = Outline {
            id: OutlineId::from("outline_1".to_owned()),
            version: 1,
            sections: vec![section("one", None), part],
        };
        let after = Outline {
            sections: vec![section("one", None)],
            ..before.clone()
        };

        assert_eq!(
            removed(&before, &after),
            vec![Removed {
                section: SectionId::from("two".to_owned()),
                title: "Part Two".to_owned(),
                scenes: vec![SceneId::from("scene_1".to_owned())],
            }]
        );
        assert!(removed(&before, &before).is_empty());
    }

    #[test]
    fn a_section_nested_under_another_is_rearranged() {
        let before = book(vec![section("one", None), section("two", None)]);
        let after = book(vec![section("one", None), section("two", Some("one"))]);

        assert_eq!(named(rearranged(&before, &after)), ["two"]);
    }

    #[test]
    fn two_sections_trading_places_are_both_rearranged() {
        let before = book(vec![section("one", None), section("two", None)]);
        let after = book(vec![section("two", None), section("one", None)]);

        assert_eq!(named(rearranged(&before, &after)), ["one", "two"]);
    }

    #[test]
    fn a_section_added_or_removed_beside_mine_rearranges_nothing() {
        let before = book(vec![section("one", None), section("two", None)]);
        let added = book(vec![
            section("new", None),
            section("one", None),
            section("two", None),
        ]);
        let removed = book(vec![section("two", None)]);

        assert!(rearranged(&before, &added).is_empty());
        assert!(rearranged(&before, &removed).is_empty());
    }
}
