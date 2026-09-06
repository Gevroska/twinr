use crate::{errors::invalid, state::AppState, twitch::gql};
use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
pub(crate) async fn stream_info(
    State(state): State<Arc<AppState>>,
    Path(username): Path<String>,
) -> impl IntoResponse {
    if !crate::security::username(&username) {
        return crate::errors::AppError::InvalidInput.into_response();
    }

    let result = gql(&state, json!({
        "query": "query TwinrStreamInfo($login: String!) { user(login: $login) { profileImageURL(width: 70) broadcastSettings { title } stream { viewersCount game { name } } } }",
        "variables": {"login": username.to_lowercase()}
    }), false).await;
    let Ok(result) = result else {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": "Unable to load channel metadata"})),
        )
            .into_response();
    };
    if result
        .get("errors")
        .and_then(Value::as_array)
        .is_some_and(|errors| !errors.is_empty())
    {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": "Unable to load channel metadata"})),
        )
            .into_response();
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
    }))
    .into_response()
}

pub(crate) async fn streamer_info(
    State(state): State<Arc<AppState>>,
    Path(username): Path<String>,
) -> impl IntoResponse {
    if !crate::security::username(&username) {
        return crate::errors::AppError::InvalidInput.into_response();
    }

    let u = username.to_lowercase();
    let shell = gql(
        &state,
        json!({"operationName":"ChannelShell","variables":{"login":u},"extensions":{"persistedQuery":{"version":1,"sha256Hash":"580ab410bcd0c1ad194224957ae2241e5d252b2c5173d8e0cce9d32d5bb14efe"}}}),
        false,
    );
    let home = gql(
        &state,
        json!({"operationName":"HomeOfflineCarousel","variables":{"channelLogin":u,"includeTrailerUpsell":false,"trailerUpsellVideoID":""},"extensions":{"persistedQuery":{"version":1,"sha256Hash":"84e25789b04ac4dcaefd673cfb4259d39d03c6422838d09a4ed2aaf9b67054d8"}}}),
        false,
    );
    let (shell, home) = tokio::join!(shell, home);
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

pub(crate) async fn vod_info(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if !crate::security::vod_id(&id) {
        return crate::errors::AppError::InvalidInput.into_response();
    }
    let meta = gql(
        &state,
        json!({"operationName":"ComscoreStreamingQuery","variables":{"channel":"","clipSlug":"","isClip":false,"isLive":false,"isVodOrCollection":true,"vodID":id},"extensions":{"persistedQuery":{"version":1,"sha256Hash":"e1edae8122517d013405f237ffcc124515dc6ded82480a88daef69c83b53ac01"}}}),
        false,
    );
    let name = gql(
        &state,
        json!({"operationName":"VodChannelLoginQuery","variables":{"videoID":id},"extensions":{"persistedQuery":{"version":1,"sha256Hash":"0c5feea4dad2565508828f16e53fe62614edf015159df4b3bca33423496ce78e"}}}),
        false,
    );
    let (meta, name) = tokio::join!(meta, name);
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

pub(crate) async fn vod_comments(
    State(state): State<Arc<AppState>>,
    Path((id, offset)): Path<(String, String)>,
) -> impl IntoResponse {
    if !crate::security::vod_id(&id) || offset.parse::<u64>().is_err() {
        return crate::errors::AppError::InvalidInput.into_response();
    }

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

pub(crate) async fn vod_list(
    State(state): State<Arc<AppState>>,
    Path((username, filter, limit)): Path<(String, String, usize)>,
) -> impl IntoResponse {
    if !crate::security::username(&username)
        || limit == 0
        || limit > 100
        || !["ALL", "ARCHIVE", "UPLOAD", "HIGHLIGHT"].contains(&filter.as_str())
    {
        return crate::errors::AppError::InvalidInput.into_response();
    }

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

pub(crate) async fn clip_info(
    State(state): State<Arc<AppState>>,
    Path((username, id)): Path<(String, String)>,
) -> impl IntoResponse {
    if !crate::security::username(&username) || !crate::security::clip_id(&id) {
        return crate::errors::AppError::InvalidInput.into_response();
    }

    let metadata = gql(
        &state,
        json!({"operationName":"ClipMetadata","variables":{"channelLogin":username.to_lowercase(),"clipSlug":id},"extensions":{"persistedQuery":{"version":1,"sha256Hash":"ab70572e66f164789c87936a8291fd15e29adc2cea0114b02e60f17d60d6d154"}}}),
        false,
    );
    let media = gql(
        &state,
        json!({"operationName":"VideoAccessToken_Clip","variables":{"slug":id},"extensions":{"persistedQuery":{"version":1,"sha256Hash":"36b89d2507fce29e5ca551df756d27c1cfe079e2609642b4390aa4c35796eb11"}}}),
        false,
    );
    let (metadata, media) = tokio::join!(metadata, media);
    let (Ok(metadata), Ok(media)) = (metadata, media) else {
        return invalid().into_response();
    };
    let m = to_clip_media(&media);
    if m.is_empty() {
        return invalid().into_response();
    }
    ([(header::CACHE_CONTROL, "max-age=30")], Json(json!({
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

pub(crate) fn to_clip_media(media: &Value) -> Vec<Value> {
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

pub(crate) async fn clips_list(
    State(state): State<Arc<AppState>>,
    Path((username, filter, limit)): Path<(String, String, usize)>,
) -> impl IntoResponse {
    if !crate::security::username(&username)
        || limit == 0
        || limit > 100
        || !["LAST_DAY", "LAST_WEEK", "LAST_MONTH", "ALL_TIME"].contains(&filter.as_str())
    {
        return crate::errors::AppError::InvalidInput.into_response();
    }

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

pub(crate) async fn user_info(
    State(state): State<Arc<AppState>>,
    Path(username): Path<String>,
) -> impl IntoResponse {
    if !crate::security::username(&username) {
        return crate::errors::AppError::InvalidInput.into_response();
    }

    let u = username.to_lowercase();
    let about = gql(
        &state,
        json!({"operationName":"ChannelRoot_AboutPanel","variables":{"channelLogin":u,"skipSchedule":true},"extensions":{"persistedQuery":{"sha256Hash":"6089531acef6c09ece01b440c41978f4c8dc60cb4fa0124c9a9d3f896709b6c6","version":1}}}),
        false,
    );
    let stream_meta = gql(
        &state,
        json!({"operationName":"StreamMetadata","variables":{"channelLogin":u},"extensions":{"persistedQuery":{"sha256Hash":"a647c2a13599e5991e175155f798ca7f1ecddde73f7f341f39009c14dbf59962","version":1}}}),
        false,
    );
    let shell = gql(
        &state,
        json!({"operationName":"ChannelShell","variables":{"login":u},"extensions":{"persistedQuery":{"version":1,"sha256Hash":"580ab410bcd0c1ad194224957ae2241e5d252b2c5173d8e0cce9d32d5bb14efe"}}}),
        false,
    );
    let (about, stream_meta, shell) = tokio::join!(about, stream_meta, shell);
    let (Ok(about), Ok(stream_meta), Ok(shell)) = (about, stream_meta, shell) else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":{"status":500,"message":"fetch failed"},"data":null})),
        )
            .into_response();
    };
    if about
        .pointer("/data/user/id")
        .and_then(Value::as_str)
        .is_none()
    {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error":"Channel not found","data":null})),
        )
            .into_response();
    }
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

pub(crate) async fn emotes(
    State(state): State<Arc<AppState>>,
    Path(username): Path<String>,
) -> impl IntoResponse {
    if !crate::security::username(&username) {
        return crate::errors::AppError::InvalidInput.into_response();
    }

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
pub(crate) struct BulkQuery {
    pub usernames: Vec<String>,
}
pub(crate) async fn users_bulk(
    State(state): State<Arc<AppState>>,
    Json(query): Json<BulkQuery>,
) -> Response {
    use futures::{stream, StreamExt};
    if query.usernames.len() > 100 {
        return crate::errors::AppError::InvalidInput.into_response();
    }
    let names: std::collections::BTreeSet<_> = query
        .usernames
        .into_iter()
        .map(|s| s.to_lowercase())
        .collect();
    if names.iter().any(|s| !crate::security::username(s)) {
        return crate::errors::AppError::InvalidInput.into_response();
    }
    let results = stream::iter(names)
        .map(|name| {
            let state = state.clone();
            async move {
                let response = user_info(State(state), Path(name.clone()))
                    .await
                    .into_response();
                let ok = response.status().is_success();
                let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
                    .await
                    .ok();
                let value = bytes.and_then(|b| serde_json::from_slice::<Value>(&b).ok());
                (
                    name,
                    if ok {
                        value.and_then(|v| v.get("data").filter(|d| d.is_object()).cloned())
                    } else {
                        None
                    },
                )
            }
        })
        .buffer_unordered(8)
        .collect::<Vec<_>>()
        .await;
    let mut data = vec![];
    let mut failed = vec![];
    for (name, value) in results {
        if let Some(mut value) = value {
            value["login"] = json!(name);
            data.push(value);
        } else {
            failed.push(name);
        }
    }
    (
        [(header::CACHE_CONTROL, "no-store")],
        Json(json!({"data":data,"failed":failed})),
    )
        .into_response()
}
