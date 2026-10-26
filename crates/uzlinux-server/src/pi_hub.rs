use std::sync::Arc;
use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use uzlinux_protocol::{InboundEvent, RegistrationMessage};

pub type PiInboundSender = mpsc::UnboundedSender<(String /* context */, InboundEvent)>;
pub type PiInboundReceiver = mpsc::UnboundedReceiver<(String /* context */, InboundEvent)>;

#[derive(Clone)]
pub struct PropertyInspectorHub {
    port: u16,
    // Active connections: context -> sender
    connections: Arc<DashMap<String, mpsc::UnboundedSender<Message>>>,
    // Cached PI events by key (context, action, "apps", "sinks")
    cached_events: Arc<DashMap<String, serde_json::Value>>,
    inbound_tx: PiInboundSender,
}

impl PropertyInspectorHub {
    pub fn new(port: u16) -> (Self, PiInboundReceiver) {
        let (inbound_tx, inbound_rx) = mpsc::unbounded_channel();
        let hub = Self {
            port,
            connections: Arc::new(DashMap::new()),
            cached_events: Arc::new(DashMap::new()),
            inbound_tx,
        };
        (hub, inbound_rx)
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn send_to_property_inspector(&self, context: &str, event_json: &serde_json::Value) -> bool {
        // Cache event so newly opened Property Inspectors can receive data (such as apps list)
        if !context.is_empty() {
            self.cached_events.insert(context.to_string(), event_json.clone());
        }
        if let Some(action) = event_json.get("action").and_then(|a| a.as_str()) {
            if !action.is_empty() {
                self.cached_events.insert(action.to_string(), event_json.clone());
            }
        }
        if let Some(payload) = event_json.get("payload") {
            if payload.get("apps").is_some() {
                self.cached_events.insert("apps".to_string(), event_json.clone());
            }
            if payload.get("sinks").is_some() {
                self.cached_events.insert("sinks".to_string(), event_json.clone());
            }
        }

        // Direct delivery to matching context connection
        if let Some(sender) = self.connections.get(context) {
            match serde_json::to_string(event_json) {
                Ok(json) => return sender.send(Message::Text(json.into())).is_ok(),
                Err(e) => {
                    error!("Failed to serialize PI event for context {}: {}", context, e);
                    return false;
                }
            }
        }

        // Fallback: If there is exactly one Property Inspector active in the UI, deliver to it!
        if self.connections.len() == 1 {
            if let Some(entry) = self.connections.iter().next() {
                let target_context = entry.key().clone();
                let mut modified = event_json.clone();
                if let Some(obj) = modified.as_object_mut() {
                    obj.insert("context".to_string(), serde_json::Value::String(target_context.clone()));
                }
                if let Ok(json) = serde_json::to_string(&modified) {
                    return entry.value().send(Message::Text(json.into())).is_ok();
                }
            }
        }

        false
    }

    pub async fn start(&self) -> Result<(), std::io::Error> {
        let addr_v4 = format!("127.0.0.1:{}", self.port);
        let listener_v4 = TcpListener::bind(&addr_v4).await?;
        info!("Property Inspector WebSocket Hub listening on ws://{}", addr_v4);

        let hub = self.clone();
        tokio::spawn(async move {
            while let Ok((stream, peer_addr)) = listener_v4.accept().await {
                let hub_clone = hub.clone();
                tokio::spawn(async move {
                    if let Err(e) = hub_clone.handle_connection(stream).await {
                        warn!("PI Connection error with {}: {}", peer_addr, e);
                    }
                });
            }
        });

        // Also attempt IPv6 loopback binding so ws://localhost: never fails on IPv6-first systems
        let addr_v6 = format!("[::1]:{}", self.port);
        if let Ok(listener_v6) = TcpListener::bind(&addr_v6).await {
            info!("Property Inspector WebSocket Hub listening on ws://{}", addr_v6);
            let hub_v6 = self.clone();
            tokio::spawn(async move {
                while let Ok((stream, peer_addr)) = listener_v6.accept().await {
                    let hub_clone = hub_v6.clone();
                    tokio::spawn(async move {
                        if let Err(e) = hub_clone.handle_connection(stream).await {
                            warn!("PI Connection error with {}: {}", peer_addr, e);
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
            Some(Ok(other)) => anyhow::bail!("Unexpected PI initial message: {:?}", other),
            Some(Err(e)) => anyhow::bail!("Error reading PI registration: {}", e),
            None => anyhow::bail!("PI Connection closed before registration"),
        };

        let registration: RegistrationMessage = serde_json::from_str(&first_msg)
            .map_err(|e| anyhow::anyhow!("Failed to parse PI registration '{}': {}", first_msg, e))?;

        let context = registration.uuid.clone();
        info!("Property Inspector registered successfully for context='{}'", context);

        // 2. Set up outgoing queue
        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
        self.connections.insert(context.clone(), tx.clone());

        // Prime the newly connected Property Inspector with current settings
        let _ = self.inbound_tx.send((context.clone(), InboundEvent::GetSettings(uzlinux_protocol::ContextOnlyMessage {
            context: context.clone(),
        })));
        let _ = self.inbound_tx.send((context.clone(), InboundEvent::GetGlobalSettings(uzlinux_protocol::ContextOnlyMessage {
            context: context.clone(),
        })));

        // Immediately replay any cached events (e.g. apps list from oadesktopentry, audio sinks, or context settings)
        if let Some(apps_ev) = self.cached_events.get("apps") {
            let mut ev = apps_ev.clone();
            if let Some(obj) = ev.as_object_mut() {
                obj.insert("context".to_string(), serde_json::Value::String(context.clone()));
            }
            if let Ok(json) = serde_json::to_string(&ev) {
                let _ = tx.send(Message::Text(json.into()));
            }
        }

        if let Some(sinks_ev) = self.cached_events.get("sinks") {
            let mut ev = sinks_ev.clone();
            if let Some(obj) = ev.as_object_mut() {
                obj.insert("context".to_string(), serde_json::Value::String(context.clone()));
            }
            if let Ok(json) = serde_json::to_string(&ev) {
                let _ = tx.send(Message::Text(json.into()));
            }
        }

        if let Some(cached) = self.cached_events.get(&context) {
            if let Ok(json) = serde_json::to_string(&*cached) {
                let _ = tx.send(Message::Text(json.into()));
            }
        }

        let write_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Err(e) = ws_write.send(msg).await {
                    warn!("Failed to send to Property Inspector: {}", e);
                    break;
                }
            }
        });

        // 3. Read incoming messages from PI
        let connections = self.connections.clone();
        let inbound_tx = self.inbound_tx.clone();
        let context_read = context.clone();

        while let Some(msg_res) = ws_read.next().await {
            match msg_res {
                Ok(Message::Text(text)) => {
                    match serde_json::from_str::<InboundEvent>(&text) {
                        Ok(event) => {
                            let _ = inbound_tx.send((context_read.clone(), event));
                        }
                        Err(e) => {
                            warn!("Failed to deserialize inbound event from PI '{}': {} | text: {}", context_read, e, text);
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    info!("Property Inspector '{}' closed connection", context_read);
                    break;
                }
                Ok(_) => {}
                Err(e) => {
                    warn!("WebSocket error from PI '{}': {}", context_read, e);
                    break;
                }
            }
        }

        connections.remove(&context);
        write_task.abort();
        info!("Property Inspector for '{}' disconnected", context);

        Ok(())
    }
}
