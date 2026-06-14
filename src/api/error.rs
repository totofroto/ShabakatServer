use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Internal server error: {0}")]
    Internal(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Scan in progress: {0}")]
    ScanInProgress(String),
}

impl ApiError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::ScanInProgress(_) => StatusCode::CONFLICT,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let details = match &self {
            Self::Internal(s) => s.clone(),
            Self::NotFound(s) => s.clone(),
            Self::Unauthorized(s) => s.clone(),
            Self::BadRequest(s) => s.clone(),
            Self::Database(e) => e.to_string(),
            Self::ScanInProgress(s) => s.clone(),
        };

        let body = Json(json!({
            "error": self.to_string(),
            "code": status.as_u16(),
            "details": details
        }));

        (status, body).into_response()
    }
}

impl From<String> for ApiError {
    fn from(s: String) -> Self {
        Self::Internal(s)
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
