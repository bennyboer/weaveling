use crate::projects::model::ProjectId;

pub const WORKSPACE: &str = "/";

pub fn project(id: &ProjectId) -> String {
    format!("/projects/{id}")
}
