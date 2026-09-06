use crate::{chat::root_or_ws, clips::*, media::*, metadata::*, proxy::*, state::AppState};
use axum::{
    extract::State,
    http::{header, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde_json::json;
use std::sync::Arc;
use tokio::fs;
use tower_http::services::ServeDir;
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(root_or_ws))
        .route("/api", get(api_root))
        .route("/api/streaminfo/:username", get(stream_info))
        .route("/api/streamer/:username", get(streamer_info))
        .route("/api/vodinfo/:id", get(vod_info))
        .route("/api/vodinfo/comments/:id/:offset", get(vod_comments))
        .route("/api/vods/:username/:filter/:limit", get(vod_list))
        .route("/api/clipinfo/:username/:id", get(clip_info))
        .route("/api/clips/:username/:filter/:limit", get(clips_list))
        .route("/api/emotes/:username", get(emotes))
        .route("/api/user/:username", get(user_info))
        .route("/api/users", axum::routing::post(users_bulk))
        .route("/api/vod/:id/download", get(vod_download))
        .route("/api/stream/:username", get(stream_proxy))
        .route("/api/vod/:id", get(vod_proxy))
        .route("/clipproxy/:media/:sig/:token", get(clip_proxy))
        .route("/api/urlproxy", get(urlproxy))
        .route("/api/proxy", get(proxy))
        .route("/api/playlist", get(playlist_proxy))
        .route("/videos/:id", get(index_file))
        .route("/:username/clip/:id", get(clip_page_or_index))
        .route("/:username", get(index_file))
        .fallback_service(ServeDir::new("public"))
        .layer(axum::extract::DefaultBodyLimit::max(16384))
        .layer(axum::middleware::from_fn(response_headers))
        .with_state(state)
}

async fn response_headers(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let path = request.uri().path().to_string();
    let mut response = next.run(request).await;
    let h = response.headers_mut();
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    h.insert(
        header::X_FRAME_OPTIONS,
        HeaderValue::from_static("SAMEORIGIN"),
    );
    h.insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    if !h.contains_key(header::CACHE_CONTROL) {
        h.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static(if path.starts_with("/assets/") {
                "public, max-age=31536000, immutable"
            } else {
                "no-cache"
            }),
        );
    }
    response
}
pub(crate) async fn index_file() -> impl IntoResponse {
    match fs::read_to_string("public/index.html").await {
        Ok(s) => Html(s).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "index.html missing").into_response(),
    }
}

async fn api_root(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(
        json!({"version": state.version, "api":"v0", "opusAudioBitrates": state.opus_audio_bitrates}),
    )
}
