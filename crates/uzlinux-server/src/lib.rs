pub mod ports;
pub mod plugin_hub;
pub mod pi_hub;
pub mod asset_server;

pub use ports::ServerPorts;
pub use plugin_hub::{PluginHub, InboundReceiver, InboundSender};
pub use pi_hub::{PropertyInspectorHub, PiInboundReceiver, PiInboundSender};
pub use asset_server::AssetServer;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_port_allocation() {
        let ports = ServerPorts::allocate(58000).expect("Should allocate ports");
        assert_eq!(ports.pi_ws, ports.plugin_ws + 1);
        assert_eq!(ports.http_assets, ports.plugin_ws + 2);
    }

    #[test]
    fn test_asset_server_serves_files() {
        let dir = tempdir().expect("Failed to create tempdir");
        let test_file = dir.path().join("index.html");
        fs::write(&test_file, "<h1>Hello Uzlinux</h1>").expect("Failed to write test file");

        let ports = ServerPorts::allocate(58100).expect("Should allocate ports");
        let asset_server = AssetServer::new(ports.http_assets, dir.path());
        asset_server.start().expect("Should start asset server");

        // Request using reqwest or std TcpStream
        use std::io::{Read, Write};
        use std::net::TcpStream;

        let mut stream = TcpStream::connect(("127.0.0.1", ports.http_assets)).expect("Should connect to asset server");
        stream.write_all(b"GET /index.html HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n").expect("Should send request");

        let mut response = String::new();
        stream.read_to_string(&mut response).expect("Should read response");
        assert!(response.contains("200 OK"));
        assert!(response.contains("<h1>Hello Uzlinux</h1>"));
        assert!(response.contains("Access-Control-Allow-Origin: *"));
    }
}
