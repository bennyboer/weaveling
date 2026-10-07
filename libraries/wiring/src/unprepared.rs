use thiserror::Error;

#[derive(Debug, Error)]
pub enum Unprepared {
    #[error("{database} could not be reached: {why}")]
    Unreachable { database: String, why: String },
    #[error("{database} could not be created: {why}")]
    Uncreatable { database: String, why: String },
    #[error("the schema of {database} could not be laid down: {why}")]
    Unmigrated { database: String, why: String },
    #[error("{feature} cannot be kept in {backend}, because this build left that out")]
    NotBuiltIn {
        feature: &'static str,
        backend: &'static str,
    },
}
