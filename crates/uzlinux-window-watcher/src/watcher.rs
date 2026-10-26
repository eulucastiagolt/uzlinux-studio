use std::time::Duration;
use active_win_pos_rs::get_active_window;
use log::{debug, info};
use tokio::sync::mpsc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveAppEvent {
    pub app_name: String,
    pub title: String,
    pub process_id: u64,
}

pub struct WindowWatcher {
    poll_interval: Duration,
}

impl WindowWatcher {
    pub fn new(poll_interval: Duration) -> Self {
        Self { poll_interval }
    }

    /// Spawns a background task that polls for active window changes and sends events to the channel.
    pub fn start(&self) -> mpsc::UnboundedReceiver<ActiveAppEvent> {
        let (tx, rx) = mpsc::unbounded_channel();
        let interval = self.poll_interval;

        tokio::spawn(async move {
            let mut last_app = String::new();
            let mut last_title = String::new();

            loop {
                tokio::time::sleep(interval).await;

                if let Ok(window) = get_active_window() {
                    let app_name = window.app_name;
                    let title = window.title;
                    let process_id = window.process_id;

                    if app_name != last_app || title != last_title {
                        debug!("Active window changed: app='{}', title='{}', pid={}", app_name, title, process_id);
                        last_app = app_name.clone();
                        last_title = title.clone();

                        let event = ActiveAppEvent {
                            app_name,
                            title,
                            process_id,
                        };

                        if tx.send(event).is_err() {
                            info!("WindowWatcher channel closed. Stopping watcher task.");
                            break;
                        }
                    }
                }
            }
        });

        rx
    }
}
