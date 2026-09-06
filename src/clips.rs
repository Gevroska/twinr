use crate::{metadata::to_clip_media, routes::index_file};
use crate::{state::AppState, twitch::gql};
use axum::{
    extract::{Path, Query, State},
    response::{Html, IntoResponse},
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
#[derive(Deserialize)]
pub(crate) struct ClipPageQuery {
    quality: Option<String>,
    embed: Option<String>,
}

pub(crate) async fn clip_page_or_index(
    State(state): State<Arc<AppState>>,
    Path((username, id)): Path<(String, String)>,
    Query(query): Query<ClipPageQuery>,
) -> impl IntoResponse {
    if !crate::security::username(&username) || !crate::security::clip_id(&id) {
        return crate::errors::AppError::InvalidInput.into_response();
    }

    if query.embed.is_none() {
        return index_file().await.into_response();
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
    let title = escape(title);
    let game = escape(game);
    let author = escape(author);
    let avatar = escape(&urlencoding::encode(avatar));
    let src = escape(src);
    let username = escape(username);
    let date = serde_json::to_string(date)
        .unwrap_or_else(|_| "null".into())
        .replace('<', "\\u003c");
    let video_tags = state.base_url.as_ref().map(|base| {let base=escape(base); format!("<meta name=\"twitter:card\" content=\"player\" /><meta property=\"og:video\" content=\"{base}{src}\" />")}).unwrap_or_default();

    format!("<!doctype html><html><head><meta charset=\"UTF-8\" /><title>Twinr - Clip {title}</title>{video_tags}<link rel=\"stylesheet\" href=\"/styles.min.css\"><link rel=\"stylesheet\" href=\"/poppins.css\"></head><body><div class=\"container\"><video controls src=\"{src}\"></video><span id=\"date\"></span><h3>{title}</h3><div>{game}</div><div><span>By {author}</span> <span>{views} views</span></div><div><a href=\"/{username}?home=true\"><img class=\"w-8 rounded-full\" src=\"/api/urlproxy?url={avatar}\" /></a><a href=\"/{username}?home=true\">{username}</a></div></div><script>const date=Date.parse({date})-Date.now(),sec=Math.abs(Math.floor(date/1000)),min=Math.abs(Math.floor(sec/60)),hours=Math.abs(Math.floor(min/60)),days=Math.abs(Math.floor(hours/24));document.getElementById('date').innerText=`${{days}} days, ${{hours%24}} hours, ${{min%60}} minutes, and ${{sec%60}} seconds ago`;</script><footer><p>Twinr Version {} - <a href=\"https://github.com/Gevroska/twinr\">Source</a></p></footer></body></html>", state.version)
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn escapes_untrusted_embed_fields() {
        assert_eq!(escape("<script>\"'&"), "&lt;script&gt;&quot;&#39;&amp;");
    }
}
