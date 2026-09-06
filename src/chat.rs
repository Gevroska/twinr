use crate::state::AppState;
use axum::{
    extract::{State, WebSocketUpgrade},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::sync::Arc;
pub async fn root_or_ws(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    ws: Option<WebSocketUpgrade>,
) -> Response {
    if let Some(upgrade) = ws {
        if let Some(origin) = headers.get("origin").and_then(|h| h.to_str().ok()) {
            let permitted = state
                .base_url
                .as_deref()
                .and_then(|v| reqwest::Url::parse(v).ok())
                .map(|v| v.origin().ascii_serialization() == origin)
                .unwrap_or_else(|| {
                    reqwest::Url::parse(origin)
                        .ok()
                        .and_then(|v| v.host_str().map(|h| h.to_string()))
                        == headers
                            .get("host")
                            .and_then(|h| h.to_str().ok())
                            .map(|h| h.split(':').next().unwrap_or("").to_string())
                });
            if !permitted {
                return StatusCode::FORBIDDEN.into_response();
            }
        }
        return upgrade
            .max_message_size(4096)
            .max_frame_size(4096)
            .on_upgrade(chat_socket)
            .into_response();
    }
    crate::routes::index_file().await.into_response()
}
fn unescape(value: &str) -> String {
    let mut out = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(match next {
                    's' => ' ',
                    ':' => ';',
                    'n' => '\n',
                    'r' => '\r',
                    x => x,
                });
            }
        } else {
            out.push(c);
        }
    }
    out
}
pub(crate) fn parse_message(line: &str) -> Option<Value> {
    let (tags, rest) = line.strip_prefix('@')?.split_once(' ')?;
    let (prefix, rest) = rest.strip_prefix(':')?.split_once(' ')?;
    let rest = rest.strip_prefix("PRIVMSG #")?;
    let (_, raw) = rest.split_once(" :")?;
    let message = raw
        .strip_prefix("\x01ACTION ")
        .and_then(|s| s.strip_suffix('\x01'))
        .unwrap_or(raw);
    let tags: std::collections::HashMap<_, _> = tags
        .split(';')
        .filter_map(|s| s.split_once('='))
        .map(|(k, v)| (k, unescape(v)))
        .collect();
    let get = |key| tags.get(key).map(String::as_str).unwrap_or("");
    let chars: Vec<_> = message.chars().collect();
    let mut ranges = Vec::new();
    for group in get("emotes").split('/') {
        if let Some((id, positions)) = group.split_once(':') {
            if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
                continue;
            }
            for pos in positions.split(',') {
                if let Some((start, end)) = pos.split_once('-') {
                    if let (Ok(s), Ok(e)) = (start.parse::<usize>(), end.parse::<usize>()) {
                        if s <= e && e < chars.len() {
                            ranges.push((s, e, id));
                        }
                    }
                }
            }
        }
    }
    ranges.sort_by_key(|r| r.0);
    let mut fragments = Vec::new();
    let mut cursor = 0;
    for (start, end, id) in ranges {
        if start < cursor {
            continue;
        }
        if start > cursor {
            fragments.push(json!({"text":chars[cursor..start].iter().collect::<String>()}));
        }
        fragments.push(json!({"text":chars[start..=end].iter().collect::<String>(),"emoteId":id}));
        cursor = end + 1;
    }
    if cursor < chars.len() {
        fragments.push(json!({"text":chars[cursor..].iter().collect::<String>()}));
    }
    Some(
        json!({"username":prefix.split('!').next().unwrap_or(""),"display-name":get("display-name"),"color":get("color"),"mod":get("mod"),"subscriber":get("subscriber"),"emotes":get("emotes"),"message":message,"fragments":fragments}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_tags_unicode_emotes_and_messages() {
        let value=parse_message("@display-name=Some\\sName;color=#123456;mod=1;subscriber=1;emotes=25:2-6 :login!login@host PRIVMSG #room :😀 Kappa!").unwrap();
        assert_eq!(value["username"], "login");
        assert_eq!(value["display-name"], "Some Name");
        assert_eq!(value["mod"], "1");
        assert_eq!(value["fragments"][1]["text"], "Kappa");
        assert_eq!(value["fragments"][1]["emoteId"], "25");
    }
    #[test]
    fn ignores_protocol_and_invalid_emotes() {
        assert!(parse_message("PING :tmi.twitch.tv").is_none());
        let v = parse_message("@emotes=25:99-100 :a!b PRIVMSG #room :hello").unwrap();
        assert_eq!(v["fragments"], json!([{"text":"hello"}]));
        assert_eq!(unescape("a\\:b\\\\c"), "a;b\\c");
    }
    #[test]
    fn actions_are_plain_messages() {
        assert_eq!(
            parse_message("@emotes= :a!b PRIVMSG #room :\x01ACTION waves\x01").unwrap()["message"],
            "waves"
        );
    }
}
async fn chat_socket(stream: axum::extract::ws::WebSocket) {
    use axum::extract::ws::Message;
    let (mut sender, mut receiver) = stream.split();
    let first = tokio::time::timeout(std::time::Duration::from_secs(10), receiver.next())
        .await
        .ok()
        .flatten();
    let Some(Ok(Message::Text(cmd))) = first else {
        return;
    };
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.len() != 2 || parts[0] != "JOIN" || parts[1].contains(',') {
        let _ = sender.close().await;
        return;
    }
    let channel = parts[1].trim_start_matches('#').to_lowercase();
    if !crate::security::username(&channel) {
        return;
    }
    let tws = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        tokio_tungstenite::connect_async("wss://irc-ws.chat.twitch.tv:443"),
    )
    .await;
    let Ok(Ok((twitch_ws, _))) = tws else {
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
                    let messages:Vec<_>=text.lines().filter_map(parse_message).collect();
                    if !messages.is_empty() && sender.send(Message::Text(serde_json::to_string(&messages).unwrap())).await.is_err() { break; }
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
