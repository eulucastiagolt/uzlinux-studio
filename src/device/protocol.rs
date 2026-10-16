#![allow(dead_code)]

pub const MAGIC: [u8; 2] = [0x7C, 0x7C]; // "||"
pub const HID_CHUNK_SIZE: usize = 1024;

pub const PROTOCOL_HOST_CMD: u8 = 0x00;
pub const PROTOCOL_DEVICE_EVENT: u8 = 0x01;

pub const CMD_KEY_EVENT: u8 = 0x01;
pub const CMD_HEARTBEAT: u8 = 0x03;
pub const CMD_UPDATE_INFO: u8 = 0x06;
pub const CMD_SET_BRIGHTNESS: u8 = 0x0A;
pub const CMD_SET_SLEEPTIME: u8 = 0x1A;
pub const CMD_SEND_IMAGE_SLOT: u8 = 0x0E;
pub const CMD_SEND_SCREEN_DATA: u8 = 0x0F;

#[derive(Debug, Clone)]
pub enum DeckEvent {
    KeyEvent {
        key: u8,
        col: u8,
        row: u8,
        pos: u8,
        is_encoder: bool,
        pressed: bool,
    },
    Heartbeat,
    Unknown {
        proto: u8,
        cmd: u8,
        len: u32,
    },
}

/// Builds a 1024-byte padded packet according to the reversed protocol:
/// [0..2]: "||" (0x7C, 0x7C)
/// [2]: protocol_id (0x00 for host command)
/// [3]: command_code (u8)
/// [4..8]: payload length (u32 LE)
/// [8..8+N]: payload data
/// [8+N..10+N]: 0x00, 0x00 trailer
/// zero-padded to the next 1024-byte boundary.
pub fn build_packet(proto: u8, cmd: u8, payload: &[u8]) -> Vec<u8> {
    let payload_len = payload.len();
    let mut packet = Vec::with_capacity(payload_len + 10 + HID_CHUNK_SIZE);

    // Header
    packet.extend_from_slice(&MAGIC);
    packet.push(proto);
    packet.push(cmd);
    packet.extend_from_slice(&(payload_len as u32).to_le_bytes());

    // Payload
    packet.extend_from_slice(payload);

    // Trailer
    packet.push(0x00);
    packet.push(0x00);

    // Pad to multiple of 1024
    let rem = packet.len() % HID_CHUNK_SIZE;
    if rem != 0 {
        let pad_len = HID_CHUNK_SIZE - rem;
        packet.resize(packet.len() + pad_len, 0);
    }

    packet
}

/// Parses an incoming 1024-byte HID buffer into a DeckEvent
pub fn parse_packet(buf: &[u8]) -> Option<DeckEvent> {
    if buf.len() < 8 || buf[0] != MAGIC[0] || buf[1] != MAGIC[1] {
        return None;
    }

    let proto = buf[2];
    let cmd = buf[3];
    let len = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);

    if proto == PROTOCOL_DEVICE_EVENT && cmd == CMD_KEY_EVENT {
        if buf.len() >= 12 {
            let key = buf[8];
            let pos = buf[9];
            let event_type = buf[10];
            let status = buf[11];

            let col = pos % 5;
            let row = pos / 5;
            let is_encoder = event_type == 1 || event_type == 2;
            let pressed = status == 1;

            return Some(DeckEvent::KeyEvent {
                key,
                col,
                row,
                pos,
                is_encoder,
                pressed,
            });
        }
    } else if cmd == CMD_HEARTBEAT {
        return Some(DeckEvent::Heartbeat);
    }

    Some(DeckEvent::Unknown { proto, cmd, len })
}
