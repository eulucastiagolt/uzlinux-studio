use std::sync::Arc;
use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use uzlinux_protocol::{InboundEvent, OutboundEvent, RegistrationMessage};

pub type InboundSender = mpsc::UnboundedSender<(String, InboundEvent)>;
pub type InboundReceiver = mpsc::UnboundedReceiver<(String, InboundEvent)>;

#[derive(Clone)]
pub struct PluginHub {
    port: u16,
    // Active connections: plugin_uuid -> sender
    connections: Arc<DashMap<String, mpsc::UnboundedSender<Message>>>,
    // Mapping of context -> plugin_uuid for fast routing
    context_to_plugin: Arc<DashMap<String, String>>,
    inbound_tx: InboundSender,
}

impl PluginHub {
    pub fn new(port: u16) -> (Self, InboundReceiver) {
        let (inbound_tx, inbound_rx) = mpsc::unbounded_channel();
        let hub = Self {
            port,
            connections: Arc::new(DashMap::new()),
            context_to_plugin: Arc::new(DashMap::new()),
            inbound_tx,
        };
        (hub, inbound_rx)
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn register_context(&self, context: String, plugin_uuid: String) {
        self.context_to_plugin.insert(context, plugin_uuid);
    }

    pub fn unregister_context(&self, context: &str) {
        self.context_to_plugin.remove(context);
    }

    pub fn get_plugin_for_context(&self, context: &str) -> Option<String> {
        self.context_to_plugin.get(context).map(|r| r.value().clone())
    }

    pub fn is_plugin_connected(&self, plugin_uuid: &str) -> bool {
        if self.connections.contains_key(plugin_uuid) {
            return true;
        }
        if let Some(bare) = plugin_uuid.strip_suffix(".sdPlugin") {
            if self.connections.contains_key(bare) {
                return true;
            }
        } else {
            let with_ext = format!("{}.sdPlugin", plugin_uuid);
            if self.connections.contains_key(&with_ext) {
                return true;
            }
        }
        false
    }

    pub fn list_connected_plugins(&self) -> Vec<String> {
        self.connections.iter().map(|kv| kv.key().clone()).collect()
    }

    pub fn send_to_plugin(&self, plugin_uuid: &str, event: &OutboundEvent) -> bool {
        let sender_opt = self.connections.get(plugin_uuid)
            .or_else(|| {
                plugin_uuid.strip_suffix(".sdPlugin").and_then(|bare| self.connections.get(bare))
            })
            .or_else(|| {
                let with_ext = format!("{}.sdPlugin", plugin_uuid);
                self.connections.get(&with_ext)
            });

        if let Some(sender) = sender_opt {
            match serde_json::to_string(event) {
                Ok(json) => sender.send(Message::Text(json.into())).is_ok(),
                Err(e) => {
                    error!("Failed to serialize outbound event for plugin {}: {}", plugin_uuid, e);
                    false
                }
            }
        } else {
            false
        }
    }

    pub fn broadcast(&self, event: &OutboundEvent) {
        if let Ok(json) = serde_json::to_string(event) {
            let msg = Message::Text(json.into());
            for entry in self.connections.iter() {
                let _ = entry.value().send(msg.clone());
            }
        }
    }

    pub fn send_raw_to_plugin(&self, plugin_uuid: &str, json: &serde_json::Value) -> bool {
        if let Some(sender) = self.connections.get(plugin_uuid) {
            match serde_json::to_string(json) {
                Ok(text) => sender.send(Message::Text(text.into())).is_ok(),
                Err(e) => {
                    error!("Failed to serialize raw json for plugin {}: {}", plugin_uuid, e);
                    false
                }
            }
        } else {
            false
        }
    }

    pub fn broadcast_raw(&self, json: &serde_json::Value) {
        if let Ok(text) = serde_json::to_string(json) {
            let msg = Message::Text(text.into());
            for entry in self.connections.iter() {
                let _ = entry.value().send(msg.clone());
            }
        }
    }

    pub async fn start(&self) -> Result<(), std::io::Error> {
        let addr_v4 = format!("127.0.0.1:{}", self.port);
        let listener_v4 = TcpListener::bind(&addr_v4).await?;
        info!("Plugin WebSocket Hub listening on ws://{}", addr_v4);

        let hub = self.clone();
        tokio::spawn(async move {
            while let Ok((stream, peer_addr)) = listener_v4.accept().await {
                let hub_clone = hub.clone();
                tokio::spawn(async move {
                    if let Err(e) = hub_clone.handle_connection(stream).await {
                        warn!("Connection error with {}: {}", peer_addr, e);
                    }
                });
            }
        });

        // Also attempt IPv6 loopback binding so ws://localhost: never fails
        let addr_v6 = format!("[::1]:{}", self.port);
        if let Ok(listener_v6) = TcpListener::bind(&addr_v6).await {
            info!("Plugin WebSocket Hub listening on ws://{}", addr_v6);
            let hub_v6 = self.clone();
            tokio::spawn(async move {
                while let Ok((stream, peer_addr)) = listener_v6.accept().await {
                    let hub_clone = hub_v6.clone();
                    tokio::spawn(async move {
                        if let Err(e) = hub_clone.handle_connection(stream).await {
                            warn!("Connection error with {}: {}", peer_addr, e);
                        }
                    });
                }
            });
        }

        Ok(())
    }

    async fn handle_connection(&self, stream: TcpStream) -> anyhow::Result<()> {
        let ws_stream = tokio_tungstenite::accept_async(stream).await?;
        let (mut ws_write, mut ws_read) = ws_stream.split();

        // 1. Await registration message
        let first_msg = match ws_read.next().await {
            Some(Ok(Message::Text(text))) => text,
            Some(Ok(other)) => {
                anyhow::bail!("Unexpected initial message type: {:?}", other);
            }
            Some(Err(e)) => anyhow::bail!("Error reading registration: {}", e),
            None => anyhow::bail!("Connection closed before registration"),
        };

        let registration: RegistrationMessage = serde_json::from_str(&first_msg)
            .map_err(|e| anyhow::anyhow!("Failed to parse registration message '{}': {}", first_msg, e))?;

        let plugin_uuid = registration.uuid.clone();
        info!("Plugin registered successfully: UUID='{}', event='{}'", plugin_uuid, registration.event);

        // 2. Set up outgoing message queue
        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
        self.connections.insert(plugin_uuid.clone(), tx.clone());
        if plugin_uuid.ends_with(".sdPlugin") {
            let bare = plugin_uuid.trim_end_matches(".sdPlugin").to_string();
            self.connections.insert(bare, tx.clone());
        } else {
            let with_ext = format!("{}.sdPlugin", plugin_uuid);
            self.connections.insert(with_ext, tx.clone());
        }

        // Task to write messages to websocket
        let write_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Err(e) = ws_write.send(msg).await {
                    warn!("Failed to send WS message: {}", e);
                    break;
                }
            }
        });

        // 3. Read incoming messages from plugin
        let connections = self.connections.clone();
        let inbound_tx = self.inbound_tx.clone();
        let plugin_uuid_read = plugin_uuid.clone();

        while let Some(msg_res) = ws_read.next().await {
            match msg_res {
                Ok(Message::Text(text)) => {
                    match serde_json::from_str::<InboundEvent>(&text) {
                        Ok(event) => {
                            let _ = inbound_tx.send((plugin_uuid_read.clone(), event));
                        }
                        Err(e) => {
                            warn!("Failed to deserialize inbound event from plugin '{}': {} | text: {}", plugin_uuid_read, e, text);
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    info!("Plugin '{}' requested connection close", plugin_uuid_read);
                    break;
                }
                Ok(Message::Ping(_)) => {}
                Ok(Message::Pong(_)) => {}
                Ok(Message::Binary(_)) => {}
                Ok(Message::Frame(_)) => {}
                Err(e) => {
                    warn!("WebSocket read error from plugin '{}': {}", plugin_uuid_read, e);
                    break;
                }
            }
        }

        // Cleanup
        connections.remove(&plugin_uuid);
        if plugin_uuid.ends_with(".sdPlugin") {
            let bare = plugin_uuid.trim_end_matches(".sdPlugin");
            connections.remove(bare);
        } else {
            let with_ext = format!("{}.sdPlugin", plugin_uuid);
            connections.remove(&with_ext);
        }
        write_task.abort();
        info!("Plugin '{}' disconnected", plugin_uuid);

        Ok(())
    }
}
