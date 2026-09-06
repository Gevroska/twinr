use crate::{cache::MetadataCache, config::Config, errors::AppError, security::SafeResolver};
use reqwest::Client;
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;
pub struct AppState {
    pub client: Client,
    pub client_id: String,
    pub user_agent: String,
    pub base_url: Option<String>,
    pub version: String,
    pub opus_audio_bitrates: Vec<u32>,
    pub cache: MetadataCache,
    pub gql_slots: Arc<Semaphore>,
    pub ffmpeg_slots: Arc<Semaphore>,
}
impl AppState {
    pub fn new(config: Config) -> Result<Arc<Self>, AppError> {
        let client = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .dns_resolver(Arc::new(SafeResolver))
            .connect_timeout(config.connect_timeout)
            .read_timeout(config.read_timeout)
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(16)
            .build()
            .map_err(|_| AppError::InvalidInput)?;
        Ok(Arc::new(Self {
            client,
            client_id: config.client_id,
            user_agent: config.user_agent,
            base_url: config.base_url,
            version: env!("CARGO_PKG_VERSION").into(),
            opus_audio_bitrates: config.opus_audio_bitrates,
            cache: MetadataCache::new(config.cache_entries),
            gql_slots: Arc::new(Semaphore::new(config.gql_concurrency)),
            ffmpeg_slots: Arc::new(Semaphore::new(config.max_ffmpeg)),
        }))
    }
}
