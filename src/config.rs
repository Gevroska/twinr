use crate::errors::AppError;
use std::{collections::BTreeSet, time::Duration};
#[derive(Clone)]
pub struct Config {
    pub client_id: String,
    pub user_agent: String,
    pub base_url: Option<String>,
    pub opus_audio_bitrates: Vec<u32>,
    pub max_ffmpeg: usize,
    pub cache_entries: usize,
    pub gql_concurrency: usize,
    pub connect_timeout: Duration,
    pub read_timeout: Duration,
}
pub fn bounded(raw: Option<&str>, default: usize, max: usize) -> Result<usize, AppError> {
    match raw {
        None => Ok(default),
        Some(s) => s
            .parse::<usize>()
            .ok()
            .filter(|v| *v > 0 && *v <= max)
            .ok_or(AppError::InvalidInput),
    }
}
pub fn opus_bitrates(raw: Option<&str>) -> Result<Vec<u32>, AppError> {
    let Some(raw) = raw else {
        return Ok(vec![]);
    };
    if raw.trim().is_empty() || raw.trim().eq_ignore_ascii_case("no") {
        return Ok(vec![]);
    }
    raw.split(',')
        .map(|s| {
            s.trim()
                .parse::<u32>()
                .ok()
                .filter(|v| (6..=256).contains(v))
                .ok_or(AppError::InvalidInput)
        })
        .collect::<Result<BTreeSet<_>, _>>()
        .map(|s| s.into_iter().collect())
}
impl Config {
    pub fn from_env() -> Result<Self, AppError> {
        let get = |name| std::env::var(name).ok();
        let config = Self {
            client_id: get("CLIENTID").unwrap_or_else(|| "kimne78kx3ncx6brgo4mv6wki5h1ko".into()),
            user_agent: get("USERAGENT").unwrap_or_else(|| "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36".into()),
            base_url: get("INSTANCE_URL"),
            opus_audio_bitrates: opus_bitrates(get("OPUS_AUDIO_BITRATES").as_deref())?,
            max_ffmpeg: bounded(get("MAX_FFMPEG_PROCESSES").as_deref(), 4, 64)?,
            cache_entries: bounded(get("METADATA_CACHE_ENTRIES").as_deref(), 2048, 100000)?,
            gql_concurrency: bounded(get("TWITCH_REQUEST_CONCURRENCY").as_deref(), 16, 128)?,
            connect_timeout: Duration::from_secs(bounded(
                get("UPSTREAM_CONNECT_TIMEOUT_SECONDS").as_deref(),
                10,
                120,
            )? as u64),
            read_timeout: Duration::from_secs(bounded(
                get("UPSTREAM_READ_TIMEOUT_SECONDS").as_deref(),
                30,
                300,
            )? as u64),
        };
        for value in [&config.client_id, &config.user_agent] {
            axum::http::HeaderValue::from_str(value).map_err(|_| AppError::InvalidInput)?;
        }
        if let Some(url) = &config.base_url {
            let parsed = reqwest::Url::parse(url).map_err(|_| AppError::InvalidInput)?;
            if !matches!(parsed.scheme(), "https" | "http")
                || !parsed.username().is_empty()
                || parsed.password().is_some()
            {
                return Err(AppError::InvalidInput);
            }
        }
        Ok(config)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn limits_are_validated() {
        assert_eq!(bounded(None, 4, 64), Ok(4));
        for v in ["0", "-1", "65", "garbage"] {
            assert!(bounded(Some(v), 4, 64).is_err());
        }
    }
    #[test]
    fn opus_config_is_bounded_and_deduplicated() {
        assert_eq!(opus_bitrates(Some("64,32,64")).unwrap(), vec![32, 64]);
        assert!(opus_bitrates(Some("9999")).is_err());
        assert!(opus_bitrates(Some("no")).unwrap().is_empty());
    }
}
