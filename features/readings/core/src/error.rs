use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReadingsError {
    #[error("there is no such project")]
    ProjectNotThere,
}
