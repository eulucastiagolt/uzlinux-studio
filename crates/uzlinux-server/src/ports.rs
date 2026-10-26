use std::net::TcpListener;
use anyhow::{Result, anyhow};
use log::info;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerPorts {
    pub plugin_ws: u16,
    pub pi_ws: u16,
    pub http_assets: u16,
}

impl ServerPorts {
    /// Attempts to bind 3 consecutive available ports, starting from `start_port` (default 57116).
    pub fn allocate(start_port: u16) -> Result<Self> {
        let mut port = start_port;
        // Search up to 1000 ports ahead
        while port < start_port.saturating_add(1000) {
            if is_port_available(port)
                && is_port_available(port + 1)
                && is_port_available(port + 2)
            {
                let ports = Self {
                    plugin_ws: port,
                    pi_ws: port + 1,
                    http_assets: port + 2,
                };
                info!("Allocated server ports: plugin_ws={}, pi_ws={}, http_assets={}", ports.plugin_ws, ports.pi_ws, ports.http_assets);
                return Ok(ports);
            }
            port += 3;
        }

        Err(anyhow!("Could not find 3 consecutive available ports starting from {}", start_port))
    }
}

fn is_port_available(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}
