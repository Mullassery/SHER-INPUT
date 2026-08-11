use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("device not found: {0}")]
    DeviceNotFound(String),

    #[error("device already registered: {0}")]
    DeviceAlreadyRegistered(String),

    #[error("backend error: {0}")]
    Backend(String),

    #[error("backend unsupported on this platform: {0}")]
    Unsupported(String),

    #[error("grab denied: {0}")]
    GrabDenied(String),

    #[error("synthetic input denied: {0}")]
    SyntheticInputDenied(String),

    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
