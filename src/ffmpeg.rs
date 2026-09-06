use crate::{
    errors::AppError, hls::rewrite_with, proxy::fetch_raw_text, security, state::AppState,
};
use axum::{
    body::{Body, Bytes},
    extract::{Path, Query, State},
    http::{header, HeaderValue},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use futures::StreamExt;
use serde::Deserialize;
use std::{process::Stdio, sync::Arc, time::Duration};
use tokio::{
    process::{Child, Command},
    sync::OwnedSemaphorePermit,
    task::JoinHandle,
};
use tokio_util::io::ReaderStream;
#[derive(Clone)]
struct Gateway {
    state: Arc<AppState>,
    secret: String,
    base: String,
}
#[derive(Deserialize)]
struct Resource {
    url: String,
}
async fn resource(
    State(g): State<Gateway>,
    Path((secret, kind)): Path<(String, String)>,
    Query(q): Query<Resource>,
    headers: axum::http::HeaderMap,
) -> Response {
    if secret != g.secret {
        return AppError::ForbiddenUrl.into_response();
    }
    let kind = kind.split('.').next().unwrap_or("");
    let Ok(bytes) = STANDARD.decode(&q.url) else {
        return AppError::InvalidInput.into_response();
    };
    let Ok(url) = String::from_utf8(bytes) else {
        return AppError::InvalidInput.into_response();
    };
    if security::validate_url(&url).is_err() {
        return AppError::ForbiddenUrl.into_response();
    }
    if kind == "playlist" {
        match fetch_raw_text(&g.state, &url, false).await {
            Ok(manifest) => (
                [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")],
                rewrite_with(&manifest, |r, p| {
                    gateway_url(&g.base, &crate::hls::resolve_playlist_url(&url, r), p)
                }),
            )
                .into_response(),
            Err(response) => response,
        }
    } else if kind == "media" {
        crate::proxy::pipe_url_range(&g.state, &url, &headers).await
    } else {
        AppError::InvalidInput.into_response()
    }
}
fn gateway_url(base: &str, url: &str, playlist: bool) -> String {
    // FFmpeg checks segment extensions separately from playlist extensions.
    // Keep the source extension visible despite carrying the URL in a query.
    let parsed = reqwest::Url::parse(url).ok();
    let extension = parsed
        .as_ref()
        .and_then(|u| u.path().rsplit('.').next())
        .filter(|ext| {
            !ext.is_empty() && ext.len() <= 8 && ext.bytes().all(|c| c.is_ascii_alphanumeric())
        })
        .unwrap_or("ts");
    format!(
        "{base}/{}.{}?url={}",
        if playlist { "playlist" } else { "media" },
        if playlist { "m3u8" } else { extension },
        urlencoding::encode(&STANDARD.encode(url))
    )
}

pub(crate) enum Output {
    Opus(u32),
    Download,
}
fn output_args(output: &Output) -> Vec<String> {
    match output {
        Output::Opus(bitrate) => vec![
            "-map".into(),
            "0:a:0".into(),
            "-vn".into(),
            "-c:a".into(),
            "libopus".into(),
            "-b:a".into(),
            format!("{bitrate}k"),
            "-vbr".into(),
            "on".into(),
            "-f".into(),
            "ogg".into(),
            "-flush_packets".into(),
            "1".into(),
        ],
        Output::Download => [
            "-map",
            "0:v:0?",
            "-map",
            "0:a:0?",
            "-c",
            "copy",
            "-movflags",
            "frag_keyframe+empty_moov+default_base_moof",
            "-f",
            "mp4",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect(),
    }
}
struct Job {
    reader: ReaderStream<tokio::process::ChildStdout>,
    child: Option<Child>,
    permit: Option<OwnedSemaphorePermit>,
    gateway: JoinHandle<()>,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.gateway.abort();
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
            let permit = self.permit.take();
            tokio::spawn(async move {
                let _ = child.wait().await;
                drop(permit);
            });
        }
    }
}
pub(crate) async fn spawn_opus_transcode_response(
    state: Arc<AppState>,
    source: &str,
    bitrate: u32,
) -> Response {
    process(state, source, Output::Opus(bitrate), None)
        .await
        .unwrap_or_else(IntoResponse::into_response)
}
pub(crate) async fn process(
    state: Arc<AppState>,
    source: &str,
    output: Output,
    filename: Option<String>,
) -> Result<Response, AppError> {
    security::validate_url(source)?;
    let permit = state
        .ffmpeg_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| AppError::Busy)?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|_| AppError::Transcode)?;
    let secret = uuid::Uuid::new_v4().to_string();
    let base = format!(
        "http://{}/{}",
        listener.local_addr().map_err(|_| AppError::Transcode)?,
        secret
    );
    let gateway = Gateway {
        state,
        secret,
        base: base.clone(),
    };
    let app = Router::new()
        .route("/:secret/:kind", get(resource))
        .with_state(gateway);
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    let mut command = Command::new("ffmpeg");
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-protocol_whitelist",
            "http,tcp,crypto",
            "-rw_timeout",
            "30000000",
            "-allowed_extensions",
            "ALL",
            "-i",
        ])
        .arg(gateway_url(&base, source, true))
        .args(output_args(&output))
        .arg("pipe:1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(_) => {
            server.abort();
            return Err(AppError::Transcode);
        }
    };
    let Some(stdout) = child.stdout.take() else {
        server.abort();
        let _ = child.start_kill();
        let _ = child.wait().await;
        return Err(AppError::Transcode);
    };
    let mut job = Job {
        reader: ReaderStream::with_capacity(stdout, 64 * 1024),
        child: Some(child),
        permit: Some(permit),
        gateway: server,
    };
    let first = tokio::time::timeout(Duration::from_secs(30), job.reader.next())
        .await
        .map_err(|_| AppError::Timeout)?
        .ok_or(AppError::Transcode)?
        .map_err(|_| AppError::Transcode)?;
    let remainder = futures::stream::try_unfold(job, |mut job| async move {
        match tokio::time::timeout(Duration::from_secs(60), job.reader.next())
            .await
            .map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::TimedOut, "FFmpeg output stalled")
            })? {
            Some(Ok(chunk)) => Ok(Some((chunk, job))),
            Some(Err(error)) => Err(error),
            None => {
                let status = job.child.as_mut().expect("owned child").wait().await?;
                if !status.success() {
                    return Err(std::io::Error::other("FFmpeg failed"));
                }
                Ok(None)
            }
        }
    });
    let stream =
        futures::stream::once(async { Ok::<Bytes, std::io::Error>(first) }).chain(remainder);
    let content_type = match output {
        Output::Opus(_) => "audio/ogg",
        Output::Download => "video/mp4",
    };
    let mut response = (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "no-store"),
            (header::HeaderName::from_static("x-accel-buffering"), "no"),
        ],
        Body::from_stream(stream),
    )
        .into_response();
    if let Some(filename) = filename {
        response.headers_mut().insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
                .map_err(|_| AppError::InvalidInput)?,
        );
    }
    Ok(response)
}
pub(crate) fn filename(title: &str, id: &str) -> String {
    let clean: String = title
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        .take(100)
        .collect();
    let clean = clean.trim();
    format!(
        "{}-{id}.mp4",
        if clean.is_empty() { "twinr-vod" } else { clean }
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_gateway_preserves_ffmpeg_segment_extensions() {
        for (url, extension) in [
            ("https://cdn.ttvnw.net/0-muted.ts?sig=x", "ts"),
            ("https://cdn.ttvnw.net/init.mp4", "mp4"),
            ("https://cdn.ttvnw.net/segment.m4s", "m4s"),
        ] {
            assert!(gateway_url("http://127.0.0.1/secret", url, false)
                .starts_with(&format!("http://127.0.0.1/secret/media.{extension}?url=")));
        }
        assert!(gateway_url(
            "http://127.0.0.1/secret",
            "https://cdn.ttvnw.net/index.m3u8",
            true
        )
        .contains("/playlist.m3u8?"));
    }
    #[test]
    fn download_copies_and_opus_maps_audio() {
        let args = output_args(&Output::Download);
        assert!(args.windows(2).any(|p| p == ["-c", "copy"]));
        assert!(args.contains(&"frag_keyframe+empty_moov+default_base_moof".into()));
        let args = output_args(&Output::Opus(64));
        assert!(args.windows(2).any(|p| p == ["-map", "0:a:0"]));
        assert!(args.contains(&"64k".into()));
        assert_eq!(filename("bad\r\n\"/title", "123"), "badtitle-123.mp4");
    }
    #[test]
    #[ignore = "subprocess fixture, launched by cleanup test"]
    fn child_fixture() {
        println!("ready");
        std::thread::sleep(Duration::from_secs(60));
    }
    #[tokio::test]
    async fn disconnect_reaps_child_before_releasing_permit() {
        let slots = Arc::new(tokio::sync::Semaphore::new(1));
        let permit = slots.clone().try_acquire_owned().unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "ffmpeg::tests::child_fixture",
                "--ignored",
                "--nocapture",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let reader = ReaderStream::new(child.stdout.take().unwrap());
        let gateway = tokio::spawn(std::future::pending());
        let mut job = Job {
            reader,
            child: Some(child),
            permit: Some(permit),
            gateway,
        };
        tokio::time::timeout(Duration::from_secs(5), job.reader.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(slots.clone().try_acquire_owned().is_err());
        drop(job);
        let permit = tokio::time::timeout(Duration::from_secs(5), slots.acquire())
            .await
            .unwrap()
            .unwrap();
        drop(permit);
        assert_eq!(slots.available_permits(), 1);
    }
}
