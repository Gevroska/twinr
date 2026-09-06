use crate::{errors::AppError, state::AppState};
use axum::http::{header, HeaderMap};
use reqwest::{
    dns::{Addrs, Name, Resolve, Resolving},
    Url,
};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

pub fn username(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 25
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_')
}
pub fn vod_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 20 && value.bytes().all(|c| c.is_ascii_digit())
}
pub fn clip_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 200
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
pub fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => public_v4(ip),
        IpAddr::V6(ip) => public_v6(ip),
    }
}
fn public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(a == 0
        || a == 10
        || a == 127
        || a >= 224
        || (a == 100 && (64..=127).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && (b == 168 || b == 0 || (b == 88 && c == 99)))
        || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
        || (a == 203 && b == 0 && c == 113))
}
fn public_v6(ip: Ipv6Addr) -> bool {
    let s = ip.segments();
    s[0] & 0xe000 == 0x2000
        && !(s[0] == 0x2001 && (s[1] < 0x0200 || s[1] == 0x0db8))
        && s[0] != 0x2002
        && !(s[0] == 0x3fff && s[1] < 0x1000)
}
pub fn allowed_host(host: &str) -> bool {
    ["twitch.tv", "ttvnw.net", "jtvnw.net", "twitchcdn.net"]
        .iter()
        .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
        || [
            "d3stzm2eumvgb4.cloudfront.net",
            "d2nvs31859zcd8.cloudfront.net",
            "d2e2de1etea730.cloudfront.net",
            "d1m7jfoe9zdc1j.cloudfront.net",
            "d2vjef5jvl6bfs.cloudfront.net",
            "d1ymi26ma8va5x.cloudfront.net",
        ]
        .contains(&host)
}
pub fn validate_url(value: &str) -> Result<Url, AppError> {
    if value.len() > 16384 {
        return Err(AppError::ForbiddenUrl);
    }
    let url = Url::parse(value).map_err(|_| AppError::ForbiddenUrl)?;
    let host = url.host_str().ok_or(AppError::ForbiddenUrl)?;
    if url.scheme() != "https"
        || url.port().is_some_and(|p| p != 443)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || !allowed_host(host)
        || host.parse::<IpAddr>().is_ok()
    {
        return Err(AppError::ForbiddenUrl);
    }
    Ok(url)
}
fn public_addresses(addresses: &[std::net::SocketAddr]) -> bool {
    !addresses.is_empty() && addresses.iter().all(|a| public_ip(a.ip()))
}
fn redirect_url(base: &Url, location: &str) -> Result<Url, AppError> {
    let next = base.join(location).map_err(|_| AppError::ForbiddenUrl)?;
    validate_url(next.as_str())
}
#[derive(Debug)]
pub struct SafeResolver;
impl Resolve for SafeResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_string();
        Box::pin(async move {
            let addresses: Vec<_> = tokio::net::lookup_host((host.as_str(), 0)).await?.collect();
            if !public_addresses(&addresses) {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "Non-public DNS answer",
                ))
                    as Box<dyn std::error::Error + Send + Sync>);
            }
            Ok(Box::new(addresses.into_iter()) as Addrs)
        })
    }
}
pub async fn get(
    state: &AppState,
    value: &str,
    headers: HeaderMap,
) -> Result<reqwest::Response, AppError> {
    let mut url = validate_url(value)?;
    for _ in 0..5 {
        let response = state
            .client
            .get(url.clone())
            .headers(headers.clone())
            .send()
            .await
            .map_err(|_| AppError::Upstream)?;
        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or(AppError::Upstream)?;
            url = redirect_url(&url, location)?;
        } else {
            return Ok(response);
        }
    }
    Err(AppError::Upstream)
}
pub async fn limited_text(mut response: reqwest::Response, max: usize) -> Result<String, AppError> {
    if !response.status().is_success() {
        return Err(AppError::Upstream);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| AppError::Upstream)? {
        if bytes.len().saturating_add(chunk.len()) > max {
            return Err(AppError::Upstream);
        }
        bytes.extend_from_slice(&chunk);
    }
    String::from_utf8(bytes).map_err(|_| AppError::Upstream)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_mixed_dns_answers_and_redirect_bypasses() {
        assert!(!public_addresses(&[]));
        assert!(!public_addresses(&[
            "8.8.8.8:443".parse().unwrap(),
            "127.0.0.1:443".parse().unwrap()
        ]));
        assert!(public_addresses(&["8.8.8.8:443".parse().unwrap()]));
        let base = validate_url("https://cdn.ttvnw.net/path/list.m3u8").unwrap();
        for location in [
            "//127.0.0.1/admin",
            "https://evil.com/",
            "http://cdn.ttvnw.net/",
            "https://[::ffff:127.0.0.1]/",
        ] {
            assert!(redirect_url(&base, location).is_err());
        }
        assert_eq!(
            redirect_url(&base, "../next.m3u8").unwrap().as_str(),
            "https://cdn.ttvnw.net/next.m3u8"
        );
    }
    #[test]
    fn blocks_unsafe_urls() {
        for url in [
            "http://static-cdn.jtvnw.net/a",
            "https://127.0.0.1/",
            "https://[::1]/",
            "https://2130706433/",
            "https://twitch.tv.evil.com/",
            "https://evil.com/?twitch.tv",
            "https://twitch.tv@evil.com/",
            "https://x@twitch.tv/",
            "https://twitch.tv:444/a",
            "file:///etc/passwd",
            "https://twitch.tv./",
            "https://arbitrary.cloudfront.net/x",
        ] {
            assert!(validate_url(url).is_err(), "{url}");
        }
        assert!(validate_url("https://static-cdn.jtvnw.net/a?x=1").is_ok());
        assert!(validate_url("https://video-edge-123.ttvnw.net/a").is_ok());
    }
    #[test]
    fn blocks_private_reserved_and_ipv6_bypasses() {
        for ip in [
            "0.0.0.0",
            "10.2.3.4",
            "127.0.0.1",
            "100.64.1.1",
            "169.254.169.254",
            "172.16.1.1",
            "192.168.1.1",
            "192.0.2.1",
            "198.19.1.1",
            "198.51.100.2",
            "203.0.113.2",
            "224.0.0.1",
            "255.255.255.255",
            "::",
            "::1",
            "::ffff:8.8.8.8",
            "fc00::1",
            "fe80::1",
            "ff00::1",
            "64:ff9b::a00:1",
            "2001:db8::1",
            "2002:a00:1::",
            "3fff::1",
        ] {
            assert!(!public_ip(ip.parse().unwrap()), "{ip}");
        }
        for ip in [
            "8.8.8.8",
            "1.1.1.1",
            "2606:4700:4700::1111",
            "2001:4860:4860::8888",
        ] {
            assert!(public_ip(ip.parse().unwrap()), "{ip}");
        }
    }
    #[test]
    fn validates_route_identifiers() {
        assert!(username("raz404"));
        assert!(!username("a\r\nJOIN #evil"));
        assert!(!vod_id("../1"));
        assert!(vod_id("2864317262"));
        assert!(!clip_id("a/b"));
    }
}
