use crate::errors::AppError;
use serde_json::Value;
use std::{
    collections::HashMap,
    future::Future,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, OnceCell};

struct Entry {
    created: Instant,
    value: OnceCell<Result<Arc<Value>, AppError>>,
    weight: AtomicUsize,
}
pub struct MetadataCache {
    entries: Mutex<HashMap<String, Arc<Entry>>>,
    capacity: usize,
}
impl MetadataCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            capacity,
        }
    }
    pub async fn get<F: Future<Output = Result<Value, AppError>>>(
        &self,
        key: String,
        ttl: Duration,
        fetch: F,
    ) -> Result<Value, AppError> {
        let entry = {
            let mut entries = self.entries.lock().await;
            if let Some(entry) = entries.get(&key) {
                if entry.created.elapsed() >= ttl && entry.value.initialized() {
                    entries.remove(&key);
                }
            }
            if let Some(entry) = entries.get(&key) {
                if let Some(Ok(value)) = entry.value.get() {
                    return Ok((**value).clone());
                }
                entry.clone()
            } else {
                if entries.len() >= self.capacity {
                    let oldest = entries
                        .iter()
                        .filter(|(_, v)| {
                            v.value.initialized() || v.created.elapsed() > Duration::from_secs(60)
                        })
                        .min_by_key(|(_, v)| v.created)
                        .map(|(k, _)| k.clone());
                    if let Some(key) = oldest {
                        entries.remove(&key);
                    } else {
                        return Err(AppError::Busy);
                    }
                }
                let entry = Arc::new(Entry {
                    created: Instant::now(),
                    value: OnceCell::new(),
                    weight: AtomicUsize::new(0),
                });
                entries.insert(key.clone(), entry.clone());
                entry
            }
        };
        let result = entry
            .value
            .get_or_init(|| async {
                fetch.await.map(|v| {
                    entry.weight.store(
                        serde_json::to_vec(&v).map(|s| s.len()).unwrap_or(0),
                        Ordering::Relaxed,
                    );
                    Arc::new(v)
                })
            })
            .await
            .clone();
        if result.is_err() || entry.weight.load(Ordering::Relaxed) > 256 * 1024 {
            let mut entries = self.entries.lock().await;
            if entries.get(&key).is_some_and(|v| Arc::ptr_eq(v, &entry)) {
                entries.remove(&key);
            }
        }
        // Cap serialized payload bytes as well as entry count. In-flight responses
        // are separately bounded by the upstream semaphore and body limit.
        let mut entries = self.entries.lock().await;
        let mut total: usize = entries
            .values()
            .map(|e| e.weight.load(Ordering::Relaxed))
            .sum();
        while total > 32 * 1024 * 1024 {
            let oldest = entries
                .iter()
                .filter(|(_, e)| e.value.initialized())
                .min_by_key(|(_, e)| e.created)
                .map(|(k, _)| k.clone());
            if let Some(key) = oldest {
                if let Some(e) = entries.remove(&key) {
                    total = total.saturating_sub(e.weight.load(Ordering::Relaxed));
                }
            } else {
                break;
            }
        }
        drop(entries);
        result.map(|value| (*value).clone())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[tokio::test]
    async fn cancelled_loader_can_be_retried_and_large_responses_are_not_retained() {
        let cache = MetadataCache::new(2);
        let cancelled = tokio::time::timeout(
            Duration::from_millis(5),
            cache.get("a".into(), Duration::from_secs(10), std::future::pending()),
        )
        .await;
        assert!(cancelled.is_err());
        assert_eq!(
            cache
                .get("a".into(), Duration::from_secs(10), async {
                    Ok(Value::Bool(true))
                })
                .await
                .unwrap(),
            Value::Bool(true)
        );
        cache
            .get("large".into(), Duration::from_secs(10), async {
                Ok(Value::String("x".repeat(300 * 1024)))
            })
            .await
            .unwrap();
        assert!(!cache.entries.lock().await.contains_key("large"));
    }
    #[tokio::test]
    async fn coalesces_and_expires() {
        let cache = MetadataCache::new(2);
        let calls = AtomicUsize::new(0);
        let fetch = || async {
            calls.fetch_add(1, Ordering::SeqCst);
            tokio::task::yield_now().await;
            Ok(serde_json::json!({"ok":true}))
        };
        let (a, b) = tokio::join!(
            cache.get("a".into(), Duration::from_secs(1), fetch()),
            cache.get("a".into(), Duration::from_secs(1), fetch())
        );
        assert_eq!(a, b);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        cache
            .get("a".into(), Duration::ZERO, fetch())
            .await
            .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
    #[tokio::test]
    async fn cache_growth_is_bounded_and_errors_are_not_cached() {
        let cache = MetadataCache::new(2);
        for i in 0..10 {
            cache
                .get(i.to_string(), Duration::from_secs(10), async {
                    Ok(Value::Null)
                })
                .await
                .unwrap();
        }
        assert_eq!(cache.entries.lock().await.len(), 2);
        assert!(cache
            .get("error".into(), Duration::from_secs(10), async {
                Err(AppError::Upstream)
            })
            .await
            .is_err());
        assert!(cache
            .get("error".into(), Duration::from_secs(10), async {
                Ok(Value::Null)
            })
            .await
            .is_ok());
    }
}
