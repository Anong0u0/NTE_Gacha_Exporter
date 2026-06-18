use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RuntimeError {
    pub(crate) code: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ApiError {
    code: String,
    message: String,
}

pub(crate) fn api_error(error: impl std::fmt::Display) -> ApiError {
    ApiError {
        code: "internal_error".to_string(),
        message: error.to_string(),
    }
}

pub(crate) fn api_error_message(
    code: impl Into<String>,
    message: impl std::fmt::Display,
) -> ApiError {
    ApiError {
        code: code.into(),
        message: message.to_string(),
    }
}
