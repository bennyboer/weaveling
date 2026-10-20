use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};

use outline_contract::{AttachmentDTO, OutlineEventDTO};

use crate::ideas::model::IdeaId;
use crate::scenes::model::SceneId;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OutlineId(String);

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SectionId(String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Attachment {
    Scene(SceneId),
    Idea(IdeaId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub id: SectionId,
    pub parent: Option<SectionId>,
    pub title: String,
    pub attachments: Vec<Attachment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outline {
    pub id: OutlineId,
    pub version: u64,
    pub sections: Vec<Section>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectionChange {
    pub id: SectionId,
    pub parent: Option<SectionId>,
    pub title: String,
    pub attachments: Option<Vec<Attachment>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutlineChange {
    pub id: OutlineId,
    pub version: u64,
    pub sections: Vec<SectionChange>,
}

impl From<String> for OutlineId {
    fn from(given: String) -> Self {
        Self(given)
    }
}

impl Display for OutlineId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl From<String> for SectionId {
    fn from(given: String) -> Self {
        Self(given)
    }
}

impl Display for SectionId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Section {
    pub fn ideas(&self) -> Vec<IdeaId> {
        self.attachments
            .iter()
            .filter_map(|attachment| match attachment {
                Attachment::Idea(idea) => Some(idea.clone()),
                Attachment::Scene(_) => None,
            })
            .collect()
    }

    pub fn scenes(&self) -> Vec<SceneId> {
        self.attachments
            .iter()
            .filter_map(|attachment| match attachment {
                Attachment::Scene(scene) => Some(scene.clone()),
                Attachment::Idea(_) => None,
            })
            .collect()
    }
}

impl Outline {
    pub fn taken_in(self, event: OutlineEventDTO) -> Option<Self> {
        let mut tree = Tree::of(&self.sections);
        let fitted = match event {
            OutlineEventDTO::SectionAdded {
                section,
                under,
                after,
                title,
            } => tree.add(
                section.into(),
                under.map(Into::into),
                after.map(Into::into),
                title,
            ),
            OutlineEventDTO::SectionRetitled { section, title } => {
                tree.retitle(&section.into(), title)
            }
            OutlineEventDTO::SectionMoved {
                section,
                under,
                after,
            } => tree.moved(section.into(), under.map(Into::into), after.map(Into::into)),
            OutlineEventDTO::SectionPromoted { section } => tree.promote(section.into()),
            OutlineEventDTO::SectionDemoted { section } => tree.demote(section.into()),
            OutlineEventDTO::SectionRemoved { section } => tree.remove(section.into()),
            OutlineEventDTO::Attached {
                attachment,
                to,
                after,
            } => tree.attach(attached(attachment), &to.into(), after.map(attached)),
            OutlineEventDTO::Detached { attachment, .. } => tree.detach(&attached(attachment)),
            OutlineEventDTO::Started { .. } | OutlineEventDTO::Discarded => None,
        };

        fitted.map(|()| Self {
            id: self.id,
            version: self.version + 1,
            sections: tree.walked(),
        })
    }

    pub fn changed_by(&self, change: OutlineChange) -> Self {
        Self {
            id: change.id,
            version: change.version,
            sections: change
                .sections
                .into_iter()
                .map(|changed| Section {
                    attachments: changed.attachments.unwrap_or_else(|| {
                        self.sections
                            .iter()
                            .find(|section| section.id == changed.id)
                            .map(|section| section.attachments.clone())
                            .unwrap_or_default()
                    }),
                    id: changed.id,
                    parent: changed.parent,
                    title: changed.title,
                })
                .collect(),
        }
    }

    pub fn holds(&self, attachment: &Attachment) -> bool {
        self.sections
            .iter()
            .any(|section| section.attachments.contains(attachment))
    }

    pub fn in_reading_order(&self) -> Vec<(usize, &Section)> {
        let mut read = Vec::with_capacity(self.sections.len());
        self.read_under(None, 0, &mut read);

        read
    }

    fn read_under<'a>(
        &'a self,
        parent: Option<&SectionId>,
        depth: usize,
        read: &mut Vec<(usize, &'a Section)>,
    ) {
        for section in self
            .sections
            .iter()
            .filter(|held| held.parent.as_ref() == parent)
        {
            read.push((depth, section));
            self.read_under(Some(&section.id), depth + 1, read);
        }
    }

    pub fn siblings_of(&self, section: &SectionId) -> Vec<SectionId> {
        let parent = self.parent_of(section);

        self.sections
            .iter()
            .filter(|held| held.parent == parent)
            .map(|held| held.id.clone())
            .collect()
    }

    pub fn rank_of(&self, section: &SectionId) -> Option<usize> {
        self.siblings_of(section)
            .iter()
            .position(|held| held == section)
    }

    pub fn descends_from(&self, section: &SectionId, ancestor: &SectionId) -> bool {
        let mut walking = Some(section.clone());

        while let Some(here) = walking {
            if &here == ancestor {
                return true;
            }

            walking = self.parent_of(&here);
        }

        false
    }

    pub fn last_child_of(&self, section: &SectionId) -> Option<SectionId> {
        self.sections
            .iter()
            .rfind(|held| held.parent.as_ref() == Some(section))
            .map(|held| held.id.clone())
    }

    pub fn before(&self, section: &SectionId) -> Option<SectionId> {
        let siblings = self.siblings_of(section);
        let rank = siblings.iter().position(|held| held == section)?;

        rank.checked_sub(1)
            .and_then(|back| siblings.get(back).cloned())
    }

    pub fn can_promote(&self, section: &SectionId) -> bool {
        self.parent_of(section).is_some()
    }

    pub fn can_demote(&self, section: &SectionId) -> bool {
        self.rank_of(section).is_some_and(|rank| rank > 0)
    }

    pub fn can_move_earlier(&self, section: &SectionId) -> bool {
        self.can_demote(section)
    }

    pub fn can_move_later(&self, section: &SectionId) -> bool {
        let siblings = self.siblings_of(section);

        siblings
            .iter()
            .position(|held| held == section)
            .is_some_and(|rank| rank + 1 < siblings.len())
    }

    pub fn landing_for(&self, section: &SectionId, later: bool) -> Option<SectionId> {
        let siblings = self.siblings_of(section);
        let rank = siblings.iter().position(|held| held == section)?;

        match later {
            true => siblings.get(rank + 1).cloned(),
            false => rank
                .checked_sub(2)
                .and_then(|before| siblings.get(before).cloned()),
        }
    }

    pub fn parent_of(&self, section: &SectionId) -> Option<SectionId> {
        self.sections
            .iter()
            .find(|held| &held.id == section)
            .and_then(|held| held.parent.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_section(id: &str, parent: Option<&str>) -> Section {
        Section {
            id: SectionId::from(id.to_owned()),
            parent: parent.map(|parent| SectionId::from(parent.to_owned())),
            title: id.to_owned(),
            attachments: Vec::new(),
        }
    }

    fn an_outline(sections: Vec<Section>) -> Outline {
        Outline {
            id: OutlineId::from("outline_1".to_owned()),
            version: 1,
            sections,
        }
    }

    fn read(outline: &Outline) -> Vec<(usize, &str)> {
        outline
            .in_reading_order()
            .into_iter()
            .map(|(depth, section)| (depth, section.title.as_str()))
            .collect()
    }

    fn a_scene(id: &str) -> Attachment {
        Attachment::Scene(SceneId::from(id.to_owned()))
    }

    fn changed(id: &str, attachments: Option<Vec<Attachment>>) -> SectionChange {
        SectionChange {
            id: SectionId::from(id.to_owned()),
            parent: None,
            title: id.to_owned(),
            attachments,
        }
    }

    #[test]
    fn a_change_takes_the_scenes_it_sends_and_keeps_the_scenes_it_does_not() {
        let mut kept = a_section("kept", None);
        kept.attachments = vec![a_scene("scene_1")];
        let mut moved = a_section("moved", None);
        moved.attachments = vec![a_scene("scene_2")];
        let held = an_outline(vec![kept, moved, a_section("removed", None)]);

        let now = held.changed_by(OutlineChange {
            id: OutlineId::from("outline_1".to_owned()),
            version: 7,
            sections: vec![
                changed("new", None),
                changed("moved", Some(vec![a_scene("scene_2"), a_scene("scene_3")])),
                changed("kept", None),
            ],
        });

        assert_eq!(now.version, 7);
        assert_eq!(
            now.sections
                .iter()
                .map(|section| (section.title.as_str(), section.attachments.clone()))
                .collect::<Vec<_>>(),
            vec![
                ("new", Vec::new()),
                ("moved", vec![a_scene("scene_2"), a_scene("scene_3")]),
                ("kept", vec![a_scene("scene_1")]),
            ],
            "the order and the sections are the server's, the scenes of a section it did not send are ours"
        );
    }

    #[test]
    fn a_book_is_read_depth_first_with_each_section_before_its_children() {
        let outline = an_outline(vec![
            a_section("part_1", None),
            a_section("part_2", None),
            a_section("chapter_1", Some("part_1")),
            a_section("scene_a", Some("chapter_1")),
            a_section("chapter_2", Some("part_1")),
            a_section("chapter_3", Some("part_2")),
        ]);

        assert_eq!(
            read(&outline),
            vec![
                (0, "part_1"),
                (1, "chapter_1"),
                (2, "scene_a"),
                (1, "chapter_2"),
                (0, "part_2"),
                (1, "chapter_3"),
            ],
            "the outline keeps sections flat, so a child stored after the next part is still read inside its own"
        );
    }

    #[test]
    fn an_empty_book_has_nothing_to_read() {
        assert!(an_outline(Vec::new()).in_reading_order().is_empty());
    }
}

fn attached(dto: AttachmentDTO) -> Attachment {
    match dto {
        AttachmentDTO::Scene { id } => Attachment::Scene(SceneId::from(id)),
        AttachmentDTO::Idea { id } => Attachment::Idea(IdeaId::from(id)),
    }
}

struct Held {
    parent: Option<SectionId>,
    title: String,
    children: Vec<SectionId>,
    attachments: Vec<Attachment>,
}

struct Tree {
    sections: HashMap<SectionId, Held>,
    children: Vec<SectionId>,
}

impl Tree {
    fn of(sections: &[Section]) -> Self {
        let mut tree = Self {
            sections: sections
                .iter()
                .map(|section| {
                    (
                        section.id.clone(),
                        Held {
                            parent: section.parent.clone(),
                            title: section.title.clone(),
                            children: Vec::new(),
                            attachments: section.attachments.clone(),
                        },
                    )
                })
                .collect(),
            children: Vec::new(),
        };
        for section in sections {
            tree.siblings_mut(section.parent.as_ref())
                .push(section.id.clone());
        }

        tree
    }

    fn walked(&self) -> Vec<Section> {
        let mut walked = Vec::with_capacity(self.sections.len());
        for child in &self.children {
            self.walk(child, &mut walked);
        }

        walked
    }

    fn walk(&self, section: &SectionId, into: &mut Vec<Section>) {
        let Some(held) = self.sections.get(section) else {
            return;
        };
        into.push(Section {
            id: section.clone(),
            parent: held.parent.clone(),
            title: held.title.clone(),
            attachments: held.attachments.clone(),
        });
        for child in &held.children {
            self.walk(child, into);
        }
    }

    fn holds(&self, section: &SectionId) -> bool {
        self.sections.contains_key(section)
    }

    fn parent_of(&self, section: &SectionId) -> Option<SectionId> {
        self.sections
            .get(section)
            .and_then(|held| held.parent.clone())
    }

    fn siblings_of(&self, parent: Option<&SectionId>) -> &[SectionId] {
        match parent {
            None => &self.children,
            Some(parent) => self
                .sections
                .get(parent)
                .map(|held| held.children.as_slice())
                .unwrap_or_default(),
        }
    }

    fn siblings_mut(&mut self, parent: Option<&SectionId>) -> &mut Vec<SectionId> {
        match parent {
            Some(parent) if self.sections.contains_key(parent) => {
                &mut self
                    .sections
                    .get_mut(parent)
                    .expect("a section just found is still there")
                    .children
            }
            _ => &mut self.children,
        }
    }

    fn rank_of(&self, section: &SectionId) -> Option<usize> {
        self.siblings_of(self.parent_of(section).as_ref())
            .iter()
            .position(|held| held == section)
    }

    fn descends_from(&self, section: &SectionId, ancestor: &SectionId) -> bool {
        let mut walking = Some(section.clone());
        while let Some(here) = walking {
            if &here == ancestor {
                return true;
            }
            walking = self.parent_of(&here);
        }

        false
    }

    fn knows_where(&self, under: Option<&SectionId>, after: Option<&SectionId>) -> bool {
        under.is_none_or(|under| self.holds(under))
            && after.is_none_or(|after| self.siblings_of(under).contains(after))
    }

    fn lift(&mut self, section: &SectionId) {
        let parent = self.parent_of(section);
        self.siblings_mut(parent.as_ref())
            .retain(|held| held != section);
    }

    fn put(&mut self, section: SectionId, under: Option<SectionId>, after: Option<&SectionId>) {
        self.lift(&section);
        if let Some(held) = self.sections.get_mut(&section) {
            held.parent = under.clone();
        }
        let siblings = self.siblings_mut(under.as_ref());
        let at = after
            .and_then(|after| siblings.iter().position(|held| held == after))
            .map_or(0, |nth| nth + 1);
        siblings.insert(at.min(siblings.len()), section);
    }

    fn add(
        &mut self,
        section: SectionId,
        under: Option<SectionId>,
        after: Option<SectionId>,
        title: String,
    ) -> Option<()> {
        if self.holds(&section) || !self.knows_where(under.as_ref(), after.as_ref()) {
            return None;
        }
        self.sections.insert(
            section.clone(),
            Held {
                parent: None,
                title,
                children: Vec::new(),
                attachments: Vec::new(),
            },
        );
        self.put(section, under, after.as_ref());

        Some(())
    }

    fn retitle(&mut self, section: &SectionId, title: String) -> Option<()> {
        self.sections.get_mut(section)?.title = title;

        Some(())
    }

    fn moved(
        &mut self,
        section: SectionId,
        under: Option<SectionId>,
        after: Option<SectionId>,
    ) -> Option<()> {
        let fits = self.holds(&section)
            && self.knows_where(under.as_ref(), after.as_ref())
            && under
                .as_ref()
                .is_none_or(|under| !self.descends_from(under, &section))
            && after.as_ref() != Some(&section);
        if !fits {
            return None;
        }
        self.put(section, under, after.as_ref());

        Some(())
    }

    fn promote(&mut self, section: SectionId) -> Option<()> {
        let parent = self.parent_of(&section)?;
        let rank = self.rank_of(&section)?;
        let adopted = self.siblings_of(Some(&parent))[rank + 1..].to_vec();
        let grandparent = self.parent_of(&parent);
        self.put(section.clone(), grandparent, Some(&parent));
        for each in adopted {
            let last = self.siblings_of(Some(&section)).last().cloned();
            self.put(each, Some(section.clone()), last.as_ref());
        }

        Some(())
    }

    fn demote(&mut self, section: SectionId) -> Option<()> {
        let rank = self.rank_of(&section)?;
        let elder = self
            .siblings_of(self.parent_of(&section).as_ref())
            .get(rank.checked_sub(1)?)?
            .clone();
        let last = self.siblings_of(Some(&elder)).last().cloned();
        self.put(section, Some(elder), last.as_ref());

        Some(())
    }

    fn remove(&mut self, section: SectionId) -> Option<()> {
        let rank = self.rank_of(&section)?;
        let parent = self.parent_of(&section);
        self.lift(&section);
        let orphans = self.sections.remove(&section)?.children;
        for (at, orphan) in (rank..).zip(orphans) {
            if let Some(held) = self.sections.get_mut(&orphan) {
                held.parent = parent.clone();
            }
            let siblings = self.siblings_mut(parent.as_ref());
            siblings.insert(at.min(siblings.len()), orphan);
        }

        Some(())
    }

    fn attach(
        &mut self,
        attachment: Attachment,
        to: &SectionId,
        after: Option<Attachment>,
    ) -> Option<()> {
        if !self.holds(to) {
            return None;
        }
        for held in self.sections.values_mut() {
            held.attachments.retain(|held| held != &attachment);
        }
        let held = self.sections.get_mut(to)?;
        let at = after
            .and_then(|after| held.attachments.iter().position(|held| held == &after))
            .map_or(0, |nth| nth + 1);
        held.attachments
            .insert(at.min(held.attachments.len()), attachment);

        Some(())
    }

    fn detach(&mut self, attachment: &Attachment) -> Option<()> {
        let held = self
            .sections
            .values_mut()
            .find(|held| held.attachments.contains(attachment))?;
        held.attachments.retain(|held| held != attachment);

        Some(())
    }
}

#[cfg(test)]
mod taken_in_tests {
    use super::*;

    fn a_section(id: &str, parent: Option<&str>) -> Section {
        Section {
            id: SectionId::from(id.to_owned()),
            parent: parent.map(|parent| SectionId::from(parent.to_owned())),
            title: id.to_owned(),
            attachments: Vec::new(),
        }
    }

    fn an_outline(sections: Vec<Section>) -> Outline {
        Outline {
            id: OutlineId::from("outline_1".to_owned()),
            version: 7,
            sections,
        }
    }

    fn shape(outline: &Outline) -> Vec<String> {
        outline
            .sections
            .iter()
            .map(|section| match &section.parent {
                None => section.id.to_string(),
                Some(parent) => format!("{} in {parent}", section.id),
            })
            .collect()
    }

    fn a_book() -> Outline {
        an_outline(vec![
            a_section("one", None),
            a_section("one_a", Some("one")),
            a_section("one_b", Some("one")),
            a_section("one_c", Some("one")),
            a_section("two", None),
        ])
    }

    fn scene(id: &str) -> AttachmentDTO {
        AttachmentDTO::Scene { id: id.to_owned() }
    }

    #[test]
    fn a_section_added_after_another_lands_beside_it_and_the_version_moves_on() {
        let outline = a_book()
            .taken_in(OutlineEventDTO::SectionAdded {
                section: "one_ab".to_owned(),
                under: Some("one".to_owned()),
                after: Some("one_a".to_owned()),
                title: "Between".to_owned(),
            })
            .expect("its place is known");

        assert_eq!(
            shape(&outline),
            vec![
                "one",
                "one_a in one",
                "one_ab in one",
                "one_b in one",
                "one_c in one",
                "two"
            ]
        );
        assert_eq!(outline.sections[2].title, "Between");
        assert_eq!(outline.version, 8);
    }

    #[test]
    fn a_section_added_with_no_neighbour_comes_first_among_its_siblings() {
        let outline = a_book()
            .taken_in(OutlineEventDTO::SectionAdded {
                section: "zero".to_owned(),
                under: None,
                after: None,
                title: String::new(),
            })
            .expect("the top of the book is known");

        assert_eq!(shape(&outline)[0], "zero");
    }

    #[test]
    fn a_section_added_twice_or_beside_one_not_there_does_not_fit() {
        let added = |section: &str, under: Option<&str>, after: Option<&str>| {
            a_book().taken_in(OutlineEventDTO::SectionAdded {
                section: section.to_owned(),
                under: under.map(str::to_owned),
                after: after.map(str::to_owned),
                title: String::new(),
            })
        };

        assert_eq!(added("two", None, None), None);
        assert_eq!(added("new", Some("nowhere"), None), None);
        assert_eq!(added("new", None, Some("one_a")), None);
    }

    #[test]
    fn a_section_retitled_keeps_its_place() {
        let outline = a_book()
            .taken_in(OutlineEventDTO::SectionRetitled {
                section: "one_b".to_owned(),
                title: "The middle".to_owned(),
            })
            .expect("the section is there");

        assert_eq!(outline.sections[2].title, "The middle");
        assert_eq!(shape(&outline), shape(&a_book()));
    }

    #[test]
    fn a_section_moved_takes_what_is_inside_it_along() {
        let outline = a_book()
            .taken_in(OutlineEventDTO::SectionMoved {
                section: "one".to_owned(),
                under: None,
                after: Some("two".to_owned()),
            })
            .expect("its place is known");

        assert_eq!(
            shape(&outline),
            vec!["two", "one", "one_a in one", "one_b in one", "one_c in one"]
        );
    }

    #[test]
    fn a_section_moved_inside_itself_or_after_itself_does_not_fit() {
        let moved = |under: Option<&str>, after: Option<&str>| {
            a_book().taken_in(OutlineEventDTO::SectionMoved {
                section: "one".to_owned(),
                under: under.map(str::to_owned),
                after: after.map(str::to_owned),
            })
        };

        assert_eq!(moved(Some("one_a"), None), None);
        assert_eq!(moved(None, Some("one")), None);
    }

    #[test]
    fn a_section_promoted_adopts_the_siblings_after_it() {
        let outline = a_book()
            .taken_in(OutlineEventDTO::SectionPromoted {
                section: "one_b".to_owned(),
            })
            .expect("it has a parent to step out of");

        assert_eq!(
            shape(&outline),
            vec!["one", "one_a in one", "one_b", "one_c in one_b", "two"]
        );
    }

    #[test]
    fn a_section_demoted_goes_last_into_the_one_before_it() {
        let outline = a_book()
            .taken_in(OutlineEventDTO::SectionDemoted {
                section: "two".to_owned(),
            })
            .expect("it has a section before it");

        assert_eq!(
            shape(&outline),
            vec![
                "one",
                "one_a in one",
                "one_b in one",
                "one_c in one",
                "two in one"
            ]
        );
    }

    #[test]
    fn a_section_with_nowhere_to_step_does_not_fit() {
        assert_eq!(
            a_book().taken_in(OutlineEventDTO::SectionPromoted {
                section: "one".to_owned()
            }),
            None
        );
        assert_eq!(
            a_book().taken_in(OutlineEventDTO::SectionDemoted {
                section: "one".to_owned()
            }),
            None
        );
    }

    #[test]
    fn a_section_removed_leaves_what_was_inside_it_in_its_place() {
        let outline = a_book()
            .taken_in(OutlineEventDTO::SectionRemoved {
                section: "one".to_owned(),
            })
            .expect("the section is there");

        assert_eq!(shape(&outline), vec!["one_a", "one_b", "one_c", "two"]);
    }

    #[test]
    fn a_section_removed_from_among_its_siblings_leaves_its_own_in_its_place_there() {
        let outline = an_outline(vec![
            a_section("one", None),
            a_section("one_a", Some("one")),
            a_section("one_b", Some("one")),
            a_section("one_b_i", Some("one_b")),
            a_section("one_b_ii", Some("one_b")),
            a_section("one_c", Some("one")),
        ])
        .taken_in(OutlineEventDTO::SectionRemoved {
            section: "one_b".to_owned(),
        })
        .expect("the section is there");

        assert_eq!(
            shape(&outline),
            vec![
                "one",
                "one_a in one",
                "one_b_i in one",
                "one_b_ii in one",
                "one_c in one"
            ]
        );
    }

    #[test]
    fn something_attached_leaves_where_it_was_and_lands_after_its_neighbour() {
        let mut book = a_book();
        book.sections[1].attachments = vec![
            Attachment::Scene(SceneId::from("scene_1".to_owned())),
            Attachment::Scene(SceneId::from("scene_2".to_owned())),
        ];
        book.sections[4].attachments = vec![Attachment::Scene(SceneId::from("scene_3".to_owned()))];

        let outline = book
            .taken_in(OutlineEventDTO::Attached {
                attachment: scene("scene_1"),
                to: "two".to_owned(),
                after: Some(scene("scene_3")),
            })
            .expect("the section is there");

        assert_eq!(
            outline.sections[1].attachments,
            vec![Attachment::Scene(SceneId::from("scene_2".to_owned()))]
        );
        assert_eq!(
            outline.sections[4].attachments,
            vec![
                Attachment::Scene(SceneId::from("scene_3".to_owned())),
                Attachment::Scene(SceneId::from("scene_1".to_owned())),
            ]
        );
    }

    #[test]
    fn something_detached_leaves_and_what_is_not_there_does_not_fit() {
        let mut book = a_book();
        book.sections[0].attachments = vec![Attachment::Scene(SceneId::from("scene_1".to_owned()))];

        let outline = book
            .clone()
            .taken_in(OutlineEventDTO::Detached {
                attachment: scene("scene_1"),
                from: "one".to_owned(),
            })
            .expect("it is there");

        assert!(outline.sections[0].attachments.is_empty());
        assert_eq!(
            book.taken_in(OutlineEventDTO::Detached {
                attachment: scene("scene_9"),
                from: "one".to_owned(),
            }),
            None
        );
    }

    #[test]
    fn a_change_to_a_section_not_there_or_an_outline_started_or_discarded_does_not_fit() {
        assert_eq!(
            a_book().taken_in(OutlineEventDTO::SectionRetitled {
                section: "nowhere".to_owned(),
                title: String::new(),
            }),
            None
        );
        assert_eq!(
            a_book().taken_in(OutlineEventDTO::SectionRemoved {
                section: "nowhere".to_owned(),
            }),
            None
        );
        assert_eq!(
            a_book().taken_in(OutlineEventDTO::Attached {
                attachment: scene("scene_1"),
                to: "nowhere".to_owned(),
                after: None,
            }),
            None
        );
        assert_eq!(
            a_book().taken_in(OutlineEventDTO::Started {
                project: "project_1".to_owned()
            }),
            None
        );
        assert_eq!(a_book().taken_in(OutlineEventDTO::Discarded), None);
    }
}
