use std::sync::Mutex;

use crate::protocol::transport::ProtocolError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MdrDataType {
    Ack = 0x01,
    DataMdr = 0x0C,
    DataMdrNo2 = 0x0E,
}

impl MdrDataType {
    pub fn wire_byte(self) -> u8 {
        self as u8
    }

    pub fn from_byte(value: u8) -> Option<Self> {
        match value {
            0x01 => Some(Self::Ack),
            0x0C => Some(Self::DataMdr),
            0x0E => Some(Self::DataMdrNo2),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MdrFrame {
    pub frame_type: MdrDataType,
    pub sequence: u8,
    pub payload: Vec<u8>,
}

/// Hard-blocks any firmware update, log upload, or factory-reset opcodes at the
/// framing layer so they can never be transmitted to the headphones.
pub struct ProtocolFirewall;

impl ProtocolFirewall {
    pub fn verify_allowed(frame_type: MdrDataType, payload: &[u8]) -> Result<(), ProtocolError> {
        if frame_type == MdrDataType::Ack {
            return Ok(());
        }
        if payload.is_empty() {
            return Err(ProtocolError::Protocol("Blocked empty MDR payload".into()));
        }
        let opcode = payload[0];
        if frame_type == MdrDataType::DataMdr {
            if (0x30..=0x39).contains(&opcode) || opcode == 0xC4 {
                return Err(ProtocolError::Protocol(format!(
                    "ProtocolFirewall blocked prohibited T1 opcode 0x{:02X}",
                    opcode
                )));
            }
            // Block SYSTEM_SET_PARAM 0xF8 subtype 0x09 (settings/factory reset)
            if opcode == 0xF8 && payload.len() >= 2 && payload[1] == 0x09 {
                return Err(ProtocolError::Protocol(
                    "ProtocolFirewall blocked factory/settings reset command".into(),
                ));
            }
        }
        Ok(())
    }
}

pub const START_MARKER: u8 = 0x3E;
pub const END_MARKER: u8 = 0x3C;
pub const ESCAPE_MARKER: u8 = 0x3D;
pub const MAX_FRAME_SIZE: usize = 2048;
const MIN_BODY_SIZE: usize = 7; // type(1) + seq(1) + len(4) + checksum(1)

pub fn escape_bytes(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 4);
    for &b in data {
        match b {
            END_MARKER => {
                out.push(ESCAPE_MARKER);
                out.push(0x2C);
            }
            ESCAPE_MARKER => {
                out.push(ESCAPE_MARKER);
                out.push(0x2D);
            }
            START_MARKER => {
                out.push(ESCAPE_MARKER);
                out.push(0x2E);
            }
            _ => out.push(b),
        }
    }
    out
}

pub fn unescape_bytes(data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        if b == ESCAPE_MARKER {
            if i + 1 >= data.len() {
                return Err(ProtocolError::Protocol("Truncated escape sequence".into()));
            }
            i += 1;
            match data[i] {
                0x2C => out.push(END_MARKER),
                0x2D => out.push(ESCAPE_MARKER),
                0x2E => out.push(START_MARKER),
                other => {
                    return Err(ProtocolError::Protocol(format!(
                        "Unexpected escaped byte 0x{:02X}",
                        other
                    )))
                }
            }
        } else {
            out.push(b);
        }
        i += 1;
    }
    Ok(out)
}

pub fn checksum(unescaped_body: &[u8]) -> u8 {
    let mut acc: u8 = 0;
    for &b in unescaped_body {
        acc = acc.wrapping_add(b);
    }
    acc
}

pub fn encode_frame(frame: &MdrFrame) -> Result<Vec<u8>, ProtocolError> {
    ProtocolFirewall::verify_allowed(frame.frame_type, &frame.payload)?;

    let payload_len = frame.payload.len();
    let mut body = Vec::with_capacity(6 + payload_len + 1);
    body.push(frame.frame_type.wire_byte());
    body.push(frame.sequence & 0x01);
    body.push(((payload_len >> 24) & 0xFF) as u8);
    body.push(((payload_len >> 16) & 0xFF) as u8);
    body.push(((payload_len >> 8) & 0xFF) as u8);
    body.push((payload_len & 0xFF) as u8);
    body.extend_from_slice(&frame.payload);
    let csum = checksum(&body);
    body.push(csum);

    let escaped = escape_bytes(&body);
    if escaped.len() + 2 > MAX_FRAME_SIZE {
        return Err(ProtocolError::Protocol(format!(
            "Encoded frame exceeds MAX_FRAME_SIZE ({})",
            MAX_FRAME_SIZE
        )));
    }

    let mut result = Vec::with_capacity(escaped.len() + 2);
    result.push(START_MARKER);
    result.extend_from_slice(&escaped);
    result.push(END_MARKER);
    Ok(result)
}

pub fn decode_body(escaped_body: &[u8]) -> Result<MdrFrame, ProtocolError> {
    let unescaped = unescape_bytes(escaped_body)?;
    if unescaped.len() < MIN_BODY_SIZE {
        return Err(ProtocolError::Protocol(
            "Frame smaller than minimum size".into(),
        ));
    }

    let data_size = ((unescaped[2] as usize) << 24)
        | ((unescaped[3] as usize) << 16)
        | ((unescaped[4] as usize) << 8)
        | (unescaped[5] as usize);

    if unescaped.len() < 6 + data_size + 1 {
        return Err(ProtocolError::Protocol(format!(
            "Declared payload size ({}) exceeds frame bytes ({})",
            data_size,
            unescaped.len()
        )));
    }

    let expected_checksum = unescaped[6 + data_size];
    let actual_checksum = checksum(&unescaped[..6 + data_size]);
    if expected_checksum != actual_checksum {
        return Err(ProtocolError::Protocol("Checksum mismatch".into()));
    }

    let frame_type = MdrDataType::from_byte(unescaped[0]).ok_or_else(|| {
        ProtocolError::Protocol(format!("Unsupported data type 0x{:02X}", unescaped[0]))
    })?;
    let sequence = unescaped[1];
    let payload = unescaped[6..6 + data_size].to_vec();

    Ok(MdrFrame {
        frame_type,
        sequence,
        payload,
    })
}

pub fn decode_frame(frame_bytes: &[u8]) -> Result<MdrFrame, ProtocolError> {
    if frame_bytes.len() < 2
        || *frame_bytes.first().unwrap() != START_MARKER
        || *frame_bytes.last().unwrap() != END_MARKER
    {
        return Err(ProtocolError::Protocol("Invalid frame delimiters".into()));
    }
    decode_body(&frame_bytes[1..frame_bytes.len() - 1])
}

pub struct MdrStreamDecoder {
    buffer: Mutex<Vec<u8>>,
}

impl Default for MdrStreamDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl MdrStreamDecoder {
    pub fn new() -> Self {
        Self {
            buffer: Mutex::new(Vec::with_capacity(512)),
        }
    }

    pub fn feed(&self, chunk: &[u8]) -> Vec<MdrFrame> {
        let mut buf = self.buffer.lock().unwrap();
        buf.extend_from_slice(chunk);
        let mut frames = Vec::new();

        loop {
            let Some(start_idx) = buf.iter().position(|&b| b == START_MARKER) else {
                buf.clear();
                break;
            };
            if start_idx > 0 {
                buf.drain(0..start_idx);
            }

            let mut nested_start = None;
            let mut end_idx = None;
            for (i, &b) in buf.iter().enumerate().skip(1) {
                if b == START_MARKER {
                    nested_start = Some(i);
                    break;
                }
                if b == END_MARKER {
                    end_idx = Some(i);
                    break;
                }
            }

            if let Some(ns) = nested_start {
                buf.drain(0..ns);
                continue;
            }

            let Some(end) = end_idx else {
                if buf.len() > MAX_FRAME_SIZE * 2 {
                    buf.remove(0);
                }
                break;
            };

            let body = buf[1..end].to_vec();
            buf.drain(0..=end);
            if let Ok(frame) = decode_body(&body) {
                frames.push(frame);
            }
        }

        frames
    }

    pub fn clear(&self) {
        self.buffer.lock().unwrap().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn firewall_blocks_firmware_and_reset() {
        for op in 0x30..=0x39 {
            let err = ProtocolFirewall::verify_allowed(MdrDataType::DataMdr, &[op, 0x00]);
            assert!(err.is_err(), "Expected opcode 0x{:02X} blocked", op);
        }
        assert!(ProtocolFirewall::verify_allowed(MdrDataType::DataMdr, &[0xC4, 0x00]).is_err());
        assert!(ProtocolFirewall::verify_allowed(MdrDataType::DataMdr, &[0xF8, 0x09]).is_err());
        assert!(ProtocolFirewall::verify_allowed(MdrDataType::DataMdr, &[0x68, 0x17]).is_ok());
        assert!(ProtocolFirewall::verify_allowed(MdrDataType::Ack, &[]).is_ok());
    }

    #[test]
    fn roundtrip_escape_and_fragmented_stream() {
        let payload = vec![0x68, 0x3E, 0x3D, 0x3C, 0x01, 0xFF];
        let frame = MdrFrame {
            frame_type: MdrDataType::DataMdr,
            sequence: 1,
            payload: payload.clone(),
        };
        let encoded = encode_frame(&frame).unwrap();
        let decoded = decode_frame(&encoded).unwrap();
        assert_eq!(decoded, frame);

        let decoder = MdrStreamDecoder::new();
        let mid = encoded.len() / 2;
        assert!(decoder.feed(&encoded[..mid]).is_empty());
        let out = decoder.feed(&encoded[mid..]);
        assert_eq!(out, vec![frame]);
    }
}
