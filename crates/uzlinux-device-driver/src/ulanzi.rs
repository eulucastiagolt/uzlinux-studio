use std::collections::HashMap;
use std::io::{Cursor, Write};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_hid::{AsyncHidRead, AsyncHidWrite, DeviceWriter, HidBackend};
use async_trait::async_trait;
use byteorder::{BigEndian, LittleEndian, WriteBytesExt};
use futures_util::StreamExt;
use log::{debug, error, info, warn};
use rand::Rng;
use rand::seq::SliceRandom;
use tokio::sync::{mpsc, Mutex as TokioMutex};
use tokio::time::Instant;
use uuid::Uuid;

use image::{DynamicImage, GenericImageView, RgbImage, RgbaImage};
use uzlinux_core::models::DeviceInfo;

use crate::event::DeviceEvent;
use crate::traits::StreamDeckDevice;

// ---------------------------------------------------------------------------
// Hardware Constants
// ---------------------------------------------------------------------------

pub const VENDOR_ID: u16 = 0x2207;
pub const PRODUCT_ID: u16 = 0x0019;
pub const DEVICE_NAMESPACE: &str = "e9";
pub const NUM_BUTTONS: usize = 14;

const PACKET_SIZE: usize = 1024;
const HEADER: [u8; 2] = [0x7c, 0x7c];
const USAGE_PAGE: u16 = 0x000c;
const MAX_COMMAND_PAYLOAD: usize = PACKET_SIZE - 8; // 1016

static FLUSH_COUNTER: AtomicU64 = AtomicU64::new(0);

// ---------------------------------------------------------------------------
// Command protocol
// ---------------------------------------------------------------------------

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CommandProtocol {
    OutSetButtons = 0x0001,
    OutSetSmallWindowData = 0x0006,
    OutSetBrightness = 0x000a,
    OutSetLabelStyle = 0x000b,
    InButton = 0x0101,
    InButton2 = 0x0102,
}

// ---------------------------------------------------------------------------
// Button event
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct ButtonEvent {
    pub index: usize,
    pub pressed: bool,
    pub state: u8,
}

pub fn parse_report(buf: &[u8]) -> Option<ButtonEvent> {
    if buf.len() < 12 || buf[0..2] != HEADER {
        return None;
    }

    let command = u16::from_be_bytes([buf[2], buf[3]]);
    if command != CommandProtocol::InButton as u16 && command != CommandProtocol::InButton2 as u16 {
        return None;
    }

    let index = buf[9] as usize;
    if index >= NUM_BUTTONS {
        warn!("Received button event with out-of-range index {}", index);
        return None;
    }

    Some(ButtonEvent {
        state: buf[8],
        index,
        pressed: buf[11] == 0x01,
    })
}

// ---------------------------------------------------------------------------
// Button image structure
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ButtonImageData {
    pub image: Vec<u8>,
    pub uuid: Uuid,
}

// ---------------------------------------------------------------------------
// Image Resizing Helper
// ---------------------------------------------------------------------------

fn resize_canvas(img: &DynamicImage, target_w: u32, target_h: u32) -> DynamicImage {
    let (w, h) = img.dimensions();
    if w == target_w && h == target_h {
        return img.clone();
    }

    let scale = (target_w as f64 / w as f64).min(target_h as f64 / h as f64);
    let new_w = (w as f64 * scale).round() as u32;
    let new_h = (h as f64 * scale).round() as u32;

    let resized = img.resize(new_w, new_h, image::imageops::FilterType::Triangle);

    let has_alpha = matches!(
        img.color(),
        image::ColorType::Rgba8 | image::ColorType::La8 | image::ColorType::Rgba16 | image::ColorType::La16
    );

    let x = ((target_w - new_w) / 2) as i64;
    let y = ((target_h - new_h) / 2) as i64;

    if has_alpha {
        let mut canvas = RgbaImage::from_pixel(target_w, target_h, image::Rgba([0, 0, 0, 0]));
        image::imageops::overlay(&mut canvas, &resized.to_rgba8(), x, y);
        DynamicImage::ImageRgba8(canvas)
    } else {
        let mut canvas = RgbImage::from_pixel(target_w, target_h, image::Rgb([0, 0, 0]));
        image::imageops::overlay(&mut canvas, &resized.to_rgb8(), x, y);
        DynamicImage::ImageRgb8(canvas)
    }
}

// ---------------------------------------------------------------------------
// Native Ulanzi D200 Device
// ---------------------------------------------------------------------------

pub struct UlanziD200Device {
    info: DeviceInfo,
    writer: Arc<TokioMutex<DeviceWriter>>,
    button_images: Arc<Mutex<HashMap<usize, ButtonImageData>>>,
    brightness: AtomicU8,
    flush_trigger_tx: mpsc::Sender<()>,
}

impl UlanziD200Device {
    /// Attempts to find and connect to a physical Ulanzi D200 / D200H on the system.
    pub async fn find_and_connect(
        event_tx: mpsc::UnboundedSender<(String, DeviceEvent)>,
    ) -> Result<Option<Arc<Self>>> {
        let backend = HidBackend::default();
        let devices: Vec<_> = {
            match backend.enumerate().await {
                Ok(stream) => stream.collect().await,
                Err(e) => {
                    warn!("Failed to enumerate HID devices: {}", e);
                    return Ok(None);
                }
            }
        };

        let ulanzi_info = devices.into_iter().find(|d| {
            d.vendor_id == VENDOR_ID && d.product_id == PRODUCT_ID && d.usage_page == USAGE_PAGE
        });

        let target_device = match ulanzi_info {
            Some(dev) => dev,
            None => {
                debug!("No physical Ulanzi D200 hardware found on HID bus");
                return Ok(None);
            }
        };

        let serial = target_device.serial_number.clone().unwrap_or_else(|| "02C53A015U3671380".to_string());
        info!("Found physical Ulanzi D200 hardware! Serial: {}", serial);

        let (reader, writer) = match target_device.open().await {
            Ok(pair) => pair,
            Err(e) => {
                warn!("Found Ulanzi D200 but failed to open HID device: {}", e);
                return Ok(None);
            }
        };

        let device_info = DeviceInfo::new_ulanzi_d200(&serial);
        let device_id = device_info.id.clone();
        let writer_arc = Arc::new(TokioMutex::new(writer));
        let button_images = Arc::new(Mutex::new(HashMap::new()));
        let (flush_trigger_tx, mut flush_trigger_rx) = mpsc::channel::<()>(50);

        // 1. Spawn Button Input Reader Task
        let read_dev_id = device_id.clone();
        let read_event_tx = event_tx.clone();
        let mut dev_reader = reader;
        tokio::spawn(async move {
            info!("Ulanzi D200 native input reader started for '{}'", read_dev_id);
            let mut buf = [0u8; 1024];
            loop {
                match dev_reader.read_input_report(&mut buf).await {
                    Ok(len) if len > 0 => {
                        if let Some(btn_event) = parse_report(&buf[..len]) {
                            let row = (btn_event.index as u8) / 5;
                            let column = (btn_event.index as u8) % 5;
                            let dev_ev = if btn_event.pressed {
                                DeviceEvent::KeyDown {
                                    row,
                                    column,
                                    key_index: btn_event.index as u8,
                                }
                            } else {
                                DeviceEvent::KeyUp {
                                    row,
                                    column,
                                    key_index: btn_event.index as u8,
                                }
                            };
                            if read_event_tx.send((read_dev_id.clone(), dev_ev)).is_err() {
                                break;
                            }
                        }
                    }
                    Ok(_) => continue,
                    Err(e) => {
                        warn!("Ulanzi D200 input reader disconnected: {}", e);
                        break;
                    }
                }
            }
            info!("Ulanzi D200 native input reader finished for '{}'", read_dev_id);
        });

        // 2. Spawn Debounced Button Images Flusher Task
        let flush_writer = writer_arc.clone();
        let flush_images = button_images.clone();
        let flush_dev_id = device_id.clone();
        tokio::spawn(async move {
            info!("Ulanzi D200 native button image flusher started for '{}'", flush_dev_id);
            while flush_trigger_rx.recv().await.is_some() {
                // Debounce: wait for any rapid successive calls within 35ms
                let deadline = Instant::now() + Duration::from_millis(35);
                while tokio::time::sleep_until(deadline).await == () {
                    // Check if more triggers arrived
                    if flush_trigger_rx.try_recv().is_err() {
                        break;
                    }
                }

                if let Err(e) = Self::flush_images_to_hardware(&flush_writer, &flush_images).await {
                    error!("Failed to flush images to Ulanzi D200 hardware: {}", e);
                }
            }
        });

        let device = Arc::new(Self {
            info: device_info,
            writer: writer_arc,
            button_images,
            brightness: AtomicU8::new(100),
            flush_trigger_tx,
        });

        // Initial reset: set brightness and clear keys
        let _ = device.set_brightness(100).await;

        info!("Ulanzi D200 native hardware driver successfully connected: {}", device_id);
        Ok(Some(device))
    }

    /// Triggers a debounced flush to push all staged button images to hardware.
    pub fn trigger_flush(&self) {
        let _ = self.flush_trigger_tx.try_send(());
    }

    /// Sets small window data (clock, pc stats, etc.)
    pub async fn set_small_window_data(
        &self,
        mode: u8,
        cpu: u8,
        mem: u8,
        time_str: &str,
        gpu: u8,
    ) -> Result<()> {
        let payload = format!("{}|{}|{}|{}|{}", mode, cpu, mem, time_str, gpu).into_bytes();
        self.send_command(CommandProtocol::OutSetSmallWindowData, &payload).await
    }

    async fn send_command(&self, command: CommandProtocol, payload: &[u8]) -> Result<()> {
        if payload.len() > MAX_COMMAND_PAYLOAD {
            return Err(anyhow!(
                "Command payload too large: {} bytes (max {})",
                payload.len(),
                MAX_COMMAND_PAYLOAD
            ));
        }
        let packet = Self::build_packet(command, payload, payload.len() as u32);
        self.writer.lock().await.write_output_report(&packet).await?;
        Ok(())
    }

    async fn flush_images_to_hardware(
        writer: &Arc<TokioMutex<DeviceWriter>>,
        button_images: &Arc<Mutex<HashMap<usize, ButtonImageData>>>,
    ) -> Result<()> {
        let images_snapshot = {
            let map = button_images.lock().unwrap();
            map.clone()
        };

        const INVALID_BYTES: [u8; 2] = [0x00, 0x7c];
        const MAX_RETRIES: usize = 1000;

        let mut dummy_retries = 0;
        let mut zip_data = Vec::new();

        loop {
            let _flush_id = FLUSH_COUNTER.fetch_add(1, Ordering::Relaxed);
            zip_data.clear();
            let mut cursor = Cursor::new(Vec::new());
            {
                let mut zip = zip::ZipWriter::new(&mut cursor);
                let deflated = zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated);

                // Dummy padding file to bypass Ulanzi firmware packet boundary quirk
                let dummy_content: String = rand::thread_rng()
                    .sample_iter(&rand::distributions::Alphanumeric)
                    .take(1024 * dummy_retries)
                    .map(char::from)
                    .collect();

                zip.start_file("dummy.txt", deflated)?;
                zip.write_all(dummy_content.as_bytes())?;

                let mut manifest = serde_json::json!({});

                let mut numbers: Vec<usize> = (0..NUM_BUTTONS).collect();
                numbers.shuffle(&mut rand::thread_rng());
                for (index, value) in numbers.into_iter().enumerate() {
                    let col = value % 5;
                    let row = value / 5;
                    let key = format!("{}_{}", col, row);
                    let mut view_param = serde_json::json!({ "Text": "" });

                    if let Some(img_data) = images_snapshot.get(&value) {
                        let icon_name = format!("{}_{}.png", index, img_data.uuid);
                        zip.start_file(format!("Images/{}", icon_name), deflated)?;
                        zip.write_all(&img_data.image)?;
                        view_param["Icon"] = serde_json::json!(format!("Images/{}", icon_name));
                    } else {
                        view_param["Icon"] = serde_json::json!("");
                    }

                    manifest[key] = serde_json::json!({ "State": 0, "ViewParam": [view_param] });
                }

                zip.start_file("manifest.json", deflated)?;
                zip.write_all(serde_json::to_string(&manifest)?.as_bytes())?;

                zip.start_file("sentinel.txt", deflated)?;
                zip.write_all(b"")?;

                zip.finish()?;
            }

            zip_data = cursor.into_inner();

            let file_size = zip_data.len();
            let mut valid = true;
            for offset in (92152..file_size).step_by(1024) {
                if let Some(&byte) = zip_data.get(offset) {
                    if INVALID_BYTES.contains(&byte) {
                        valid = false;
                        break;
                    }
                }
            }

            if valid {
                break;
            }

            dummy_retries += 1;
            if dummy_retries >= MAX_RETRIES {
                return Err(anyhow!("Failed to build a valid ZIP after {} retries", MAX_RETRIES));
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }

        Self::send_file_to_writer(writer, &zip_data).await?;
        debug!("Successfully flushed {} bytes to Ulanzi D200 buttons", zip_data.len());
        Ok(())
    }

    async fn send_file_to_writer(writer: &Arc<TokioMutex<DeviceWriter>>, data: &[u8]) -> Result<()> {
        let file_size = data.len() as u32;
        let first_chunk = if data.len() >= 1016 {
            &data[..1016]
        } else {
            data
        };
        let first_packet = Self::build_packet(CommandProtocol::OutSetButtons, first_chunk, file_size);

        let mut lock = writer.lock().await;
        lock.write_output_report(&first_packet).await?;

        if data.len() > 1016 {
            for chunk in data[1016..].chunks(1024) {
                let mut packet = [0u8; PACKET_SIZE];
                let len = chunk.len().min(PACKET_SIZE);
                packet[..len].copy_from_slice(&chunk[..len]);
                lock.write_output_report(&packet).await?;
            }
        }
        Ok(())
    }

    fn build_packet(command: CommandProtocol, data: &[u8], total_length: u32) -> Vec<u8> {
        let mut packet = Vec::with_capacity(PACKET_SIZE);
        packet.extend_from_slice(&HEADER);

        let mut cmd_buf = [0u8; 2];
        (&mut cmd_buf[..]).write_u16::<BigEndian>(command as u16).unwrap();
        packet.extend_from_slice(&cmd_buf);

        let mut len_buf = [0u8; 4];
        (&mut len_buf[..]).write_u32::<LittleEndian>(total_length).unwrap();
        packet.extend_from_slice(&len_buf);

        let data_len = data.len().min(PACKET_SIZE - 8);
        packet.extend_from_slice(&data[..data_len]);

        if packet.len() < PACKET_SIZE {
            packet.resize(PACKET_SIZE, 0);
        }
        packet
    }
}

// ---------------------------------------------------------------------------
// StreamDeckDevice Trait Implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl StreamDeckDevice for UlanziD200Device {
    fn id(&self) -> &str {
        &self.info.id
    }

    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn is_hardware(&self) -> bool {
        true
    }

    async fn set_brightness(&self, percentage: u8) -> Result<()> {
        let brightness = percentage.min(100);
        self.brightness.store(brightness, Ordering::Relaxed);
        let payload = brightness.to_string().into_bytes();
        self.send_command(CommandProtocol::OutSetBrightness, &payload).await?;
        info!("Ulanzi D200 brightness set to {}%", brightness);
        Ok(())
    }

    async fn set_key_image(&self, key_index: u8, image_bytes: &[u8]) -> Result<()> {
        if (key_index as usize) >= NUM_BUTTONS {
            return Ok(());
        }

        let idx = key_index as usize;
        let png_bytes = match image::load_from_memory(image_bytes) {
            Ok(img) => {
                // Key 13 is wide button: 392x196; other keys are 196x196
                let (tw, th) = if idx == 13 { (392, 196) } else { (196, 196) };
                let resized = resize_canvas(&img, tw, th);
                let mut buf = Vec::new();
                {
                    let mut cursor = Cursor::new(&mut buf);
                    resized.write_to(&mut cursor, image::ImageFormat::Png)?;
                }
                buf
            }
            Err(_) => image_bytes.to_vec(),
        };

        let changed = {
            let map = self.button_images.lock().unwrap();
            match map.get(&idx) {
                Some(existing) => existing.image != png_bytes,
                None => true,
            }
        };

        if changed {
            {
                let mut map = self.button_images.lock().unwrap();
                map.insert(idx, ButtonImageData {
                    image: png_bytes,
                    uuid: Uuid::new_v4(),
                });
            }
            self.trigger_flush();
        }

        Ok(())
    }

    async fn set_lcd_strip(&self, _image_bytes: &[u8]) -> Result<()> {
        Ok(())
    }

    async fn clear_all_keys(&self) -> Result<()> {
        {
            let mut map = self.button_images.lock().unwrap();
            map.clear();
        }
        self.trigger_flush();
        Ok(())
    }
}
