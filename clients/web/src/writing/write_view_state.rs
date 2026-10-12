use crate::projects::model::ProjectId;
use crate::writing::beginnings::Beginnings;
use crate::writing::labels::Labels;
use crate::writing::problems::Problems;
use crate::writing::structure::Structure;
use crate::writing::walk::Walk;

#[derive(Clone, Copy)]
pub struct WriteViewState {
    pub problems: Problems,
    pub structure: Structure,
    pub walk: Walk,
    pub beginnings: Beginnings,
    pub labels: Labels,
}

impl WriteViewState {
    pub fn open(project: &ProjectId) -> Self {
        let problems = Problems::none();
        let structure = Structure::open(project, problems);

        Self {
            problems,
            structure,
            walk: Walk::through(structure),
            beginnings: Beginnings::of(structure, problems),
            labels: Labels::of(project, problems),
        }
    }
}
