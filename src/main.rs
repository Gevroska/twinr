use axum::{
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
use std::{net::SocketAddr, sync::Arc};
use tokio::fs;
use tower_http::{cors::CorsLayer, services::ServeDir};

#[derive(Clone)]
struct AppState {
    client: Client,
    client_id: String,
    user_agent: String,
    base_url: Option<String>,
    version: String,
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

    while let Some(Ok(msg)) = tw_recv.next().await {
        if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
            let _ = sender.send(Message::Text(text.to_string())).await;
        }
    }
}

async fn index_file() -> impl IntoResponse {
    match fs::read_to_string("public/index.html").await {
        Ok(s) => Html(s).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "index.html missing").into_response(),
    }
}

async fn api_root(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(json!({"version": state.version, "api":"v0"}))
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
    let u = username.to_lowercase();
    let cat = gql(&state, json!({"operationName":"SignupPromptCategory","variables":{"channelLogin":u,"isLive":true,"isVod":false,"videoID":""},"extensions":{"persistedQuery":{"version":1,"sha256Hash":"21c86683bbfd1a6e9e6636c2b460f94c5014272dcb56f0aa04a7d28d0633502c"}}}), false).await;
    let avatar = gql(&state, json!({"operationName":"ChannelShell","variables":{"login":u},"extensions":{"persistedQuery":{"version":1,"sha256Hash":"580ab410bcd0c1ad194224957ae2241e5d252b2c5173d8e0cce9d32d5bb14efe"}}}), false).await;
    let title = gql(&state, json!({"operationName":"ComscoreStreamingQuery","variables":{"channel":u,"clipSlug":"","isClip":false,"isLive":true,"isVodOrCollection":false,"vodID":""},"extensions":{"persistedQuery":{"version":1,"sha256Hash":"e1edae8122517d013405f237ffcc124515dc6ded82480a88daef69c83b53ac01"}}}), false).await;
    let (Ok(cat), Ok(avatar), Ok(title)) = (cat, avatar, title) else {
        return invalid().into_response();
    };

    let user_id = cat
        .pointer("/data/user/id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let game = cat
        .pointer("/data/user/stream/game/name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let av = avatar
        .pointer("/data/userOrError/profileImageURL")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let t = title
        .pointer("/data/user/broadcastSettings/title")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if user_id.is_empty() || game.is_empty() || av.is_empty() || t.is_empty() {
        return invalid().into_response();
    }

    let views = gql(&state, json!({"query": format!("query UseViewCount {{ user(id: {}) {{ stream {{ viewersCount }} }} }}", user_id),"variables":{}}), false).await;
    let Ok(views) = views else {
        return invalid().into_response();
    };
    let v = views
        .pointer("/data/user/stream/viewersCount")
        .and_then(|x| x.as_i64())
        .unwrap_or(-1);
    if v < 0 {
        return invalid().into_response();
    }

    Json(json!({"views":v,"game":game,"avatar":av,"title":t})).into_response()
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
                "message": n.pointer("/message/fragments/0/text").cloned().unwrap_or(json!("")),
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
                out.push(json!({"id":id,"token":em.get("token").cloned().unwrap_or(json!("")),"url":format!("/api/proxy?url={}", STANDARD.encode(format!("https://static-cdn.jtvnw.net/emoticons/v2/{id}/static/dark/2.0")))}));
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

async fn stream_proxy(
    State(state): State<Arc<AppState>>,
    Path(username): Path<String>,
    Query(query): Query<QualityQuery>,
) -> impl IntoResponse {
    let quality = query.quality.unwrap_or_else(|| "720".to_string());
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
    let url = format!(
        "https://usher.ttvnw.net/api/channel/hls/{}.m3u8?player_type=pulsar&player_backend=mediaplayer&playlist_include_framerate=true&allow_source=true&transcode_mode=cbr_v1&cdm=wv&player_version=1.22.0&token={}&sig={}",
        username.to_lowercase(),
        urlencoding::encode(token),
        sig
    );
    let list_text = match fetch_raw_text(&state, &url, true).await {
        Ok(t) => t,
        Err(r) => return r,
    };
    let selected = select_playlist(&list_text, &quality).unwrap_or_default();
    if selected.is_empty() {
        return (
            [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")],
            list_text,
        )
            .into_response();
    }
    fetch_text(&state, &selected, true).await
}

async fn vod_proxy(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<QualityQuery>,
) -> impl IntoResponse {
    let quality = query.quality.unwrap_or_else(|| "720".to_string());
    let token = gql(&state, json!({"query":"query PlaybackAccessToken_Template($login: String!, $isLive: Boolean!, $vodID: ID!, $isVod: Boolean!, $playerType: String!) { videoPlaybackAccessToken(id: $vodID, params: {platform: \"web\", playerBackend: \"mediaplayer\", playerType: $playerType}) @include(if: $isVod) { value signature } }","variables":{"isLive":false,"login":"","isVod":true,"vodID":id,"playerType":"site"}}), false).await;
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
    if sig.is_empty() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":true,"data":null})),
        )
            .into_response();
    }

    let p: u32 = rand::thread_rng().gen_range(1..=99999);
    let playlist_url = format!("https://usher.ttvnw.net/vod/{id}.m3u8?acmb=e30%3D&allow_source=true&p={p}&cdm=wv&transcode_mode=cbr_v1&supported_codecs=avc1&player_version=1.19.0&player_base=mediaplayer&reassignments_supported=true&playlist_include_framerate=true&player_backend=mediaplayer&token={}&sig={}", urlencoding::encode(val), sig);
    let list_text = match fetch_raw_text(&state, &playlist_url, false).await {
        Ok(t) => t,
        Err(r) => return r,
    };
    let selected = select_playlist(&list_text, &quality).unwrap_or_default();
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
    let base = selected.split("index-dvr.m3u8").next().unwrap_or("");
    let body = manifest
        .lines()
        .map(|line| {
            if line.starts_with('#') || line.trim().is_empty() {
                line.to_string()
            } else {
                format!(
                    "/api/proxy?url={}",
                    STANDARD.encode(format!("{base}{line}"))
                )
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    (
        [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")],
        body,
    )
        .into_response()
}

fn select_playlist(manifest: &str, quality: &str) -> Option<String> {
    let lines: Vec<&str> = manifest.lines().collect();
    if quality.eq_ignore_ascii_case("audio") {
        for i in 0..lines.len().saturating_sub(1) {
            let line = lines[i].to_ascii_lowercase();
            if line.contains("audio_only") || line.contains("audio-only") {
                let next = lines[i + 1].trim();
                if next.starts_with("http") {
                    return Some(next.to_string());
                }
            }
        }
    }

    let quality = quality.parse::<u32>().unwrap_or(720);
    for i in 0..lines.len().saturating_sub(1) {
        if lines[i].contains("RESOLUTION=") && lines[i].contains(&format!("x{quality}")) {
            let next = lines[i + 1].trim();
            if next.starts_with("http") {
                return Some(next.to_string());
            }
        }
    }
    lines
        .iter()
        .find(|l| l.starts_with("http"))
        .map(|x| x.to_string())
}

async fn fetch_text(state: &AppState, url: &str, mobile: bool) -> Response {
    match fetch_raw_text(state, url, mobile).await {
        Ok(text) => (
            [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")],
            text,
        )
            .into_response(),
        Err(r) => r,
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
    let bytes = resp.bytes().await.unwrap_or_default();

    let mut out_headers = HeaderMap::new();
    out_headers.insert(header::CONTENT_TYPE, ct);
    if let Some(ccv) = cc {
        out_headers.insert(header::CACHE_CONTROL, ccv);
    }
    (status, out_headers, bytes).into_response()
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

    format!("<!doctype html><html><head><meta charset=\"UTF-8\" /><title>Twinr - Clip {title}</title>{video_tags}<link rel=\"stylesheet\" href=\"/styles.min.css\"><link rel=\"stylesheet\" href=\"/poppins.css\"></head><body><div class=\"container\"><video controls src=\"{src}\"></video><span id=\"date\"></span><h3>{title}</h3><div>{game}</div><div><span>By {author}</span> <span>{views} views</span></div><div><a href=\"/{username}?home=true\"><img class=\"w-8 rounded-full\" src=\"/api/urlproxy?url={avatar}\" /></a><a href=\"/{username}?home=true\">{username}</a></div></div><script>const date=Date.parse('{date}')-Date.now(),sec=Math.abs(Math.floor(date/1000)),min=Math.abs(Math.floor(sec/60)),hours=Math.abs(Math.floor(min/60)),days=Math.abs(Math.floor(hours/24));document.getElementById('date').innerText=`${{days}} days, ${{hours%24}} hours, ${{min%60}} minutes, and ${{sec%60}} seconds ago`;</script><footer><p>Twinr Version {} - <a href=\"https://codeberg.org/CloudyyUw/twinr\">Source</a></p></footer></body></html>", state.version)
}
