#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Oracle error: {0}")]
    Oracle(#[from] oracle::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("task failed: {0}")]
    Join(#[from] tokio::task::JoinError),

    #[error("unsupported Oracle object type: {0}")]
    UnsupportedObjectType(String),

    #[error("concurrency must be greater than zero")]
    InvalidConcurrency,

    #[error("failed to export {object_type} {object_name}: {source}")]
    ExportObject {
        object_type: &'static str,
        object_name: String,
        source: Box<Error>,
    },
}

pub type Result<T> = std::result::Result<T, Error>;
