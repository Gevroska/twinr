use crate::{errors::AppError, security, state::AppState};
use axum::http::{header, HeaderMap, HeaderValue};
use serde_json::Value;
use std::time::Duration;

fn ttl(body: &Value) -> Option<Duration> {
    let operation = body
        .get("operationName")
        .and_then(Value::as_str)
        .unwrap_or("");
    let query = body.get("query").and_then(Value::as_str).unwrap_or("");
    if query.contains("playbackAccessToken") || query.contains("videoPlaybackAccessToken") {
        return None;
    }
    Some(Duration::from_secs(match operation {
        "StreamMetadata" | "SignupPromptCategory" => 5,
        "VideoAccessToken_Clip" => 30,
        "ClipsCards__User" | "FilterableVideoTower_Videos" => 60,
        "VideoCommentsByOffsetOrCursor" => 300,
        "ComscoreStreamingQuery"
            if body.pointer("/variables/isLive").and_then(Value::as_bool) == Some(true) =>
        {
            5
        }
        _ if query.contains("TwinrStreamInfo") => 5,
        _ => 300,
    }))
}
pub(crate) async fn gql(state: &AppState, body: Value, mobile: bool) -> Result<Value, AppError> {
    let key = format!("{mobile}:{}", body);
    if key.len() > 16384 {
        return Err(AppError::InvalidInput);
    }
    match ttl(&body) {
        Some(ttl) => {
            state
                .cache
                .get(key, ttl, request(state, body, mobile))
                .await
        }
        None => request(state, body, mobile).await,
    }
}
async fn request(state: &AppState, body: Value, mobile: bool) -> Result<Value, AppError> {
    let _permit = tokio::time::timeout(Duration::from_secs(2), state.gql_slots.acquire())
        .await
        .map_err(|_| AppError::Busy)?
        .map_err(|_| AppError::Busy)?;
    tokio::time::timeout(Duration::from_secs(15), async {
        let mut headers = HeaderMap::new();
        headers.insert(
            "Client-ID",
            HeaderValue::from_str(&state.client_id).map_err(|_| AppError::InvalidInput)?,
        );
        headers.insert(
            "Device-Id",
            HeaderValue::from_str(&uuid::Uuid::new_v4().to_string()).unwrap(),
        );
        headers.insert(
            header::USER_AGENT,
            HeaderValue::from_str(&state.user_agent).map_err(|_| AppError::InvalidInput)?,
        );
        let origin = if mobile {
            "https://m.twitch.tv/"
        } else {
            "https://www.twitch.tv/"
        };
        headers.insert(header::REFERER, HeaderValue::from_static(origin));
        headers.insert(header::ORIGIN, HeaderValue::from_static(origin));
        let response = state
            .client
            .post("https://gql.twitch.tv/gql")
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|_| AppError::Upstream)?;
        let text = security::limited_text(response, 4 * 1024 * 1024).await?;
        let result: Value = serde_json::from_str(&text).map_err(|_| AppError::Upstream)?;
        if result
            .get("errors")
            .and_then(Value::as_array)
            .is_some_and(|errors| !errors.is_empty())
        {
            return Err(AppError::Upstream);
        }
        Ok(result)
    })
    .await
    .map_err(|_| AppError::Timeout)?
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn short_live_ttl_and_uncached_tokens() {
        assert_eq!(
            ttl(&json!({"query":"query TwinrStreamInfo {}"})),
            Some(Duration::from_secs(5))
        );
        assert_eq!(
            ttl(&json!({"query":"query { videoPlaybackAccessToken }"})),
            None
        );
        assert_eq!(
            ttl(&json!({"operationName":"ChannelShell"})),
            Some(Duration::from_secs(300))
        );
    }
}
