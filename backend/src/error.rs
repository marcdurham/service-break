use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("not found")]
    NotFound,
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    Conflict(String),
    #[error("internal error")]
    Internal(String),
    #[error("database error")]
    Db(#[from] sqlx::Error),
    #[error("geocoding service error")]
    Upstream(#[from] reqwest::Error),
}

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            ApiError::NotFound => StatusCode::NOT_FOUND,
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            ApiError::Forbidden(_) => StatusCode::FORBIDDEN,
            ApiError::Conflict(_) => StatusCode::CONFLICT,
            ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Db(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Upstream(_) => StatusCode::BAD_GATEWAY,
        }
    }

    fn error_response(&self) -> HttpResponse {
        match self {
            ApiError::Db(err) => tracing::error!(error = %err, "database error"),
            ApiError::Internal(detail) => tracing::error!(%detail, "internal error"),
            _ => {}
        }
        HttpResponse::build(self.status_code()).json(json!({ "error": self.to_string() }))
    }
}
