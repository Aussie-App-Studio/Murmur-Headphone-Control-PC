use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::{broadcast, oneshot, Mutex as AsyncMutex};

use crate::models::{
    BatteryUnit, ChargingStatus, EqualizerPreset, EqualizerState, GeneralSetting,
    HeadphoneCapabilities, HeadphoneState, MdrProtocolVersion, NoiseControlMode, NoiseControlState,
};
use crate::protocol::transport::{
    BluetoothTransport, HeadphoneSession, ProtocolError, GALAXY_BUDS_UUID,
};

pub const SOM: u8 = 0xFD;
pub const EOM: u8 = 0xDD;

pub const MSG_STATUS_UPDATED: u8 = 0x60;
pub const MSG_EXTENDED_STATUS_UPDATED: u8 = 0x61;
pub const MSG_SET_NOISE_CONTROLS: u8 = 0x79;
pub const MSG_EQUALIZER: u8 = 0x86;
pub const MSG_LOCK_TOUCHPAD: u8 = 0x90;
pub const MSG_SET_SEAMLESS_CONNECTION: u8 = 0xAF;

const LENGTH_MASK: usize = 0x3FF;
const RESPONSE_FLAG: u16 = 0x1000;
const MIN_INNER_LENGTH: usize = 3;
const MIN_FRAME_SIZE: usize = 7;

pub fn supported_eq_preset_codes() -> Vec<i32> {
    [
        EqualizerPreset::Off,
        EqualizerPreset::BassBoost,
        EqualizerPreset::Mellow,
        EqualizerPreset::Vocal,
        EqualizerPreset::Bright,
        EqualizerPreset::TrebleBoost,
    ]
    .into_iter()
    .map(|p| p.wire_byte() as i32)
    .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GalaxyBudsFrame {
    pub msg_id: u8,
    pub payload: Vec<u8>,
    pub is_response: bool,
}

pub fn crc16_xmodem(bytes: &[u8]) -> u16 {
    let mut crc: u16 = 0x0000;
    for &b in bytes {
        crc ^= (b as u16) << 8;
        for _ in 0..8 {
            if (crc & 0x8000) != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

pub fn encode_frame(frame: &GalaxyBudsFrame) -> Result<Vec<u8>, ProtocolError> {
    let inner_len = 1 + frame.payload.len() + 2;
    if !(MIN_INNER_LENGTH..=LENGTH_MASK).contains(&inner_len) {
        return Err(ProtocolError::Protocol(format!(
            "Payload too large for Galaxy Buds frame ({})",
            inner_len
        )));
    }
    let length_field =
        (inner_len as u16 & LENGTH_MASK as u16) | if frame.is_response { RESPONSE_FLAG } else { 0 };
    let mut out = Vec::with_capacity(4 + inner_len);
    out.push(SOM);
    out.push((length_field & 0xFF) as u8);
    out.push(((length_field >> 8) & 0xFF) as u8);
    out.push(frame.msg_id);
    out.extend_from_slice(&frame.payload);
    let crc = crc16_xmodem(&out[3..4 + frame.payload.len()]);
    out.push((crc & 0xFF) as u8);
    out.push(((crc >> 8) & 0xFF) as u8);
    out.push(EOM);
    Ok(out)
}

pub fn decode_frame(bytes: &[u8]) -> Option<GalaxyBudsFrame> {
    if bytes.len() < MIN_FRAME_SIZE || bytes[0] != SOM {
        return None;
    }
    let raw_len = (bytes[1] as u16) | ((bytes[2] as u16) << 8);
    let length = (raw_len as usize) & LENGTH_MASK;
    let is_response = (raw_len & RESPONSE_FLAG) != 0;
    if length < MIN_INNER_LENGTH || bytes.len() != 4 + length || bytes[3 + length] != EOM {
        return None;
    }
    let crc_offset = 1 + length;
    let actual_crc = (bytes[crc_offset] as u16) | ((bytes[crc_offset + 1] as u16) << 8);
    let expected_crc = crc16_xmodem(&bytes[3..3 + length - 2]);
    if actual_crc != expected_crc {
        return None;
    }
    Some(GalaxyBudsFrame {
        msg_id: bytes[3],
        payload: bytes[4..crc_offset].to_vec(),
        is_response,
    })
}

pub fn set_noise_control(state: &NoiseControlState) -> Vec<u8> {
    let mode_byte = match state.mode {
        NoiseControlMode::Off => 0x00,
        NoiseControlMode::NoiseCancelling => 0x01,
        NoiseControlMode::AmbientSound => 0x02,
    };
    encode_frame(&GalaxyBudsFrame {
        msg_id: MSG_SET_NOISE_CONTROLS,
        payload: vec![mode_byte],
        is_response: false,
    })
    .unwrap()
}

pub fn set_equalizer(preset: EqualizerPreset) -> Vec<u8> {
    encode_frame(&GalaxyBudsFrame {
        msg_id: MSG_EQUALIZER,
        payload: vec![equalizer_preset_to_index(preset)],
        is_response: false,
    })
    .unwrap()
}

pub fn set_touchpad_lock(locked: bool) -> Vec<u8> {
    encode_frame(&GalaxyBudsFrame {
        msg_id: MSG_LOCK_TOUCHPAD,
        payload: vec![if locked { 0x01 } else { 0x00 }],
        is_response: false,
    })
    .unwrap()
}

pub fn set_seamless_connection(enabled: bool) -> Vec<u8> {
    encode_frame(&GalaxyBudsFrame {
        msg_id: MSG_SET_SEAMLESS_CONNECTION,
        payload: vec![if enabled { 0x01 } else { 0x00 }],
        is_response: false,
    })
    .unwrap()
}

pub fn equalizer_preset_to_index(preset: EqualizerPreset) -> u8 {
    match preset {
        EqualizerPreset::Off => 0x00,
        EqualizerPreset::BassBoost => 0x01,
        EqualizerPreset::Mellow | EqualizerPreset::Relaxed => 0x02,
        EqualizerPreset::Vocal | EqualizerPreset::Excited => 0x03,
        EqualizerPreset::Bright => 0x04,
        EqualizerPreset::TrebleBoost => 0x05,
        _ => 0x00,
    }
}

pub fn index_to_equalizer_preset(idx: u8) -> EqualizerPreset {
    match idx {
        0x01 => EqualizerPreset::BassBoost,
        0x02 => EqualizerPreset::Mellow,
        0x03 => EqualizerPreset::Vocal,
        0x04 => EqualizerPreset::Bright,
        0x05 => EqualizerPreset::TrebleBoost,
        _ => EqualizerPreset::Off,
    }
}

pub fn upsert_general_setting(
    mut settings: Vec<GeneralSetting>,
    id: i32,
    key: &str,
    is_on: bool,
) -> Vec<GeneralSetting> {
    let entry = GeneralSetting {
        id,
        key: key.to_string(),
        summary_key: String::new(),
        is_on,
    };
    if let Some(existing) = settings.iter_mut().find(|g| g.key == key) {
        *existing = entry;
    } else {
        settings.push(entry);
    }
    settings
}

pub fn apply_frame(state: &HeadphoneState, frame: &GalaxyBudsFrame) -> HeadphoneState {
    match frame.msg_id {
        MSG_STATUS_UPDATED => apply_status_updated(state, &frame.payload),
        MSG_EXTENDED_STATUS_UPDATED => apply_extended_status_updated(state, &frame.payload),
        _ => state.clone(),
    }
}

fn parse_unit(b: u8) -> Option<BatteryUnit> {
    if b <= 100 {
        Some(BatteryUnit::new(b as i32, ChargingStatus::NotCharging))
    } else {
        None
    }
}

fn apply_status_updated(state: &HeadphoneState, payload: &[u8]) -> HeadphoneState {
    if payload.len() < 2 {
        return state.clone();
    }
    let mut next = state.clone();
    next.battery.left = parse_unit(payload[1]);
    if payload.len() > 2 {
        next.battery.right = parse_unit(payload[2]);
    }
    if payload.len() > 7 {
        next.battery.case_battery = parse_unit(payload[7]);
    }
    next.battery.level = next.battery.effective_level();
    next
}

fn apply_extended_status_updated(state: &HeadphoneState, payload: &[u8]) -> HeadphoneState {
    let mut next = state.clone();
    if payload.len() > 2 {
        next.battery.left = parse_unit(payload[2]);
    }
    if payload.len() > 3 {
        next.battery.right = parse_unit(payload[3]);
    }
    if payload.len() > 7 {
        next.battery.case_battery = parse_unit(payload[7]);
    }
    next.battery.level = next.battery.effective_level();

    if payload.len() > 8 {
        let preset = index_to_equalizer_preset(payload[8]);
        next.equalizer.preset_code = preset.wire_byte() as i32;
        next.equalizer.available_preset_codes = supported_eq_preset_codes();
    }
    if payload.len() > 9 && payload[9] > 0 {
        next.noise_control.ambient_level = (payload[9] as i32).clamp(1, 20);
    }
    let nc_byte = payload
        .get(10)
        .copied()
        .filter(|&m| m <= 2)
        .or_else(|| payload.get(11).copied().filter(|&m| m <= 2));
    if let Some(m) = nc_byte {
        next.noise_control.mode = match m {
            0x00 => NoiseControlMode::Off,
            0x01 => NoiseControlMode::NoiseCancelling,
            0x02 => NoiseControlMode::AmbientSound,
            _ => next.noise_control.mode,
        };
    }
    if payload.len() > 11 {
        let locked = payload[11] != 0;
        next.general_settings =
            upsert_general_setting(next.general_settings, 0xD1, "TOUCH_PANEL_SETTING", !locked);
    }
    if payload.len() > 12 {
        let seamless = payload[12] != 0;
        next.general_settings =
            upsert_general_setting(next.general_settings, 0xD2, "MULTIPOINT_SETTING", seamless);
    }
    next
}

pub struct GalaxyBudsStreamDecoder {
    buffer: Mutex<Vec<u8>>,
}

impl Default for GalaxyBudsStreamDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl GalaxyBudsStreamDecoder {
    pub fn new() -> Self {
        Self {
            buffer: Mutex::new(Vec::with_capacity(512)),
        }
    }

    pub fn feed(&self, chunk: &[u8]) -> Vec<GalaxyBudsFrame> {
        let mut buf = self.buffer.lock().unwrap();
        buf.extend_from_slice(chunk);
        let mut frames = Vec::new();
        loop {
            let Some(start) = buf.iter().position(|&b| b == SOM) else {
                buf.clear();
                break;
            };
            if start > 0 {
                buf.drain(0..start);
            }
            if buf.len() < 3 {
                break;
            }
            let raw_len = (buf[1] as usize) | ((buf[2] as usize) << 8);
            let frame_len = raw_len & LENGTH_MASK;
            if frame_len < MIN_INNER_LENGTH {
                buf.remove(0);
                continue;
            }
            let total_size = 4 + frame_len;
            if buf.len() < total_size {
                break;
            }
            if buf[3 + frame_len] != EOM {
                buf.remove(0);
                continue;
            }
            if let Some(decoded) = decode_frame(&buf[..total_size]) {
                frames.push(decoded);
                buf.drain(0..total_size);
            } else {
                buf.remove(0);
            }
        }
        frames
    }

    pub fn clear(&self) {
        self.buffer.lock().unwrap().clear();
    }
}

pub struct GalaxyBudsSession {
    transport: Arc<dyn BluetoothTransport>,
    service_uuid: String,
    write_mutex: AsyncMutex<()>,
    state: Mutex<HeadphoneState>,
    initial_sync_tx: Mutex<Option<oneshot::Sender<()>>>,
    reader_active: Arc<AtomicBool>,
    reader_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    decoder: GalaxyBudsStreamDecoder,
    state_tx: broadcast::Sender<HeadphoneState>,
    disconnect_tx: broadcast::Sender<()>,
}

impl GalaxyBudsSession {
    pub fn new(transport: Arc<dyn BluetoothTransport>, service_uuid: Option<String>) -> Arc<Self> {
        let (state_tx, _) = broadcast::channel(32);
        let (disconnect_tx, _) = broadcast::channel(8);
        Arc::new(Self {
            transport,
            service_uuid: service_uuid.unwrap_or_else(|| GALAXY_BUDS_UUID.to_string()),
            write_mutex: AsyncMutex::new(()),
            state: Mutex::new(HeadphoneState::default()),
            initial_sync_tx: Mutex::new(None),
            reader_active: Arc::new(AtomicBool::new(false)),
            reader_handle: Mutex::new(None),
            decoder: GalaxyBudsStreamDecoder::new(),
            state_tx,
            disconnect_tx,
        })
    }

    fn start_reader(self: &Arc<Self>) {
        self.reader_active.store(true, Ordering::SeqCst);
        let session = Arc::clone(self);
        let handle = tokio::spawn(async move {
            loop {
                if !session.reader_active.load(Ordering::SeqCst) || !session.transport.is_connected()
                {
                    break;
                }
                let transport = Arc::clone(&session.transport);
                let read_res = tokio::task::spawn_blocking(move || {
                    let mut local = vec![0u8; 1024];
                    let n = transport.read(&mut local)?;
                    local.truncate(n);
                    Ok::<Vec<u8>, ProtocolError>(local)
                })
                .await;

                match read_res {
                    Ok(Ok(chunk)) if !chunk.is_empty() => {
                        for frame in session.decoder.feed(&chunk) {
                            session.handle_frame(frame);
                        }
                    }
                    _ => break,
                }
            }
            session.reader_active.store(false, Ordering::SeqCst);
            if let Some(tx) = session.initial_sync_tx.lock().unwrap().take() {
                let _ = tx.send(());
            }
            let _ = session.disconnect_tx.send(());
        });
        *self.reader_handle.lock().unwrap() = Some(handle);
    }

    fn handle_frame(&self, frame: GalaxyBudsFrame) {
        let emitted = {
            let mut st = self.state.lock().unwrap();
            let updated = apply_frame(&st, &frame);
            if updated != *st {
                *st = updated.clone();
                Some(updated)
            } else {
                None
            }
        };
        if let Some(next) = emitted {
            let _ = self.state_tx.send(next);
        }
        if frame.msg_id == MSG_EXTENDED_STATUS_UPDATED {
            if let Some(tx) = self.initial_sync_tx.lock().unwrap().take() {
                let _ = tx.send(());
            }
        }
    }

    async fn write_bytes(&self, bytes: Vec<u8>) -> Result<(), ProtocolError> {
        if !self.transport.is_connected() {
            return Err(ProtocolError::NotConnected);
        }
        let _wg = self.write_mutex.lock().await;
        let transport = Arc::clone(&self.transport);
        tokio::task::spawn_blocking(move || transport.write(&bytes))
            .await
            .map_err(|e| ProtocolError::Io(e.to_string()))?
    }
}

#[async_trait::async_trait]
impl HeadphoneSession for Arc<GalaxyBudsSession> {
    fn protocol_version(&self) -> MdrProtocolVersion {
        MdrProtocolVersion::GalaxyBuds
    }

    fn is_connected(&self) -> bool {
        self.transport.is_connected() && self.reader_active.load(Ordering::SeqCst)
    }

    fn subscribe_state(&self) -> broadcast::Receiver<HeadphoneState> {
        self.state_tx.subscribe()
    }

    fn subscribe_disconnect(&self) -> broadcast::Receiver<()> {
        self.disconnect_tx.subscribe()
    }

    async fn connect_and_sync(
        &self,
        address: &str,
        device_name: &str,
    ) -> Result<HeadphoneState, ProtocolError> {
        self.close();
        let (tx, rx) = oneshot::channel();
        {
            let mut st = self.state.lock().unwrap();
            *st = HeadphoneState {
                model_name: if device_name.trim().is_empty() {
                    "Galaxy Buds".to_string()
                } else {
                    device_name.trim().to_string()
                },
                firmware_version: "Galaxy Buds".to_string(),
                bluetooth_address: address.to_string(),
                protocol_version: MdrProtocolVersion::GalaxyBuds,
                equalizer: EqualizerState {
                    preset_code: EqualizerPreset::Off.wire_byte() as i32,
                    available_preset_codes: supported_eq_preset_codes(),
                    band_count: 5,
                    step_count: 21,
                    has_clear_bass: false,
                    ..Default::default()
                },
                general_settings: vec![
                    GeneralSetting {
                        id: 0xD1,
                        key: "TOUCH_PANEL_SETTING".to_string(),
                        summary_key: String::new(),
                        is_on: true,
                    },
                    GeneralSetting {
                        id: 0xD2,
                        key: "MULTIPOINT_SETTING".to_string(),
                        summary_key: String::new(),
                        is_on: true,
                    },
                ],
                custom_capabilities: Some(HeadphoneCapabilities::galaxy_buds()),
                ..Default::default()
            };
            *self.initial_sync_tx.lock().unwrap() = Some(tx);
        }

        let transport = Arc::clone(&self.transport);
        let addr = address.to_string();
        let uuid = self.service_uuid.clone();
        tokio::task::spawn_blocking(move || transport.connect(&addr, &uuid))
            .await
            .map_err(|e| ProtocolError::Io(e.to_string()))??;

        self.start_reader();
        let _ = tokio::time::timeout(Duration::from_millis(1500), rx).await;
        *self.initial_sync_tx.lock().unwrap() = None;
        Ok(self.state.lock().unwrap().clone())
    }

    async fn poll_battery(&self) -> Result<(), ProtocolError> {
        Ok(())
    }

    async fn set_noise_control(&self, state: &NoiseControlState) -> Result<(), ProtocolError> {
        self.write_bytes(set_noise_control(state)).await?;
        let next = {
            let mut st = self.state.lock().unwrap();
            st.noise_control = state.clone();
            st.clone()
        };
        let _ = self.state_tx.send(next);
        Ok(())
    }

    async fn set_eq_preset(&self, preset: EqualizerPreset) -> Result<(), ProtocolError> {
        self.write_bytes(set_equalizer(preset)).await?;
        let mapped = index_to_equalizer_preset(equalizer_preset_to_index(preset));
        let next = {
            let mut st = self.state.lock().unwrap();
            st.equalizer.preset_code = mapped.wire_byte() as i32;
            st.equalizer.available_preset_codes = supported_eq_preset_codes();
            st.clone()
        };
        let _ = self.state_tx.send(next);
        Ok(())
    }

    async fn set_touch_panel_enabled(&self, enabled: bool) -> Result<(), ProtocolError> {
        self.write_bytes(set_touchpad_lock(!enabled)).await?;
        let next = {
            let mut st = self.state.lock().unwrap();
            st.general_settings = upsert_general_setting(
                st.general_settings.clone(),
                0xD1,
                "TOUCH_PANEL_SETTING",
                enabled,
            );
            st.clone()
        };
        let _ = self.state_tx.send(next);
        Ok(())
    }

    async fn set_multipoint_enabled(&self, enabled: bool) -> Result<(), ProtocolError> {
        self.write_bytes(set_seamless_connection(enabled)).await?;
        let next = {
            let mut st = self.state.lock().unwrap();
            st.general_settings = upsert_general_setting(
                st.general_settings.clone(),
                0xD2,
                "MULTIPOINT_SETTING",
                enabled,
            );
            st.clone()
        };
        let _ = self.state_tx.send(next);
        Ok(())
    }

    fn close(&self) {
        self.reader_active.store(false, Ordering::SeqCst);
        if let Some(h) = self.reader_handle.lock().unwrap().take() {
            h.abort();
        }
        self.transport.close();
        self.decoder.clear();
        let _ = self.initial_sync_tx.lock().unwrap().take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_and_frame_roundtrip() {
        let frame = GalaxyBudsFrame {
            msg_id: MSG_SET_NOISE_CONTROLS,
            payload: vec![0x01],
            is_response: false,
        };
        let encoded = encode_frame(&frame).unwrap();
        let decoded = decode_frame(&encoded).unwrap();
        assert_eq!(decoded, frame);
    }
}
