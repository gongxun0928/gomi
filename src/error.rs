use thiserror::Error;

#[derive(Debug, Error)]
pub enum GomiError {
    #[error("{0}")]
    Msg(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, GomiError>;
