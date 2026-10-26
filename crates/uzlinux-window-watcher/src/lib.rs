pub mod watcher;

pub use watcher::{WindowWatcher, ActiveAppEvent};

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_window_watcher_init() {
        let watcher = WindowWatcher::new(Duration::from_millis(500));
        let _rx = watcher.start();
    }
}
