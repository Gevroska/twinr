use crate::{ffmpeg::spawn_opus_transcode_response, hls::*, proxy::fetch_raw_text};
use crate::{state::AppState, twitch::gql};
use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use rand::Rng;
use serde_json::{json, Value};
use std::sync::Arc;
pub(crate) async fn stream_proxy(
    State(state): State<Arc<AppState>>,
    Path(username): Path<String>,
    Query(query): Query<QualityQuery>,
) -> impl IntoResponse {
    if !crate::security::username(&username) {
        return crate::errors::AppError::InvalidInput.into_response();
    }

    let quality = parse_quality_preference(query.quality.as_deref());
    let tr=match gql(&state,json!({"query":"query StreamPlayer_Query($login: String!, $playerType: String!, $platform: String!, $skipPlayToken: Boolean!) { user(login: $login) { stream @skip(if: $skipPlayToken) { playbackAccessToken(params: {platform: $platform, playerType: $playerType}) { signature value } } } }","variables":{"login":username.to_lowercase(),"playerType":"pulsar","platform":"mobile_web","skipPlayToken":false}}),true).await {Ok(v)=>v,Err(e)=>return e.into_response()};
    let sig = tr
        .pointer("/data/user/stream/playbackAccessToken/signature")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let token = tr
        .pointer("/data/user/stream/playbackAccessToken/value")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if sig.is_empty() || token.is_empty() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":true,"data":null})),
        )
            .into_response();
    }
    let url = format!("https://usher.ttvnw.net/api/channel/hls/{}.m3u8?player_type=pulsar&player_backend=mediaplayer&playlist_include_framerate=true&allow_source=true&allow_audio_only=true&transcode_mode=cbr_v1&cdm=wv&player_version=1.22.0&token={}&sig={}", username.to_lowercase(), urlencoding::encode(token), sig);
    let list_text = match fetch_raw_text(&state, &url, true).await {
        Ok(t) => t,
        Err(r) => return r,
    };

    if let QualityPreference::AudioOpus(bitrate) = quality {
        if !state.opus_audio_bitrates.contains(&bitrate) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error":true,"message":"Requested opus bitrate is disabled"})),
            )
                .into_response();
        }
        let selected_ref =
            select_playlist(&list_text, &QualityPreference::AudioOnly).unwrap_or_default();
        let selected = resolve_playlist_url(&url, &selected_ref);
        if selected.is_empty() {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error":true,"data":null})),
            )
                .into_response();
        }
        return spawn_opus_transcode_response(state.clone(), &selected, bitrate).await;
    }

    if matches!(quality, QualityPreference::Auto) {
        return (
            [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")],
            proxy_vod_manifest(&url, &list_text),
        )
            .into_response();
    }

    let selected_ref = select_playlist(&list_text, &quality).unwrap_or_default();
    let selected = resolve_playlist_url(&url, &selected_ref);
    if selected.is_empty() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":true,"data":null})),
        )
            .into_response();
    }

    let manifest = match fetch_raw_text(&state, &selected, true).await {
        Ok(t) => t,
        Err(r) => return r,
    };

    let body = proxy_vod_manifest(&selected, &manifest);

    (
        [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")],
        body,
    )
        .into_response()
}

pub(crate) async fn vod_proxy(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<QualityQuery>,
) -> impl IntoResponse {
    let quality = parse_quality_preference(query.quality.as_deref());
    let (playlist_url, list_text) = match vod_master(&state, &id).await {
        Ok(value) => value,
        Err(response) => return response,
    };

    if let QualityPreference::AudioOpus(bitrate) = quality {
        if !state.opus_audio_bitrates.contains(&bitrate) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error":true,"message":"Requested opus bitrate is disabled"})),
            )
                .into_response();
        }
        let selected_ref =
            select_playlist(&list_text, &QualityPreference::AudioOnly).unwrap_or_default();
        let selected = resolve_playlist_url(&playlist_url, &selected_ref);
        if selected.is_empty() {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error":true,"data":null})),
            )
                .into_response();
        }
        return spawn_opus_transcode_response(state.clone(), &selected, bitrate).await;
    }

    if matches!(quality, QualityPreference::Auto) {
        return (
            [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")],
            proxy_vod_manifest(&playlist_url, &list_text),
        )
            .into_response();
    }

    let selected_ref = select_playlist(&list_text, &quality).unwrap_or_default();
    let selected = resolve_playlist_url(&playlist_url, &selected_ref);
    if selected.is_empty() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":true,"data":null})),
        )
            .into_response();
    }
    let manifest = match fetch_raw_text(&state, &selected, false).await {
        Ok(t) => t,
        Err(r) => return r,
    };
    let body = proxy_vod_manifest(&selected, &manifest);
    (
        [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")],
        body,
    )
        .into_response()
}

pub(crate) fn vod_token_request(id: &str) -> Value {
    json!({
        "query": "query VodPlaybackAccessToken($vodID: ID!, $playerType: String!) { videoPlaybackAccessToken(id: $vodID, params: {platform: \"web\", playerBackend: \"mediaplayer\", playerType: $playerType}) { value signature } }",
        "variables": {"vodID": id, "playerType": "site"}
    })
}

pub(crate) async fn vod_master(state: &AppState, id: &str) -> Result<(String, String), Response> {
    if !crate::security::vod_id(id) {
        return Err(crate::errors::AppError::InvalidInput.into_response());
    }
    let token = gql(state, vod_token_request(id), false).await;
    let Ok(token) = token else {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":true,"data":null})),
        )
            .into_response());
    };
    let sig = token
        .pointer("/data/videoPlaybackAccessToken/signature")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let val = token
        .pointer("/data/videoPlaybackAccessToken/value")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if sig.is_empty() || val.is_empty() {
        tracing::warn!(vod_id = %id, "Twitch did not return a VOD playback token");
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":true,"data":null})),
        )
            .into_response());
    }

    let p: u32 = rand::thread_rng().gen_range(1..=99999);
    let playlist_url = format!("https://usher.ttvnw.net/vod/{id}.m3u8?acmb=e30%3D&allow_source=true&allow_audio_only=true&p={p}&cdm=wv&transcode_mode=cbr_v1&supported_codecs=avc1&player_version=1.19.0&player_base=mediaplayer&reassignments_supported=true&playlist_include_framerate=true&player_backend=mediaplayer&token={}&sig={}", urlencoding::encode(val), sig);
    let list_text = match fetch_raw_text(&state, &playlist_url, false).await {
        Ok(t) => t,
        Err(r) => return Err(r),
    };

    Ok((playlist_url, list_text))
}

pub(crate) async fn vod_download(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<QualityQuery>,
) -> Response {
    if !crate::security::vod_id(&id) {
        return crate::errors::AppError::InvalidInput.into_response();
    }
    let (master, details) = tokio::join!(
        vod_master(&state, &id),
        crate::metadata::vod_details(&state, &id)
    );
    let (source, master) = match master {
        Ok(v) => v,
        Err(r) => return r,
    };
    let quality = parse_quality_preference(query.quality.as_deref());
    let selected = resolve_playlist_url(
        &source,
        &select_playlist(&master, &quality).unwrap_or_default(),
    );
    let title = details
        .ok()
        .and_then(|v| {
            v.pointer("/data/video/title")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "twinr-vod".into());
    crate::ffmpeg::process(
        state,
        &selected,
        crate::ffmpeg::Output::Download,
        Some(crate::ffmpeg::filename(&title, &id)),
    )
    .await
    .unwrap_or_else(IntoResponse::into_response)
}
