//! Structured HTTP error mapping for Axum handlers.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use biodex_core::crypto::CryptoError;
use biodex_core::taxonomy::TaxonomyError;
use serde::Serialize;

/// Unified API error type mapped cleanly to HTTP status codes and JSON payloads.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

#[derive(Serialize)]
struct ErrorEnvelope<'a> {
    error: ErrorBody<'a>,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    message: &'a str,
}

impl ApiError {
    /// Creates a new [`ApiError`] with status code, stable machine code, and message.
    #[must_use]
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    /// Returns a `400 Bad Request` API error.
    #[must_use]
    pub fn bad_request(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message)
    }

    /// Returns a `409 Conflict` API error.
    #[must_use]
    pub fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, code, message)
    }

    /// Returns a `500 Internal Server Error` API error (without leaking internals).
    #[must_use]
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(ErrorEnvelope {
            error: ErrorBody {
                code: self.code,
                message: &self.message,
            },
        });
        (self.status, body).into_response()
    }
}

impl From<CryptoError> for ApiError {
    fn from(err: CryptoError) -> Self {
        let (status, code) = match err {
            CryptoError::GallerySourceNotCompetitive => {
                (StatusCode::FORBIDDEN, "gallery_not_competitive")
            }
            CryptoError::MoireThresholdExceeded { .. } => {
                (StatusCode::FORBIDDEN, "moire_threshold_exceeded")
            }
            CryptoError::SignatureVerificationFailed => {
                (StatusCode::UNAUTHORIZED, "invalid_signature")
            }
            CryptoError::InvalidCaptureId
            | CryptoError::InvalidTimestamp
            | CryptoError::InvalidCoordinates
            | CryptoError::InvalidImuTelemetry
            | CryptoError::InvalidMoireScore
            | CryptoError::InvalidPublicKey
            | CryptoError::InvalidSignatureEncoding => {
                (StatusCode::BAD_REQUEST, "invalid_capture_metadata")
            }
        };
        Self::new(status, code, err.to_string())
    }
}

impl From<TaxonomyError> for ApiError {
    fn from(err: TaxonomyError) -> Self {
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "taxonomy_error",
            err.to_string(),
        )
    }
}
