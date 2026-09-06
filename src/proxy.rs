use crate::{errors::AppError, hls::proxy_vod_manifest, security, state::AppState};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Deserialize;
use std::{sync::Arc, time::Duration};
#[derive(Deserialize)]
pub(crate) struct UrlQ {
    pub url: Option<String>,
}
fn decode(value: Option<String>) -> Result<String, AppError> {
    let value = value.ok_or(AppError::InvalidInput)?;
    if value.len() > 24000 {
        return Err(AppError::InvalidInput);
    }
    String::from_utf8(
        STANDARD
            .decode(value.replace(' ', "+"))
            .map_err(|_| AppError::InvalidInput)?,
    )
    .map_err(|_| AppError::InvalidInput)
}
fn upstream_headers(state: &AppState, range: &HeaderMap) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::USER_AGENT,
        HeaderValue::from_str(&state.user_agent).unwrap(),
    );
    headers.insert(
        header::REFERER,
        HeaderValue::from_static("https://player.twitch.tv/"),
    );
    headers.insert(
        header::ORIGIN,
        HeaderValue::from_static("https://player.twitch.tv"),
    );
    if let Some(value) = range.get(header::RANGE) {
        headers.insert(header::RANGE, value.clone());
    }
    headers
}
pub(crate) async fn fetch_raw_text(
    state: &AppState,
    url: &str,
    _mobile: bool,
) -> Result<String, Response> {
    let result = tokio::time::timeout(Duration::from_secs(30), async {
        let response =
            security::get(state, url, upstream_headers(state, &HeaderMap::new())).await?;
        let final_url = response.url().to_string();
        let text = security::limited_text(response, 8 * 1024 * 1024).await?;
        if !text.trim_start_matches('\u{feff}').starts_with("#EXTM3U") {
            return Err(AppError::Upstream);
        }
        if final_url != url {
            Ok(crate::hls::rewrite_with(&text, |r, _| {
                crate::hls::resolve_playlist_url(&final_url, r)
            }))
        } else {
            Ok(text)
        }
    })
    .await
    .unwrap_or(Err(AppError::Timeout));
    result.map_err(IntoResponse::into_response)
}
pub(crate) async fn playlist_proxy(
    State(state): State<Arc<AppState>>,
    Query(q): Query<UrlQ>,
) -> Response {
    let url = match decode(q.url) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    match fetch_raw_text(&state, &url, false).await {
        Ok(manifest) => (
            [
                (header::CONTENT_TYPE, "application/vnd.apple.mpegurl"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            proxy_vod_manifest(&url, &manifest),
        )
            .into_response(),
        Err(r) => r,
    }
}
pub(crate) async fn proxy(
    State(state): State<Arc<AppState>>,
    Query(q): Query<UrlQ>,
    headers: HeaderMap,
) -> Response {
    match decode(q.url) {
        Ok(url) => pipe_url_range(&state, &url, &headers).await,
        Err(e) => e.into_response(),
    }
}
pub(crate) async fn urlproxy(
    State(state): State<Arc<AppState>>,
    Query(q): Query<UrlQ>,
    headers: HeaderMap,
) -> Response {
    match q.url {
        Some(url) => pipe_url_range(&state, &url, &headers).await,
        None => AppError::InvalidInput.into_response(),
    }
}
pub(crate) async fn clip_proxy(
    State(state): State<Arc<AppState>>,
    Path((media, sig, token)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Response {
    let mut url = match security::validate_url(&media) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    url.query_pairs_mut()
        .append_pair("sig", &sig)
        .append_pair("token", &token);
    pipe_url_range(&state, url.as_str(), &headers).await
}
pub(crate) async fn pipe_url_range(state: &AppState, url: &str, headers: &HeaderMap) -> Response {
    match security::get(state, url, upstream_headers(state, headers)).await {
        Ok(response) => stream_response(response),
        Err(e) => e.into_response(),
    }
}
fn stream_response(response: reqwest::Response) -> Response {
    let status = response.status();
    if !status.is_success() && status != StatusCode::RANGE_NOT_SATISFIABLE {
        return AppError::Upstream.into_response();
    }
    let ct = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream");
    let mime = ct.split(';').next().unwrap_or("").trim();
    if !(mime.starts_with("video/")
        || mime.starts_with("audio/")
        || [
            "image/png",
            "image/jpeg",
            "image/gif",
            "image/webp",
            "image/avif",
            "application/octet-stream",
            "binary/octet-stream",
            "application/vnd.apple.mpegurl",
            "application/x-mpegurl",
        ]
        .contains(&mime))
    {
        return AppError::Upstream.into_response();
    }
    let mut headers = HeaderMap::new();
    for name in [
        header::CONTENT_TYPE,
        header::CONTENT_LENGTH,
        header::CONTENT_RANGE,
        header::ACCEPT_RANGES,
        header::CACHE_CONTROL,
    ] {
        if let Some(v) = response.headers().get(&name) {
            headers.insert(name, v.clone());
        }
    }
    headers.insert("x-accel-buffering", HeaderValue::from_static("no"));
    let stream = futures::stream::try_unfold(response, |mut response| async move {
        response
            .chunk()
            .await
            .map(|chunk| chunk.map(|bytes| (bytes, response)))
    });
    (status, headers, Body::from_stream(stream)).into_response()
}
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    #[tokio::test]
    async fn first_chunk_arrives_before_upstream_completes() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (release, wait) = tokio::sync::oneshot::channel::<()>();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            socket.read(&mut request).await.unwrap();
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: video/mp2t\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nfirst\r\n").await.unwrap();
            let _ = wait.await;
            let _ = socket.write_all(b"4\r\nlast\r\n0\r\n\r\n").await;
        });
        let upstream = reqwest::Client::new()
            .get(format!("http://{addr}"))
            .send()
            .await
            .unwrap();
        let mut body = stream_response(upstream).into_body().into_data_stream();
        use futures::StreamExt;
        let first = tokio::time::timeout(Duration::from_secs(2), body.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(&first[..], b"first");
        assert!(!server.is_finished());
        release.send(()).unwrap();
        assert_eq!(&body.next().await.unwrap().unwrap()[..], b"last");
        server.await.unwrap();
    }
}
