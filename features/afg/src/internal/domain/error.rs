use thiserror::Error;

#[derive(Debug, Error)]
pub enum AfgError {
    #[error("invalid workflow YAML: {0}")]
    Parse(String),

    #[error("invalid workflow spec: {0}")]
    Validation(String),

    #[error("{0} not found")]
    NotFound(&'static str),

    #[error("could not read workflow file: {0}")]
    Io(String),

    #[error("must provide either source_path or yaml")]
    MissingSource,

    #[error("workflow source path escapes the project root")]
    PathOutsideProject,

    #[error("storage failure: {0}")]
    Storage(String),
}
