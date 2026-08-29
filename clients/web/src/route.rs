use crate::projects::model::ProjectId;
use crate::url;

const PROJECTS: &str = "/projects";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    Workspace,
    Project(ProjectId),
}

pub fn current() -> Route {
    read(&url::path())
}

pub fn go(route: &Route) {
    url::go(&address(route));
}

fn read(path: &str) -> Route {
    match path.trim_end_matches('/').strip_prefix(PROJECTS) {
        Some(rest) => match rest.trim_start_matches('/') {
            "" => Route::Workspace,
            project => Route::Project(ProjectId::from(project.to_owned())),
        },
        None => Route::Workspace,
    }
}

fn address(route: &Route) -> String {
    match route {
        Route::Workspace => "/".to_owned(),
        Route::Project(project) => format!("{PROJECTS}/{project}"),
    }
}
