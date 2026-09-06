use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq)]
pub enum AppError {
    InvalidInput,
    ForbiddenUrl,
    Upstream,
    Busy,
    Transcode,
    Timeout,
}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for AppError {}
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::InvalidInput => (StatusCode::BAD_REQUEST, "Invalid request"),
            Self::ForbiddenUrl => (
                StatusCode::FORBIDDEN,
                "URL is not an allowed public Twitch resource",
            ),
            Self::Busy => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Server is busy; please retry shortly",
            ),
            Self::Timeout => (StatusCode::GATEWAY_TIMEOUT, "Upstream timed out"),
            Self::Transcode => (StatusCode::BAD_GATEWAY, "Media processing failed"),
            Self::Upstream => (StatusCode::BAD_GATEWAY, "Twitch request failed"),
        };
        let mut response = (status, Json(json!({"error":message}))).into_response();
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            header::HeaderValue::from_static("no-store"),
        );
        if status == StatusCode::SERVICE_UNAVAILABLE {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, header::HeaderValue::from_static("5"));
        }
        response
    }
}
pub(crate) fn invalid() -> Json<Value> {
    Json(json!({"invalid":true}))
}
