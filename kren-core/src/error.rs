use thiserror::Error;

#[derive(Error, Debug)]
pub enum KrenError {
    #[error("failed to create shared memory segment: {0}")]
    CreateFailed(String),

    #[error("failed to open shared memory segment: {0}")]
    OpenFailed(String),

    #[error("failed to map shared memory: {0}")]
    MapFailed(String),

    #[error("invalid magic number - not a KREN buffer")]
    InvalidMagic,

    #[error("version mismatch: expected {expected}, found {found}")]
    VersionMismatch { expected: u32, found: u32 },

    #[error("buffer is full, cannot write {requested} bytes (available: {available})")]
    BufferFull { requested: usize, available: usize },

    #[error("buffer is empty, no data to read")]
    BufferEmpty,

    #[error("data too large: {size} bytes exceeds maximum {max}")]
    DataTooLarge { size: usize, max: usize },

    #[error("invalid buffer capacity: {0}")]
    InvalidCapacity(usize),

    #[error("channel already closed")]
    ChannelClosed,

    #[error("platform error: {0}")]
    Platform(String),
}

pub type Result<T> = std::result::Result<T, KrenError>;
