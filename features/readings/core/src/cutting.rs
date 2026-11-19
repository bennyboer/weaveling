use crate::contents::{Contents, Part};
use crate::link::{SceneLink, SectionLink};
use crate::reading_copies::{Extent, Shared};

/// What is to be shared is not in the work, or ends before it begins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutsideTheWork;

/// The work cut to what is shared, and how much of it that is.
/// Sections in a row hold each section from the first to the last in reading order, with the scenes right under each.
/// Scenes in a row hold the scenes from the first to the last, with the sections they stand in,
/// so a reader knows where in the work they are: the sections between them, and those the first scene stands in, from the outermost in.
pub fn cut(work: Contents, shared: &Shared) -> Result<(Contents, Extent), OutsideTheWork> {
    let Contents {
        project,
        title,
        mut parts,
    } = work;
    let (parts, extent) = match shared {
        Shared::Book => (parts, Extent::Book),
        Shared::Sections { first, last } => {
            let (from, to) = in_order(section_at(&parts, first), section_at(&parts, last))?;
            parts.truncate(next_section_after(&parts, to));
            let sections = parts.split_off(from);
            let count = counted(&sections, Part::is_section);

            (sections, Extent::Sections(count))
        }
        Shared::Scenes { first, last } => {
            let (from, to) = in_order(scene_at(&parts, first), scene_at(&parts, last))?;
            parts.truncate(to + 1);
            let scenes = parts.split_off(from);
            let count = counted(&scenes, |part| !part.is_section());
            let mut held = standing_in(parts);
            held.extend(scenes);

            (held, Extent::Scenes(count))
        }
    };

    Ok((
        Contents {
            project,
            title,
            parts,
        },
        extent,
    ))
}

impl Part {
    fn is_section(&self) -> bool {
        matches!(self, Part::Section { .. })
    }
}

fn section_at(parts: &[Part], wanted: &SectionLink) -> Option<usize> {
    parts
        .iter()
        .position(|part| matches!(part, Part::Section { section, .. } if section == wanted))
}

fn scene_at(parts: &[Part], wanted: &SceneLink) -> Option<usize> {
    parts
        .iter()
        .position(|part| matches!(part, Part::Scene { scene, .. } if scene == wanted))
}

/// Where the next section after the one at `at` begins, or the end of the work.
fn next_section_after(parts: &[Part], at: usize) -> usize {
    parts[at + 1..]
        .iter()
        .position(Part::is_section)
        .map_or(parts.len(), |after| at + 1 + after)
}

fn in_order(from: Option<usize>, to: Option<usize>) -> Result<(usize, usize), OutsideTheWork> {
    match (from, to) {
        (Some(from), Some(to)) if from <= to => Ok((from, to)),
        _ => Err(OutsideTheWork),
    }
}

fn counted(parts: &[Part], counts: impl Fn(&Part) -> bool) -> u32 {
    u32::try_from(parts.iter().filter(|part| counts(part)).count()).unwrap_or(u32::MAX)
}

/// The sections a part right after these stands in, the outermost first: the nearest section before it, then each shallower one before that.
fn standing_in(before: Vec<Part>) -> Vec<Part> {
    let mut sections = Vec::new();
    let mut shallower_than = u8::MAX;
    for part in before.into_iter().rev() {
        if let Part::Section { depth, .. } = part
            && depth < shallower_than
        {
            shallower_than = depth;
            sections.push(part);
        }
    }
    sections.reverse();

    sections
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contents::{Block, Run};
    use crate::link::ProjectLink;

    fn section(named: &str, depth: u8) -> Part {
        Part::Section {
            section: SectionLink::from(named),
            depth,
            title: named.to_owned(),
        }
    }

    fn scene(named: &str) -> Part {
        Part::Scene {
            scene: SceneLink::from(named),
            blocks: vec![Block::Paragraph(vec![Run::plain(named)])],
        }
    }

    /// Part One holds a scene and Morning, which holds two scenes and The market; Part Two holds a scene.
    fn the_loom() -> Contents {
        Contents {
            project: ProjectLink::from("project_loom"),
            title: "The Loom".to_owned(),
            parts: vec![
                section("part_one", 0),
                scene("prologue"),
                section("morning", 1),
                scene("harbour"),
                scene("wall"),
                section("market", 2),
                scene("stalls"),
                section("part_two", 0),
                scene("evening"),
            ],
        }
    }

    fn sections(first: &str, last: &str) -> Shared {
        Shared::Sections {
            first: SectionLink::from(first),
            last: SectionLink::from(last),
        }
    }

    fn scenes(first: &str, last: &str) -> Shared {
        Shared::Scenes {
            first: SceneLink::from(first),
            last: SceneLink::from(last),
        }
    }

    fn cut_to(shared: &Shared) -> (Vec<Part>, Extent) {
        let (contents, extent) = cut(the_loom(), shared).expect("the part is in the work");
        assert_eq!(contents.project, ProjectLink::from("project_loom"));
        assert_eq!(contents.title, "The Loom");

        (contents.parts, extent)
    }

    #[test]
    fn the_whole_work_is_all_of_it() {
        // expect: the whole work, as it is
        assert_eq!(cut_to(&Shared::Book), (the_loom().parts, Extent::Book));
    }

    #[test]
    fn sections_in_a_row_hold_each_with_the_scenes_right_under_it() {
        // expect: one section holds itself and its own scenes, not the section nested under it
        assert_eq!(
            cut_to(&sections("morning", "morning")),
            (
                vec![section("morning", 1), scene("harbour"), scene("wall")],
                Extent::Sections(1)
            )
        );
        // and: a run holds every section between, whatever its depth, up to the next section after the last
        assert_eq!(
            cut_to(&sections("morning", "market")),
            (
                vec![
                    section("morning", 1),
                    scene("harbour"),
                    scene("wall"),
                    section("market", 2),
                    scene("stalls"),
                ],
                Extent::Sections(2)
            )
        );
        // and: a run up to the last section holds the work to its end
        assert_eq!(
            cut_to(&sections("market", "part_two")),
            (
                vec![
                    section("market", 2),
                    scene("stalls"),
                    section("part_two", 0),
                    scene("evening"),
                ],
                Extent::Sections(2)
            )
        );
    }

    #[test]
    fn scenes_in_a_row_hold_the_sections_they_stand_in() {
        // expect: a scene deep in the work holds the sections it stands in, from the outermost in
        assert_eq!(
            cut_to(&scenes("stalls", "stalls")),
            (
                vec![
                    section("part_one", 0),
                    section("morning", 1),
                    section("market", 2),
                    scene("stalls"),
                ],
                Extent::Scenes(1)
            )
        );
        // and: a run across sections holds those between, and counts only its scenes
        assert_eq!(
            cut_to(&scenes("wall", "evening")),
            (
                vec![
                    section("part_one", 0),
                    section("morning", 1),
                    scene("wall"),
                    section("market", 2),
                    scene("stalls"),
                    section("part_two", 0),
                    scene("evening"),
                ],
                Extent::Scenes(3)
            )
        );
        // and: a scene right under a section passes over the sibling sections before it
        assert_eq!(
            cut_to(&scenes("evening", "evening")),
            (
                vec![section("part_two", 0), scene("evening")],
                Extent::Scenes(1)
            )
        );
    }

    #[test]
    fn what_is_not_in_the_work_or_ends_before_it_begins_is_outside_it() {
        // expect: an end the work does not hold, or a run backwards, is outside the work
        for shared in [
            sections("morning", "gone"),
            sections("gone", "morning"),
            sections("market", "morning"),
            scenes("wall", "harbour"),
            scenes("harbour", "gone"),
            scenes("morning", "harbour"),
        ] {
            assert_eq!(
                cut(the_loom(), &shared).map(|_| ()),
                Err(OutsideTheWork),
                "{shared:?}"
            );
        }
    }
}
