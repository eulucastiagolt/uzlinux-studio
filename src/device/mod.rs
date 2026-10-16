#![allow(dead_code)]

pub mod protocol;

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};
use std::thread;
use std::time::Duration;

pub use protocol::{DeckEvent, HID_CHUNK_SIZE};

#[derive(Clone)]
pub struct UlanziDeck {
    write_file: Arc<Mutex<Option<File>>>,
    device_path: Arc<Mutex<Option<PathBuf>>>,
    is_connected: Arc<AtomicBool>,
}

impl UlanziDeck {
    pub fn new() -> Self {
        Self {
            write_file: Arc::new(Mutex::new(None)),
            device_path: Arc::new(Mutex::new(None)),
            is_connected: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_connected(&self) -> bool {
        self.is_connected.load(Ordering::SeqCst)
    }

    pub fn current_path(&self) -> Option<PathBuf> {
        self.device_path.lock().unwrap().clone()
    }

    /// Automatically discovers the Ulanzi Deck HID device node in /dev/hidraw*
    pub fn find_device_node() -> Option<PathBuf> {
        // First inspect /sys/class/hidraw entries
        if let Ok(entries) = std::fs::read_dir("/sys/class/hidraw") {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let uevent_path = entry.path().join("device/uevent");
                if let Ok(content) = std::fs::read_to_string(&uevent_path) {
                    let content_lower = content.to_lowercase();
                    // Rockchip / Ulanzi VID 2207, PID 0019
                    if content_lower.contains("ulanzi")
                        || (content_lower.contains("2207") && content_lower.contains("0019"))
                    {
                        let dev_path = PathBuf::from(format!("/dev/{}", name));
                        // Verify we can access this node
                        if dev_path.exists() {
                            return Some(dev_path);
                        }
                    }
                }
            }
        }

        // Fallback checks
        for candidate in ["/dev/hidraw0", "/dev/hidraw1", "/dev/hidraw2"] {
            let p = Path::new(candidate);
            if p.exists() {
                return Some(p.to_path_buf());
            }
        }

        None
    }

    /// Connects to the device and starts the background event listener thread
    pub fn connect(&self, event_sender: mpsc::Sender<DeckEvent>) -> Result<PathBuf, String> {
        let node_path = Self::find_device_node().ok_or_else(|| {
            "Nenhum dispositivo Ulanzi Deck encontrado em /dev/hidraw*".to_string()
        })?;

        let read_file = OpenOptions::new()
            .read(true)
            .open(&node_path)
            .map_err(|e| format!("Erro ao abrir leitura em {}: {}", node_path.display(), e))?;

        let write_file = OpenOptions::new()
            .write(true)
            .open(&node_path)
            .map_err(|e| format!("Erro ao abrir escrita em {}: {}", node_path.display(), e))?;

        {
            let mut w = self.write_file.lock().unwrap();
            *w = Some(write_file);
        }
        {
            let mut p = self.device_path.lock().unwrap();
            *p = Some(node_path.clone());
        }
        self.is_connected.store(true, Ordering::SeqCst);

        // Spawn background reader thread
        let is_connected_flag = Arc::clone(&self.is_connected);
        let write_handle_clone = Arc::clone(&self.write_file);
        let path_clone = node_path.clone();

        thread::Builder::new()
            .name("ulanzi-hid-reader".into())
            .spawn(move || {
                let mut reader = read_file;
                let mut buf = [0u8; HID_CHUNK_SIZE];

                while is_connected_flag.load(Ordering::SeqCst) {
                    match reader.read(&mut buf) {
                        Ok(n) if n > 0 => {
                            if let Some(event) = protocol::parse_packet(&buf[..n]) {
                                let _ = event_sender.send(event);
                            }
                        }
                        Ok(_) => {
                            thread::sleep(Duration::from_millis(10));
                        }
                        Err(e) => {
                            eprintln!("Erro ao ler do dispositivo {}: {}", path_clone.display(), e);
                            is_connected_flag.store(false, Ordering::SeqCst);
                            let mut w = write_handle_clone.lock().unwrap();
                            *w = None;
                            break;
                        }
                    }
                }
            })
            .map_err(|e| format!("Falha ao iniciar thread de leitura HID: {}", e))?;

        Ok(node_path)
    }

    /// Sends raw packet to the device
    pub fn send_packet(&self, packet: &[u8]) -> Result<(), String> {
        let mut guard = self.write_file.lock().unwrap();
        if let Some(ref mut file) = *guard {
            file.write_all(packet)
                .map_err(|e| format!("Erro ao enviar pacote para o dispositivo: {}", e))?;
            file.flush()
                .map_err(|e| format!("Erro ao dar flush no pacote: {}", e))?;
            Ok(())
        } else {
            Err("Dispositivo desconectado".to_string())
        }
    }

    /// Sets screen brightness (0 to 100)
    pub fn set_brightness(&self, level: u8) -> Result<(), String> {
        let clamped = level.min(100);
        let payload = clamped.to_string();
        let packet = protocol::build_packet(
            protocol::PROTOCOL_HOST_CMD,
            protocol::CMD_SET_BRIGHTNESS,
            payload.as_bytes(),
        );
        self.send_packet(&packet)
    }

    /// Sets sleep timeout in seconds (0 = never)
    pub fn set_sleep_time(&self, seconds: u32) -> Result<(), String> {
        let payload = seconds.to_string();
        let packet = protocol::build_packet(
            protocol::PROTOCOL_HOST_CMD,
            protocol::CMD_SET_SLEEPTIME,
            payload.as_bytes(),
        );
        self.send_packet(&packet)
    }

    /// Sends an icon / image to a specific key slot (0 to 14)
    pub fn send_key_image(&self, slot: u8, image_bytes: &[u8]) -> Result<(), String> {
        let mut payload = Vec::with_capacity(1 + image_bytes.len());
        payload.push(slot);
        payload.extend_from_slice(image_bytes);
        let packet = protocol::build_packet(
            protocol::PROTOCOL_HOST_CMD,
            protocol::CMD_SEND_IMAGE_SLOT,
            &payload,
        );
        self.send_packet(&packet)
    }

    /// Sends raw screen data to the deck LCD
    pub fn send_screen_data(&self, image_bytes: &[u8]) -> Result<(), String> {
        let packet = protocol::build_packet(
            protocol::PROTOCOL_HOST_CMD,
            protocol::CMD_SEND_SCREEN_DATA,
            image_bytes,
        );
        self.send_packet(&packet)
    }

    /// Sends a general image packet to the deck LCD with a custom command code
    pub fn send_image(&self, cmd: u8, image_bytes: &[u8]) -> Result<(), String> {
        let packet = protocol::build_packet(protocol::PROTOCOL_HOST_CMD, cmd, image_bytes);
        self.send_packet(&packet)
    }
}
