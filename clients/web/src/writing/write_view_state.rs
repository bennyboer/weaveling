use crate::projects::model::ProjectId;
use crate::writing::beginnings::Beginnings;
use crate::writing::labels::Labels;
use crate::writing::margin::Margin;
use crate::writing::problems::Problems;
use crate::writing::saving::Saving;
use crate::writing::structure::Structure;
use crate::writing::walk::Walk;
use crate::writing::word_count::WordCount;

#[derive(Clone, Copy)]
pub struct WriteViewState {
    pub problems: Problems,
    pub structure: Structure,
    pub walk: Walk,
    pub beginnings: Beginnings,
    pub labels: Labels,
    pub margin: Margin,
    pub saving: Saving,
    pub word_count: WordCount,
}

impl WriteViewState {
    pub fn open(project: &ProjectId) -> Self {
        let problems = Problems::none();
        let saving = Saving::nothing();
        let structure = Structure::open(project, problems, saving);

        Self {
            problems,
            structure,
            walk: Walk::through(structure),
            beginnings: Beginnings::of(structure, problems),
            labels: Labels::of(project, problems, saving),
            margin: Margin::closed(),
            saving,
            word_count: WordCount::of(structure),
        }
    }
}
