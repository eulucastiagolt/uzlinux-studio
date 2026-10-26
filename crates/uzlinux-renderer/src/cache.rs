use std::sync::Arc;
use dashmap::DashMap;

#[derive(Clone, Default)]
pub struct ImageCache {
    cache: Arc<DashMap<String, Vec<u8>>>,
}

impl ImageCache {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(DashMap::new()),
        }
    }

    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        self.cache.get(key).map(|v| v.value().clone())
    }

    pub fn set(&self, key: String, data: Vec<u8>) {
        self.cache.insert(key, data);
    }

    pub fn clear(&self) {
        self.cache.clear();
    }
}
