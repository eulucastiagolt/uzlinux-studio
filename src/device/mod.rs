#![allow(dead_code)]

pub mod protocol;

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};
use std::thread;
use std::time::Duration;

pub use protocol::{DeckEvent, HID_CHUNK_SIZE};
use std::sync::atomic::AtomicU8;

pub fn get_system_stats() -> (u8, u8) {
    let mut cpu_pct = 0u8;
    let mut mem_pct = 0u8;

    // RAM usage from /proc/meminfo
    if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
        let mut total = 0u64;
        let mut avail = 0u64;
        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                total = line.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
            } else if line.starts_with("MemAvailable:") {
                avail = line.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
            }
        }
        if total > 0 && avail <= total {
            mem_pct = (((total - avail) * 100) / total).min(100) as u8;
        }
    }

    // CPU load average from /proc/loadavg
    if let Ok(content) = std::fs::read_to_string("/proc/loadavg") {
        if let Some(first) = content.split_whitespace().next() {
            if let Ok(load) = first.parse::<f32>() {
                let num_cpus = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f32;
                cpu_pct = ((load / num_cpus) * 100.0).clamp(0.0, 100.0) as u8;
            }
        }
    }

    (cpu_pct, mem_pct)
}

#[derive(Clone)]
pub struct UlanziDeck {
    write_file: Arc<Mutex<Option<File>>>,
    device_path: Arc<Mutex<Option<PathBuf>>>,
    is_connected: Arc<AtomicBool>,
    small_window_mode: Arc<AtomicU8>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeviceDiscovery {
    Found(PathBuf),
    FoundNoPermission(PathBuf),
    InRecoveryModeAdb,
    NotFound,
}

impl UlanziDeck {
    pub fn new() -> Self {
        Self {
            write_file: Arc::new(Mutex::new(None)),
            device_path: Arc::new(Mutex::new(None)),
            is_connected: Arc::new(AtomicBool::new(false)),
            small_window_mode: Arc::new(AtomicU8::new(1)),
        }
    }

    pub fn set_small_window_mode(&self, mode: u8) {
        self.small_window_mode.store(mode, Ordering::SeqCst);
    }

    pub fn is_connected(&self) -> bool {
        self.is_connected.load(Ordering::SeqCst)
    }

    pub fn current_path(&self) -> Option<PathBuf> {
        self.device_path.lock().unwrap().clone()
    }

    /// Discovers the Ulanzi Deck status across HID and USB buses
    pub fn discover_device() -> DeviceDiscovery {
        // 1. Inspect /sys/class/hidraw entries for operational Ulanzi composite device (2207:0019)
        // Note: The D200 exposes two HID interfaces:
        // - input0: 1024-byte vendor control report descriptor (for commands, LCD images, buttons, dial)
        // - input1: 63-byte standard USB keyboard interface
        // We collect candidates and prioritize input0.
        let mut candidates = Vec::new();
        if let Ok(entries) = std::fs::read_dir("/sys/class/hidraw") {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let uevent_path = entry.path().join("device/uevent");
                if let Ok(content) = std::fs::read_to_string(&uevent_path) {
                    let content_lower = content.to_lowercase();
                    // Rockchip / Ulanzi VID 2207, PID 0019 or explicit product names
                    if content_lower.contains("ulanzi")
                        || (content_lower.contains("2207") && content_lower.contains("0019"))
                        || content_lower.contains("d200")
                    {
                        let dev_path = PathBuf::from(format!("/dev/{}", name));
                        if dev_path.exists() {
                            let is_input0 = content_lower.contains("input0");
                            candidates.push((dev_path, is_input0));
                        }
                    }
                }
            }
        }

        // Sort so that input0 comes first
        candidates.sort_by(|a, b| b.1.cmp(&a.1));

        for (dev_path, _) in candidates {
            // Check read/write permission
            if OpenOptions::new().read(true).write(true).open(&dev_path).is_ok() {
                return DeviceDiscovery::Found(dev_path);
            } else {
                return DeviceDiscovery::FoundNoPermission(dev_path);
            }
        }

        // 2. Check if device is plugged in but stuck in USB recovery mode (18d1:d002 or Zkswe / ulanzi)
        if let Ok(usb_entries) = std::fs::read_dir("/sys/bus/usb/devices") {
            for entry in usb_entries.flatten() {
                let p = entry.path();
                let vid = std::fs::read_to_string(p.join("idVendor"))
                    .unwrap_or_default()
                    .trim()
                    .to_lowercase();
                let pid = std::fs::read_to_string(p.join("idProduct"))
                    .unwrap_or_default()
                    .trim()
                    .to_lowercase();
                let prod = std::fs::read_to_string(p.join("product"))
                    .unwrap_or_default()
                    .trim()
                    .to_lowercase();
                let mfr = std::fs::read_to_string(p.join("manufacturer"))
                    .unwrap_or_default()
                    .trim()
                    .to_lowercase();

                if (vid == "18d1" && pid == "d002")
                    || prod.contains("ulanzi")
                    || mfr.contains("zkswe")
                {
                    return DeviceDiscovery::InRecoveryModeAdb;
                }
            }
        }

        DeviceDiscovery::NotFound
    }

    /// Automatically discovers the Ulanzi Deck HID device node in /dev/hidraw*
    pub fn find_device_node() -> Option<PathBuf> {
        if let DeviceDiscovery::Found(p) = Self::discover_device() {
            Some(p)
        } else {
            None
        }
    }

    /// Connects to the device and starts the background event listener thread
    pub fn connect(&self, event_sender: mpsc::Sender<DeckEvent>) -> Result<PathBuf, String> {
        let node_path = match Self::discover_device() {
            DeviceDiscovery::Found(path) => path,
            DeviceDiscovery::FoundNoPermission(path) => {
                return Err(format!(
                    "Dispositivo Ulanzi em {} detectado, mas requer permissão. Execute: ./scripts/setup-udev.sh",
                    path.display()
                ));
            }
            DeviceDiscovery::InRecoveryModeAdb => {
                return Err(
                    "Ulanzi D200 em modo recovery (18d1:d002). Conecte em um HUB USB 2.0 ou porta USB 2.0 para inicializar como HID (2207:0019).".to_string()
                );
            }
            DeviceDiscovery::NotFound => {
                return Err(
                    "Nenhum Ulanzi Deck conectado. Conecte o cabo USB do dispositivo.".to_string()
                );
            }
        };

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

        // Spawn background keepalive & small window display update thread
        let is_connected_keepalive = Arc::clone(&self.is_connected);
        let write_keepalive = Arc::clone(&self.write_file);
        let sw_mode_keepalive = Arc::clone(&self.small_window_mode);
        let _ = thread::Builder::new()
            .name("ulanzi-keepalive".into())
            .spawn(move || {
                while is_connected_keepalive.load(Ordering::SeqCst) {
                    let mode = sw_mode_keepalive.load(Ordering::SeqCst);
                    let now = chrono::Local::now().format("%H:%M:%S").to_string();
                    let payload = if mode == 0 {
                        let (cpu, mem) = get_system_stats();
                        format!("0|{}|{}|{}|0", cpu, mem, now)
                    } else {
                        format!("1|0|0|{}|0", now)
                    };
                    let packet = protocol::build_control_packet(
                        protocol::CMD_OUT_SET_SMALL_WINDOW_DATA,
                        payload.as_bytes(),
                    );
                    {
                        let mut w = write_keepalive.lock().unwrap();
                        if let Some(ref mut file) = *w {
                            if let Err(e) = file.write_all(&packet).and_then(|_| file.flush()) {
                                eprintln!("[Keepalive] Erro ao enviar keepalive ao D200: {}", e);
                                is_connected_keepalive.store(false, Ordering::SeqCst);
                                *w = None;
                                break;
                            }
                        }
                    }
                    let sleep_secs = if mode == 0 { 2 } else { 1 };
                    thread::sleep(Duration::from_secs(sleep_secs));
                }
            });

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

    /// Sends a 1024-byte control packet with header to the device
    pub fn send_control_packet(&self, command: u16, payload: &[u8]) -> Result<(), String> {
        let packet = protocol::build_control_packet(command, payload);
        self.send_packet(&packet)
    }

    /// Queries the connected device for firmware version and device serial number
    pub fn query_device_info(&self) -> Result<(), String> {
        let ver_pkt = protocol::build_control_packet(0x0005, &[0x01, 0x04, 0x04, 0x01]);
        let _ = self.send_packet(&ver_pkt);
        let sn_pkt = protocol::build_control_packet(0x001D, &[0x01, 0x01, 0x0B, 0x01]);
        let _ = self.send_packet(&sn_pkt);
        Ok(())
    }

    /// Sets screen brightness (0 to 100)
    pub fn set_brightness(&self, level: u8) -> Result<(), String> {
        let clamped = level.min(100);
        let payload = clamped.to_string();
        self.send_control_packet(protocol::CMD_OUT_SET_BRIGHTNESS, payload.as_bytes())
    }

    /// Sets sleep timeout in seconds (0 = never)
    pub fn set_sleep_time(&self, seconds: u32) -> Result<(), String> {
        let payload = seconds.to_string();
        self.send_control_packet(0x001A, payload.as_bytes())
    }

    /// Sets small window data (mode, cpu, mem, time, gpu)
    /// Mode 0 = stats, Mode 1 = clock, Mode 2 = image
    pub fn set_small_window_data(&self, mode: u8, cpu: u8, mem: u8, time_str: &str, gpu: u8) -> Result<(), String> {
        self.small_window_mode.store(mode, Ordering::SeqCst);
        let payload = format!("{}|{}|{}|{}|{}", mode, cpu, mem, time_str, gpu);
        self.send_control_packet(protocol::CMD_OUT_SET_SMALL_WINDOW_DATA, payload.as_bytes())
    }

    /// Sets label text style
    pub fn set_label_style(&self, show_title: bool) -> Result<(), String> {
        let style = serde_json::json!({
            "Align": "bottom",
            "Color": 0xFFFFFF,
            "FontName": "Roboto",
            "ShowTitle": show_title,
            "Size": 10,
            "Weight": 80
        });
        let payload = serde_json::to_vec(&style).map_err(|e| e.to_string())?;
        self.send_control_packet(protocol::CMD_OUT_SET_LABEL_STYLE, &payload)
    }

    /// Streams a full ZIP bundle (manifest.json + PNG icons) to the deck hardware
    pub fn send_profile_bundle(&self, bundle_data: &[u8]) -> Result<(), String> {
        let packets = protocol::build_bundle_packets(bundle_data);
        let mut guard = self.write_file.lock().unwrap();
        if let Some(ref mut file) = *guard {
            for pkt in packets {
                file.write_all(&pkt)
                    .map_err(|e| format!("Erro ao enviar pacote HID do bundle: {}", e))?;
                file.flush()
                    .map_err(|e| format!("Erro no flush HID: {}", e))?;
                thread::sleep(Duration::from_millis(2));
            }
            Ok(())
        } else {
            Err("Dispositivo desconectado".to_string())
        }
    }

    /// Sends a partial update for a single key using CMD_OUT_PARTIALLY_UPDATE_BUTTONS (0x000D).
    /// This is the fast path for real-time sync: only the changed key is re-rendered and sent,
    /// resulting in ~10-15KB transfers (~30ms) instead of full 422KB bundle (~2.5s).
    /// Falls back to returning an error if the key is a no-op (phantom key 14, clock mode, etc.)
    pub fn send_key_update(&self, key: &protocol::ProfileKeyInfo) -> Result<(), String> {
        let bundle = protocol::build_single_key_zip(key)?;
        let packets = protocol::build_partial_update_packets(&bundle);
        let mut guard = self.write_file.lock().unwrap();
        if let Some(ref mut file) = *guard {
            for pkt in &packets {
                file.write_all(pkt)
                    .map_err(|e| format!("Erro ao enviar pacote HID parcial: {}", e))?;
                file.flush()
                    .map_err(|e| format!("Erro no flush HID parcial: {}", e))?;
            }
            Ok(())
        } else {
            Err("Dispositivo desconectado".to_string())
        }
    }
}

