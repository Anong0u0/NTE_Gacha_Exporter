use thiserror::Error;

#[derive(Debug, Error)]
pub enum AutomationError {
    #[error("{0}")]
    Message(String),
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[cfg(windows)]
    #[error("windows error: {0}")]
    Windows(#[from] windows::core::Error),
}

pub type AutomationResult<T> = Result<T, AutomationError>;

impl AutomationError {
    pub fn message(value: impl Into<String>) -> Self {
        Self::Message(value.into())
    }
}
