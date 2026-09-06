use axum::{
    body::Body,
    extract::{Path, Query, State, WebSocketUpgrade},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use futures::{sink::SinkExt, stream::StreamExt};
use rand::Rng;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::{net::SocketAddr, sync::Arc};
use tokio::fs;
use tokio::process::Command;
use tokio_util::io::ReaderStream;
use tower_http::{cors::CorsLayer, services::ServeDir};

#[derive(Clone)]
struct AppState {
    client: Client,
    client_id: String,
    user_agent: String,
    base_url: Option<String>,
    version: String,
    opus_audio_bitrates: Vec<u32>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let client_id =
        std::env::var("CLIENTID").unwrap_or_else(|_| "kimne78kx3ncx6brgo4mv6wki5h1ko".to_string());
    let user_agent = std::env::var("USERAGENT").unwrap_or_else(|_| {
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36".to_string()
    });
    let base_url = std::env::var("INSTANCE_URL").ok();
    let opus_audio_bitrates = parse_opus_audio_bitrates(std::env::var("OPUS_AUDIO_BITRATES").ok());

    let pkg: Value =
        serde_json::from_str(&std::fs::read_to_string("package.json").unwrap_or_default())
            .unwrap_or_else(|_| json!({"version":"unknown"}));
    let version = pkg["version"].as_str().unwrap_or("unknown").to_string();

    let state = Arc::new(AppState {
        client: Client::builder().build().expect("client"),
        client_id,
        user_agent,
        base_url,
        version,
        opus_audio_bitrates,
    });

    let app = Router::new()
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
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr: SocketAddr = "0.0.0.0:3000".parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    tracing::info!("listening on {}", addr);
    axum::serve(listener, app).await.unwrap();
}

async fn root_or_ws(ws: Option<WebSocketUpgrade>) -> Response {
    if let Some(upgrade) = ws {
        return upgrade.on_upgrade(chat_socket).into_response();
    }
    index_file().await.into_response()
}

async fn chat_socket(stream: axum::extract::ws::WebSocket) {
    use axum::extract::ws::Message;
    let (mut sender, mut receiver) = stream.split();
    let first = receiver.next().await;
    let Some(Ok(Message::Text(cmd))) = first else {
        return;
    };
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.len() != 2 || parts[0] != "JOIN" || parts[1].contains(',') {
        let _ = sender.close().await;
        return;
    }
    let channel = parts[1].to_lowercase();
    let tws = tokio_tungstenite::connect_async("wss://irc-ws.chat.twitch.tv:443").await;
    let Ok((twitch_ws, _)) = tws else {
        return;
    };
    let (mut tw_send, mut tw_recv) = twitch_ws.split();

    for cap in [
        "CAP REQ :twitch.tv/membership",
        "CAP REQ :twitch.tv/tags",
        "CAP REQ :twitch.tv/commands",
        "PASS none",
        "NICK justinfan333333333333",
    ] {
        let _ = tw_send
            .send(tokio_tungstenite::tungstenite::Message::Text(cap.into()))
            .await;
    }
    let _ = tw_send
        .send(tokio_tungstenite::tungstenite::Message::Text(
            format!("JOIN #{channel}").into(),
        ))
        .await;
    let _ = sender.send(Message::Text("OK".into())).await;

    loop {
        tokio::select! {
            incoming = tw_recv.next() => {
                let Some(Ok(msg)) = incoming else { break; };
                if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
                    for line in text.lines().filter(|line| line.starts_with("PING ")) {
                        let pong = line.replacen("PING", "PONG", 1);
                        if tw_send.send(tokio_tungstenite::tungstenite::Message::Text(pong.into())).await.is_err() { return; }
                    }
                    if sender.send(Message::Text(text.to_string())).await.is_err() { break; }
                }
            }
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Ping(data))) => { if sender.send(Message::Pong(data)).await.is_err() { break; } }
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    }
    let _ = tw_send.close().await;
}

async fn index_file() -> impl IntoResponse {
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

fn parse_opus_audio_bitrates(raw: Option<String>) -> Vec<u32> {
    let Some(value) = raw else {
        return vec![];
    };
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("no") {
        return vec![];
    }

    let mut bitrates = BTreeSet::new();
    for item in trimmed.split(',').map(str::trim).filter(|x| !x.is_empty()) {
        if let Ok(v) = item.parse::<u32>() {
            if v > 0 {
                bitrates.insert(v);
            }
        }
    }
    bitrates.into_iter().collect()
}

async fn gql(state: &AppState, body: Value, mobile: bool) -> Result<Value, ()> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "Client-ID",
        HeaderValue::from_str(&state.client_id).unwrap(),
    );
    headers.insert(
        header::USER_AGENT,
        HeaderValue::from_str(&state.user_agent).unwrap(),
    );
    headers.insert(
        header::REFERER,
        HeaderValue::from_static(if mobile {
            "https://m.twitch.tv/"
        } else {
            "https://www.twitch.tv/"
        }),
    );
    headers.insert(
        header::ORIGIN,
        HeaderValue::from_static(if mobile {
            "https://m.twitch.tv/"
        } else {
            "https://www.twitch.tv/"
        }),
    );

    let req = state
        .client
        .post("https://gql.twitch.tv/gql")
        .headers(headers)
        .json(&body)
        .send()
        .await
        .map_err(|_| ())?;
    if req.status() != StatusCode::OK {
        return Err(());
    }
    req.json().await.map_err(|_| ())
}

fn invalid() -> Json<Value> {
    Json(json!({"invalid":true}))
}

async fn stream_info(
    State(state): State<Arc<AppState>>,
    Path(username): Path<String>,
) -> impl IntoResponse {
    let result = gql(&state, json!({
        "query": "query TwinrStreamInfo($login: String!) { user(login: $login) { profileImageURL(width: 70) broadcastSettings { title } stream { viewersCount game { name } } } }",
        "variables": {"login": username.to_lowercase()}
    }), false).await;
    let Ok(result) = result else {
        return (StatusCode::BAD_GATEWAY, Json(json!({"error": "Unable to load channel metadata"}))).into_response();
    };
    if result.get("errors").and_then(Value::as_array).is_some_and(|errors| !errors.is_empty()) {
        return (StatusCode::BAD_GATEWAY, Json(json!({"error": "Unable to load channel metadata"}))).into_response();
    }
    let Some(user) = result.pointer("/data/user").filter(|user| !user.is_null()) else {
        return invalid().into_response();
    };
    let Some(stream) = user.get("stream").filter(|stream| !stream.is_null()) else {
        return invalid().into_response();
    };
    Json(json!({
        "views": stream.get("viewersCount").cloned().unwrap_or(json!(0)),
        "game": stream.pointer("/game/name").cloned().unwrap_or(json!("")),
        "avatar": user.get("profileImageURL").cloned().unwrap_or(json!("")),
        "title": user.pointer("/broadcastSettings/title").cloned().unwrap_or(json!(username))
    })).into_response()
}

async fn streamer_info(
    State(state): State<Arc<AppState>>,
    Path(username): Path<String>,
) -> impl IntoResponse {
    let u = username.to_lowercase();
    let shell = gql(&state, json!({"operationName":"ChannelShell","variables":{"login":u},"extensions":{"persistedQuery":{"version":1,"sha256Hash":"580ab410bcd0c1ad194224957ae2241e5d252b2c5173d8e0cce9d32d5bb14efe"}}}), false).await;
    let home = gql(&state, json!({"operationName":"HomeOfflineCarousel","variables":{"channelLogin":u,"includeTrailerUpsell":false,"trailerUpsellVideoID":""},"extensions":{"persistedQuery":{"version":1,"sha256Hash":"84e25789b04ac4dcaefd673cfb4259d39d03c6422838d09a4ed2aaf9b67054d8"}}}), false).await;
    let (Ok(shell), Ok(home)) = (shell, home) else {
        return invalid().into_response();
    };
    let data = json!({
      "displayName": shell.pointer("/data/userOrError/displayName").cloned().unwrap_or(json!("")),
      "description": home.pointer("/data/user/description").cloned().unwrap_or(json!("")),
      "profileImageURL": shell.pointer("/data/userOrError/profileImageURL").cloned().unwrap_or(json!("")),
      "bannerImageURL": shell.pointer("/data/userOrError/bannerImageURL").cloned().unwrap_or(json!("")),
      "socialMedias": home.pointer("/data/user/channel/socialMedias").cloned().unwrap_or(json!([]))
    });
    ([(header::CACHE_CONTROL, "max-age=3600")], Json(data)).into_response()
}

async fn vod_info(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> impl IntoResponse {
    let meta = gql(&state, json!({"operationName":"ComscoreStreamingQuery","variables":{"channel":"","clipSlug":"","isClip":false,"isLive":false,"isVodOrCollection":true,"vodID":id},"extensions":{"persistedQuery":{"version":1,"sha256Hash":"e1edae8122517d013405f237ffcc124515dc6ded82480a88daef69c83b53ac01"}}}), false).await;
    let name = gql(&state, json!({"operationName":"VodChannelLoginQuery","variables":{"videoID":id},"extensions":{"persistedQuery":{"version":1,"sha256Hash":"0c5feea4dad2565508828f16e53fe62614edf015159df4b3bca33423496ce78e"}}}), false).await;
    let (Ok(meta), Ok(name)) = (meta, name) else {
        return invalid().into_response();
    };
    let login = name
        .pointer("/data/video/owner/login")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if login.is_empty() {
        return invalid().into_response();
    }
    let avatar = gql(&state, json!({"operationName":"ChannelShell","variables":{"login":login},"extensions":{"persistedQuery":{"version":1,"sha256Hash":"580ab410bcd0c1ad194224957ae2241e5d252b2c5173d8e0cce9d32d5bb14efe"}}}), false).await;
    let Ok(avatar) = avatar else {
        return invalid().into_response();
    };
    ([(header::CACHE_CONTROL, "max-age=3600")], Json(json!({
      "game": meta.pointer("/data/video/game/name").cloned().unwrap_or(json!("")),
      "avatar": avatar.pointer("/data/userOrError/profileImageURL").cloned().unwrap_or(json!("")),
      "title": meta.pointer("/data/video/title").cloned().unwrap_or(json!("")),
      "username": meta.pointer("/data/video/owner/displayName").cloned().unwrap_or(json!("")),
      "loginName": login
    }))).into_response()
}

async fn vod_comments(
    State(state): State<Arc<AppState>>,
    Path((id, offset)): Path<(String, String)>,
) -> impl IntoResponse {
    let res = gql(&state, json!({"operationName":"VideoCommentsByOffsetOrCursor","variables":{"videoID":id,"contentOffsetSeconds":offset.parse::<u64>().unwrap_or(0)},"extensions":{"persistedQuery":{"version":1,"sha256Hash":"b70a3591ff0f4e0313d126c6a1502d79a1c02baebb288227c582044aa76adf6a"}}}), false).await;
    let Ok(res) = res else {
        return invalid().into_response();
    };
    let edges = res
        .pointer("/data/video/comments/edges")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let data: Vec<Value> = edges
        .into_iter()
        .filter_map(|e| {
            let n = e.get("node")?;
            let commenter = n.pointer("/commenter/displayName")?.as_str()?.to_string();
            Some(json!({
                "offset": n.get("contentOffsetSeconds").cloned().unwrap_or(json!(0)),
                "username": commenter,
                "message": n.pointer("/message/fragments").and_then(Value::as_array).map(|items| items.iter().filter_map(|f| f.get("text").and_then(Value::as_str)).collect::<String>()).unwrap_or_default(),
                "fragments": n.pointer("/message/fragments").and_then(Value::as_array).map(|items| items.iter().map(|f| json!({"text": f.get("text").cloned().unwrap_or(json!("")), "emoteId": f.pointer("/emote/emoteID").cloned().unwrap_or(Value::Null)})).collect::<Vec<_>>()).unwrap_or_default(),
                "color": n.pointer("/message/userColor").cloned().unwrap_or(json!("#FFFFF"))
            }))
        })
        .collect();
    (
        [(header::CACHE_CONTROL, "max-age=3600")],
        Json(json!({"valid":true,"data":data})),
    )
        .into_response()
}

async fn vod_list(
    State(state): State<Arc<AppState>>,
    Path((username, filter, limit)): Path<(String, String, usize)>,
) -> impl IntoResponse {
    let broadcast_type = if filter == "ALL" {
        Value::Null
    } else {
        json!(filter)
    };
    let res = gql(&state, json!({"operationName":"FilterableVideoTower_Videos","variables":{"limit":limit,"channelOwnerLogin":username.to_lowercase(),"broadcastType":broadcast_type,"videoSort":"TIME"},"extensions":{"persistedQuery":{"version":1,"sha256Hash":"a937f1d22e269e39a03b509f65a7490f9fc247d7f83d6ac1421523e3b68042cb"}}}), false).await;
    let Ok(res) = res else {
        return invalid().into_response();
    };
    let edges = res
        .pointer("/data/user/videos/edges")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let vods: Vec<Value> = edges.into_iter().map(|v| v["node"].clone()).map(|n| json!({
      "id": n["id"], "previewThumbnailURL": n["previewThumbnailURL"], "game": n.pointer("/game/name").cloned().unwrap_or(json!("")),
      "publishedAt": n["publishedAt"], "title": n["title"], "viewCount": n["viewCount"], "lengthSeconds": n["lengthSeconds"]
    })).collect();
    (
        [(header::CACHE_CONTROL, "max-age=1800")],
        Json(json!({"vods": vods})),
    )
        .into_response()
}

async fn clip_info(
    State(state): State<Arc<AppState>>,
    Path((username, id)): Path<(String, String)>,
) -> impl IntoResponse {
    let metadata = gql(&state, json!({"operationName":"ClipMetadata","variables":{"channelLogin":username.to_lowercase(),"clipSlug":id},"extensions":{"persistedQuery":{"version":1,"sha256Hash":"ab70572e66f164789c87936a8291fd15e29adc2cea0114b02e60f17d60d6d154"}}}), false).await;
    let media = gql(&state, json!({"operationName":"VideoAccessToken_Clip","variables":{"slug":id},"extensions":{"persistedQuery":{"version":1,"sha256Hash":"36b89d2507fce29e5ca551df756d27c1cfe079e2609642b4390aa4c35796eb11"}}}), false).await;
    let (Ok(metadata), Ok(media)) = (metadata, media) else {
        return invalid().into_response();
    };
    let m = to_clip_media(&media);
    if m.is_empty() {
        return invalid().into_response();
    }
    ([(header::CACHE_CONTROL, "max-age=3600")], Json(json!({
      "metadata": {
        "avatar": metadata.pointer("/data/user/profileImageURL").cloned().unwrap_or(json!("")),
        "date": metadata.pointer("/data/clip/createdAt").cloned().unwrap_or(json!("")),
        "title": metadata.pointer("/data/clip/title").cloned().unwrap_or(json!("")),
        "views": metadata.pointer("/data/clip/viewCount").cloned().unwrap_or(json!(0)),
        "author": metadata.pointer("/data/clip/curator/displayName").cloned().unwrap_or(json!("")),
        "game": metadata.pointer("/data/clip/game/displayName").cloned().unwrap_or(json!(""))
      },
      "media": m
    }))).into_response()
}

fn to_clip_media(media: &Value) -> Vec<Value> {
    let token_sig = media
        .pointer("/data/clip/playbackAccessToken/signature")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let token_val = media
        .pointer("/data/clip/playbackAccessToken/value")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    media.pointer("/data/clip/videoQualities").and_then(|v| v.as_array()).cloned().unwrap_or_default().into_iter().filter_map(|q| {
        let source = q.get("sourceURL")?.as_str()?;
        let quality = q.get("quality")?.as_str()?;
        Some(json!({
            "quality": quality,
            "src": format!("/clipproxy/{}/{}/{}", urlencoding::encode(source), token_sig, urlencoding::encode(token_val)),
            "originalURL": format!("{}?sig={}&token={}", source, token_sig, urlencoding::encode(token_val))
        }))
    }).collect()
}

async fn clips_list(
    State(state): State<Arc<AppState>>,
    Path((username, filter, limit)): Path<(String, String, usize)>,
) -> impl IntoResponse {
    let res = gql(&state, json!({"operationName":"ClipsCards__User","variables":{"login":username.to_lowercase(),"limit":limit,"criteria":{"filter":filter}},"extensions":{"persistedQuery":{"version":1,"sha256Hash":"b73ad2bfaecfd30a9e6c28fada15bd97032c83ec77a0440766a56fe0bd632777"}}}), false).await;
    let Ok(res) = res else {
        return invalid().into_response();
    };
    let edges = res
        .pointer("/data/user/clips/edges")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let clips: Vec<Value> = edges.into_iter().map(|x| x["node"].clone()).map(|n| json!({
      "author": n.pointer("/curator/displayName").cloned().unwrap_or(json!("")),"slug":n["slug"],"title":n["title"],
      "viewCount":n["viewCount"],"thumbnailURL":n["thumbnailURL"],"createdAt":n["createdAt"],"durationSeconds":n["durationSeconds"],"game":n.pointer("/game/name").cloned().unwrap_or(json!(""))
    })).collect();
    Json(json!({"clips":clips})).into_response()
}

async fn user_info(
    State(state): State<Arc<AppState>>,
    Path(username): Path<String>,
) -> impl IntoResponse {
    let u = username.to_lowercase();
    let about = gql(&state, json!({"operationName":"ChannelRoot_AboutPanel","variables":{"channelLogin":u,"skipSchedule":true},"extensions":{"persistedQuery":{"sha256Hash":"6089531acef6c09ece01b440c41978f4c8dc60cb4fa0124c9a9d3f896709b6c6","version":1}}}), false).await;
    let stream_meta = gql(&state, json!({"operationName":"StreamMetadata","variables":{"channelLogin":u},"extensions":{"persistedQuery":{"sha256Hash":"a647c2a13599e5991e175155f798ca7f1ecddde73f7f341f39009c14dbf59962","version":1}}}), false).await;
    let shell = gql(&state, json!({"operationName":"ChannelShell","variables":{"login":u},"extensions":{"persistedQuery":{"version":1,"sha256Hash":"580ab410bcd0c1ad194224957ae2241e5d252b2c5173d8e0cce9d32d5bb14efe"}}}), false).await;
    let (Ok(about), Ok(stream_meta), Ok(shell)) = (about, stream_meta, shell) else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":{"status":500,"message":"fetch failed"},"data":null})),
        )
            .into_response();
    };
    Json(json!({"error":null,"data":{
      "id":about.pointer("/data/user/id").cloned().unwrap_or(json!("")),
      "description":about.pointer("/data/user/description").cloned().unwrap_or(json!("")),
      "displayName":about.pointer("/data/user/displayName").cloned().unwrap_or(json!("")),
      "avatar":about.pointer("/data/user/profileImageURL").cloned().unwrap_or(json!("")),
      "banner":shell.pointer("/data/userOrError/bannerImageURL").cloned().unwrap_or(json!("")),
      "followers":about.pointer("/data/user/followers/totalCount").cloned().unwrap_or(json!(0)),
      "socialMedias":about.pointer("/data/user/channel/socialMedias").cloned().unwrap_or(json!([])),
      "live":stream_meta.pointer("/data/user/stream").map(|x| !x.is_null()).unwrap_or(false)
    }}))
    .into_response()
}

async fn emotes(
    State(state): State<Arc<AppState>>,
    Path(username): Path<String>,
) -> impl IntoResponse {
    let user = gql(&state, json!({"operationName":"ChannelRoot_AboutPanel","variables":{"channelLogin":username.to_lowercase(),"skipSchedule":true},"extensions":{"persistedQuery":{"sha256Hash":"6089531acef6c09ece01b440c41978f4c8dc60cb4fa0124c9a9d3f896709b6c6","version":1}}}), false).await;
    let Ok(user) = user else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"status":400,"message":"Invalid user"})),
        )
            .into_response();
    };
    let uid = user
        .pointer("/data/user/id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if uid.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"status":400,"message":"Invalid user"})),
        )
            .into_response();
    }
    let list = gql(&state, json!({"operationName":"EmotePicker_EmotePicker_UserSubscriptionProducts","variables":{"channelOwnerID":uid},"extensions":{"persistedQuery":{"version":1,"sha256Hash":"71b5f829a4576d53b714c01d3176f192cbd0b14973eb1c3d0ee23d5d1b78fd7e"}}}), true).await;
    let Ok(list) = list else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"status":400,"message":"Invalid user"})),
        )
            .into_response();
    };

    let mut out = vec![];
    for e in list
        .pointer("/data/channel/localEmoteSets")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
    {
        for em in e
            .get("emotes")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
        {
            if let Some(id) = em.get("id").and_then(|v| v.as_str()) {
                out.push(json!({"id":id,"token":em.get("token").cloned().unwrap_or(json!("")),"url":format!("/api/proxy?url={}", STANDARD.encode(format!("https://static-cdn.jtvnw.net/emoticons/v2/{id}/default/dark/2.0")))}));
            }
        }
    }
    (
        [(header::CACHE_CONTROL, "max-age=3600, public")],
        Json(json!({"data":out})),
    )
        .into_response()
}

#[derive(Deserialize)]
struct QualityQuery {
    quality: Option<String>,
}

enum QualityPreference {
    Auto,
    Height(u32),
    AudioOnly,
    AudioOpus(u32),
}

fn parse_quality_preference(value: Option<&str>) -> QualityPreference {
    match value {
        Some(v) if v.eq_ignore_ascii_case("audio_only") => QualityPreference::AudioOnly,
        Some(v) if v.starts_with("audio_opus_") => v
            .trim_start_matches("audio_opus_")
            .parse::<u32>()
            .map(QualityPreference::AudioOpus)
            .unwrap_or(QualityPreference::Auto),
        Some(v) => v
            .parse::<u32>()
            .map(QualityPreference::Height)
            .unwrap_or(QualityPreference::Auto),
        None => QualityPreference::Auto,
    }
}

fn spawn_opus_transcode_response(source_url: &str, bitrate: u32) -> Response {
    let mut cmd = Command::new("ffmpeg");
    cmd.args([
        "-hide_banner",
        "-loglevel",
        "error",
        "-nostdin",
        "-i",
        source_url,
        "-vn",
        "-c:a",
        "libopus",
        "-b:a",
        &format!("{bitrate}k"),
        "-vbr",
        "on",
        "-application",
        "audio",
        "-f",
        "ogg",
        "pipe:1",
    ]);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::null());

    let Ok(mut child) = cmd.spawn() else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":true,"data":null})),
        )
            .into_response();
    };
    let Some(stdout) = child.stdout.take() else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":true,"data":null})),
        )
            .into_response();
    };
    tokio::spawn(async move {
        let _ = child.wait().await;
    });

    let stream = ReaderStream::new(stdout);
    (
        [
            (header::CONTENT_TYPE, "audio/ogg"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        Body::from_stream(stream),
    )
        .into_response()
}

async fn stream_proxy(
    State(state): State<Arc<AppState>>,
    Path(username): Path<String>,
    Query(query): Query<QualityQuery>,
) -> impl IntoResponse {
    let quality = parse_quality_preference(query.quality.as_deref());
    let device = uuid::Uuid::new_v4().to_string();
    let token_req = state.client.post("https://gql.twitch.tv/gql")
      .header("Client-Id", &state.client_id)
      .header(header::USER_AGENT, &state.user_agent)
      .header(header::REFERER, "https://m.twitch.tv/")
      .header(header::ORIGIN, "https://m.twitch.tv/")
      .header("Device-Id", &device)
      .json(&json!({"query":"query StreamPlayer_Query($login: String!, $playerType: String!, $platform: String!, $skipPlayToken: Boolean!) { user(login: $login) { stream @skip(if: $skipPlayToken) { playbackAccessToken(params: {platform: $platform, playerType: $playerType}) { signature value } } } }","variables":{"login":username.to_lowercase(),"playerType":"pulsar","platform":"mobile_web","skipPlayToken":false}}))
      .send().await;
    let Ok(token_req) = token_req else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":true,"data":null})),
        )
            .into_response();
    };
    let tr: Value = token_req.json().await.unwrap_or_default();
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
            select_playlist(&list_text, &QualityPreference::Auto).unwrap_or_default();
        let selected = resolve_playlist_url(&url, &selected_ref);
        if selected.is_empty() {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error":true,"data":null})),
            )
                .into_response();
        }
        return spawn_opus_transcode_response(&selected, bitrate);
    }

    if matches!(
        quality,
        QualityPreference::Auto
    ) {
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

async fn vod_proxy(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<QualityQuery>,
) -> impl IntoResponse {
    let quality = parse_quality_preference(query.quality.as_deref());
    let token = gql(&state, vod_token_request(&id), false).await;
    let Ok(token) = token else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":true,"data":null})),
        )
            .into_response();
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
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":true,"data":null})),
        )
            .into_response();
    }

    let p: u32 = rand::thread_rng().gen_range(1..=99999);
    let playlist_url = format!("https://usher.ttvnw.net/vod/{id}.m3u8?acmb=e30%3D&allow_source=true&allow_audio_only=true&p={p}&cdm=wv&transcode_mode=cbr_v1&supported_codecs=avc1&player_version=1.19.0&player_base=mediaplayer&reassignments_supported=true&playlist_include_framerate=true&player_backend=mediaplayer&token={}&sig={}", urlencoding::encode(val), sig);
    let list_text = match fetch_raw_text(&state, &playlist_url, false).await {
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
            select_playlist(&list_text, &QualityPreference::Auto).unwrap_or_default();
        let selected = resolve_playlist_url(&playlist_url, &selected_ref);
        if selected.is_empty() {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error":true,"data":null})),
            )
                .into_response();
        }
        return spawn_opus_transcode_response(&selected, bitrate);
    }

    if matches!(quality, QualityPreference::Auto) {
        return ([(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")], proxy_vod_manifest(&playlist_url, &list_text)).into_response();
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

fn vod_token_request(id: &str) -> Value {
    json!({
        "query": "query VodPlaybackAccessToken($vodID: ID!, $playerType: String!) { videoPlaybackAccessToken(id: $vodID, params: {platform: \"web\", playerBackend: \"mediaplayer\", playerType: $playerType}) { value signature } }",
        "variables": {"vodID": id, "playerType": "site"}
    })
}

fn manifest_proxy_url(source: &str, reference: &str, playlist: bool) -> String {
    format!("/api/{}?url={}", if playlist { "playlist" } else { "proxy" },
        urlencoding::encode(&STANDARD.encode(resolve_playlist_url(source, reference))))
}

fn proxy_vod_manifest(source_url: &str, manifest: &str) -> String {
    let master = manifest.contains("#EXT-X-STREAM-INF:");
    manifest.lines().map(|line| {
        if !line.starts_with('#') && !line.trim().is_empty() {
            return manifest_proxy_url(source_url, line.trim(), master);
        }
        if let Some(start) = line.find("URI=\"") {
            let start = start + 5;
            if let Some(length) = line[start..].find('"') {
                let end = start + length;
                let playlist = line.starts_with("#EXT-X-MEDIA:") || line.starts_with("#EXT-X-I-FRAME-STREAM-INF:") || line.starts_with("#EXT-X-RENDITION-REPORT:");
                return format!("{}{}{}", &line[..start], manifest_proxy_url(source_url, &line[start..end], playlist), &line[end..]);
            }
        }
        line.to_string()
    }).collect::<Vec<_>>().join("\n")
}

async fn playlist_proxy(State(state): State<Arc<AppState>>, Query(q): Query<UrlQ>) -> Response {
    let decoded = q.url.and_then(|s| STANDARD.decode(s.replace(' ', "+")).ok()).and_then(|b| String::from_utf8(b).ok());
    let Some(url) = decoded else { return (StatusCode::BAD_REQUEST, "Invalid playlist URL").into_response(); };
    match fetch_raw_text(&state, &url, false).await {
        Ok(manifest) => ([(header::CONTENT_TYPE, "application/vnd.apple.mpegurl"), (header::CACHE_CONTROL, "no-store")], proxy_vod_manifest(&url, &manifest)).into_response(),
        Err(response) => response,
    }
}

fn select_playlist(manifest: &str, quality: &QualityPreference) -> Option<String> {
    let lines: Vec<&str> = manifest.lines().collect();
    match quality {
        QualityPreference::Height(target_height) => {
            for i in 0..lines.len().saturating_sub(1) {
                if lines[i].split(',').filter_map(|attribute| attribute.strip_prefix("RESOLUTION=")).any(|resolution| resolution.rsplit_once('x').and_then(|(_, height)| height.parse::<u32>().ok()) == Some(*target_height))
                {
                    let next = lines[i + 1].trim();
                    if !next.is_empty() && !next.starts_with('#') {
                        return Some(next.to_string());
                    }
                }
            }
        }
        QualityPreference::AudioOnly => {
            for pair in lines.windows(2) {
                if pair[0].starts_with("#EXT-X-STREAM-INF:") && !pair[0].contains("RESOLUTION=") {
                    return Some(pair[1].trim().to_string());
                }
            }
        }
        QualityPreference::Auto | QualityPreference::AudioOpus(_) => {}
    }

    lines
        .iter()
        .find(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|x| x.trim().to_string())
}

fn resolve_playlist_url(source_url: &str, selected_ref: &str) -> String {
    if selected_ref.is_empty() {
        return String::new();
    }
    reqwest::Url::parse(source_url)
        .and_then(|base| base.join(selected_ref))
        .map(|url| url.to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod media_tests {
    use super::*;

    #[test]
    fn adaptive_master_preserves_levels_and_proxies_nested_playlists() {
        let source = "https://cdn.example/vod/master.m3u8";
        let manifest = "#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"audio\",URI=\"audio/index.m3u8\"\n#EXT-X-STREAM-INF:BANDWIDTH=6000000,RESOLUTION=1920x1080\nchunked/index.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=800000,RESOLUTION=640x360\n360/index.m3u8\n";
        let rewritten = proxy_vod_manifest(source, manifest);
        assert_eq!(rewritten.matches("/api/playlist?url=").count(), 3);
        assert!(rewritten.contains("RESOLUTION=1920x1080"));
        assert!(rewritten.contains("RESOLUTION=640x360"));
        assert!(rewritten.contains(&manifest_proxy_url(source, "audio/index.m3u8", true)));
    }

    #[test]
    fn media_key_and_initialization_uris_are_proxied() {
        let source = "https://cdn.example/video/index.m3u8";
        let rewritten = proxy_vod_manifest(source, "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"../key\"\n#EXT-X-MAP:URI=\"init.mp4\"\n#EXTINF:2,\n0.m4s");
        assert_eq!(rewritten.matches("/api/proxy?url=").count(), 3);
        assert!(rewritten.contains(&manifest_proxy_url(source, "../key", false)));
        assert!(rewritten.contains(&manifest_proxy_url(source, "init.mp4", false)));
    }

    #[test]
    fn vod_segments_resolve_for_muted_and_regular_playlists() {
        for filename in ["index-dvr.m3u8", "index-muted-AC62XD2A6L.m3u8"] {
            let source = format!("https://cdn.example/vod/chunked/{filename}?token=test");
            let manifest = "#EXTM3U\n#EXTINF:10.0,\n0.ts\n#EXTINF:10.0,\n1-muted.ts\n#EXT-X-ENDLIST\n";
            let rewritten = proxy_vod_manifest(&source, manifest);
            let urls: Vec<String> = rewritten.lines().filter(|line| !line.starts_with('#'))
                .map(|line| {
                    let encoded = urlencoding::decode(line.strip_prefix("/api/proxy?url=").unwrap()).unwrap();
                    String::from_utf8(STANDARD.decode(encoded.as_bytes()).unwrap()).unwrap()
                }).collect();
            assert_eq!(urls, ["https://cdn.example/vod/chunked/0.ts", "https://cdn.example/vod/chunked/1-muted.ts"]);
            assert!(rewritten.contains("#EXT-X-ENDLIST"));
        }
    }

    #[test]
    fn playlist_references_follow_url_resolution_rules() {
        let source = "https://cdn.example/vod/chunked/index.m3u8?old=1";
        for (reference, expected) in [
            ("../audio/0.ts", "https://cdn.example/vod/audio/0.ts"),
            ("/shared/0.ts?x=1&y=2", "https://cdn.example/shared/0.ts?x=1&y=2"),
            ("https://other.example/0.ts", "https://other.example/0.ts"),
            ("//other.example/0.ts", "https://other.example/0.ts"),
            ("", ""),
        ] {
            assert_eq!(resolve_playlist_url(source, reference), expected);
        }
    }

    #[tokio::test]
    async fn media_proxy_forwards_first_chunk_before_upstream_finishes() {
        let (release, wait) = tokio::sync::oneshot::channel::<()>();
        let wait = Arc::new(tokio::sync::Mutex::new(Some(wait)));
        let app = Router::new().route("/segment", get(move || {
            let wait = wait.clone();
            async move {
                let wait = wait.lock().await.take().unwrap();
                let chunks = futures::stream::once(async { Ok::<_, std::io::Error>("first") })
                    .chain(futures::stream::once(async move {
                        let _ = wait.await;
                        Ok::<_, std::io::Error>("last")
                    }));
                ([(header::CONTENT_TYPE, "video/mp2t")], Body::from_stream(chunks))
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let state = AppState {
            client: Client::new(), client_id: String::new(), user_agent: "test".into(),
            base_url: None, version: "test".into(), opus_audio_bitrates: vec![],
        };
        let response = tokio::time::timeout(std::time::Duration::from_secs(2),
            pipe_url(&state, &format!("http://{address}/segment"), "", ""))
            .await.expect("proxy must return before the upstream body completes");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "video/mp2t");
        let mut body = response.into_body().into_data_stream();
        let first = tokio::time::timeout(std::time::Duration::from_secs(2), body.next())
            .await.unwrap().unwrap().unwrap();
        assert_eq!(&first[..], b"first");
        release.send(()).unwrap();
        let last = body.next().await.unwrap().unwrap();
        assert_eq!(&last[..], b"last");
        assert!(body.next().await.is_none());
        server.abort();
    }
}

async fn fetch_raw_text(state: &AppState, url: &str, mobile: bool) -> Result<String, Response> {
    let req = state
        .client
        .get(url)
        .header(header::USER_AGENT, &state.user_agent)
        .header(
            header::REFERER,
            if mobile {
                "https://m.twitch.tv"
            } else {
                "https://player.twitch.tv"
            },
        )
        .header(
            header::ORIGIN,
            if mobile {
                "https://m.twitch.tv"
            } else {
                "https://player.twitch.tv"
            },
        )
        .header("Client-ID", &state.client_id)
        .send()
        .await;
    let Ok(resp) = req else {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":true,"data":null})),
        )
            .into_response());
    };
    if resp.status() != StatusCode::OK {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":true,"data":null})),
        )
            .into_response());
    }
    let t = resp.text().await.unwrap_or_default();
    Ok(t)
}

#[derive(Deserialize)]
struct UrlQ {
    url: Option<String>,
}

async fn urlproxy(State(state): State<Arc<AppState>>, Query(q): Query<UrlQ>) -> impl IntoResponse {
    let Some(url) = q.url else {
        return (StatusCode::BAD_REQUEST, Json(json!({"invalid":true}))).into_response();
    };
    pipe_url(
        &state,
        &url,
        "https://player.twitch.tv",
        "https://player.twitch.tv",
    )
    .await
}

async fn clip_proxy(
    State(state): State<Arc<AppState>>,
    Path((media, sig, token)): Path<(String, String, String)>,
) -> impl IntoResponse {
    let url = format!(
        "{}?sig={}&token={}",
        urlencoding::decode(&media).unwrap_or_default(),
        sig,
        urlencoding::encode(&token)
    );
    pipe_url(
        &state,
        &url,
        "https://player.twitch.tv",
        "https://player.twitch.tv",
    )
    .await
}

async fn proxy(State(state): State<Arc<AppState>>, Query(q): Query<UrlQ>) -> impl IntoResponse {
    let Some(encoded) = q.url else {
        return (StatusCode::BAD_REQUEST, "No url provided.").into_response();
    };
    let normalized = encoded.replace(' ', "+");
    let decoded_bytes = STANDARD.decode(normalized.as_bytes()).unwrap_or_default();
    let decoded = String::from_utf8(decoded_bytes).unwrap_or_default();
    pipe_url(
        &state,
        &decoded,
        "https://www.twitch.tv",
        "https://www.twitch.tv",
    )
    .await
}

async fn pipe_url(state: &AppState, url: &str, referer: &str, origin: &str) -> Response {
    let resp = state
        .client
        .get(url)
        .header(header::USER_AGENT, &state.user_agent)
        .header(header::REFERER, referer)
        .header(header::ORIGIN, origin)
        .header("Client-ID", &state.client_id)
        .send()
        .await;
    let Ok(resp) = resp else {
        return (StatusCode::BAD_REQUEST, "err").into_response();
    };
    if resp.status() != StatusCode::OK {
        return (StatusCode::BAD_REQUEST, Json(json!({"invalid":true}))).into_response();
    }
    let status = resp.status();
    let ct = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .cloned()
        .unwrap_or(HeaderValue::from_static("application/octet-stream"));
    let cc = resp.headers().get(header::CACHE_CONTROL).cloned();

    let mut out_headers = HeaderMap::new();
    out_headers.insert(header::CONTENT_TYPE, ct);
    if let Some(ccv) = cc {
        out_headers.insert(header::CACHE_CONTROL, ccv);
    }
    // Forward chunks as they arrive, with downstream backpressure, instead of
    // retaining an entire segment (or clip) before sending its first byte.
    let stream = futures::stream::try_unfold(resp, |mut response| async move {
        response.chunk().await.map(|chunk| chunk.map(|bytes| (bytes, response)))
    });
    (status, out_headers, Body::from_stream(stream)).into_response()
}

#[derive(Deserialize)]
struct ClipPageQuery {
    quality: Option<String>,
    embed: Option<String>,
}

async fn clip_page_or_index(
    State(state): State<Arc<AppState>>,
    Path((username, id)): Path<(String, String)>,
    Query(query): Query<ClipPageQuery>,
) -> impl IntoResponse {
    if query.embed.is_none() {
        return index_file().await.into_response();
    }

    let metadata = gql(&state, json!({"operationName":"ClipMetadata","variables":{"channelLogin":username.to_lowercase(),"clipSlug":id},"extensions":{"persistedQuery":{"version":1,"sha256Hash":"ab70572e66f164789c87936a8291fd15e29adc2cea0114b02e60f17d60d6d154"}}}), false).await;
    let media = gql(&state, json!({"operationName":"VideoAccessToken_Clip","variables":{"slug":id},"extensions":{"persistedQuery":{"version":1,"sha256Hash":"36b89d2507fce29e5ca551df756d27c1cfe079e2609642b4390aa4c35796eb11"}}}), false).await;
    let (Ok(metadata), Ok(media)) = (metadata, media) else {
        return Html(render_clip_invalid(&state.version)).into_response();
    };
    let media_vec = to_clip_media(&media);
    if media_vec.is_empty() {
        return Html(render_clip_invalid(&state.version)).into_response();
    }
    let chosen = query
        .quality
        .as_deref()
        .and_then(|q| {
            media_vec
                .iter()
                .find(|m| m.get("quality").and_then(|v| v.as_str()) == Some(q))
        })
        .cloned()
        .unwrap_or_else(|| media_vec[0].clone());

    Html(render_clip_page(
        &state,
        &username.to_lowercase(),
        &id,
        chosen.get("src").and_then(|v| v.as_str()).unwrap_or(""),
        &metadata,
    ))
    .into_response()
}

fn render_clip_invalid(version: &str) -> String {
    format!("<!doctype html><html><head><title>Twinr - Clip</title><link rel=\"stylesheet\" href=\"/styles.min.css\"></head><body><div class=\"container\"><h1>Not found</h1><p>Clip not found.</p></div><footer><p>Twinr Version {version}</p></footer></body></html>")
}

fn render_clip_page(
    state: &AppState,
    username: &str,
    _slug: &str,
    src: &str,
    metadata: &Value,
) -> String {
    let title = metadata
        .pointer("/data/clip/title")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let game = metadata
        .pointer("/data/clip/game/displayName")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let author = metadata
        .pointer("/data/clip/curator/displayName")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let views = metadata
        .pointer("/data/clip/viewCount")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let date = metadata
        .pointer("/data/clip/createdAt")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let avatar = metadata
        .pointer("/data/user/profileImageURL")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let video_tags = state.base_url.as_ref().map(|base| format!("<meta name=\"twitter:card\" content=\"player\" /><meta property=\"og:video\" content=\"{base}{src}\" />")).unwrap_or_default();

    format!("<!doctype html><html><head><meta charset=\"UTF-8\" /><title>Twinr - Clip {title}</title>{video_tags}<link rel=\"stylesheet\" href=\"/styles.min.css\"><link rel=\"stylesheet\" href=\"/poppins.css\"></head><body><div class=\"container\"><video controls src=\"{src}\"></video><span id=\"date\"></span><h3>{title}</h3><div>{game}</div><div><span>By {author}</span> <span>{views} views</span></div><div><a href=\"/{username}?home=true\"><img class=\"w-8 rounded-full\" src=\"/api/urlproxy?url={avatar}\" /></a><a href=\"/{username}?home=true\">{username}</a></div></div><script>const date=Date.parse('{date}')-Date.now(),sec=Math.abs(Math.floor(date/1000)),min=Math.abs(Math.floor(sec/60)),hours=Math.abs(Math.floor(min/60)),days=Math.abs(Math.floor(hours/24));document.getElementById('date').innerText=`${{days}} days, ${{hours%24}} hours, ${{min%60}} minutes, and ${{sec%60}} seconds ago`;</script><footer><p>Twinr Version {} - <a href=\"https://github.com/Gevroska/twinr\">Source</a></p></footer></body></html>", state.version)
}
