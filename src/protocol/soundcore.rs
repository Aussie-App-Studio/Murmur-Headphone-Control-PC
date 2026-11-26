use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::{broadcast, oneshot, Mutex as AsyncMutex};

use crate::models::{
    ChargingStatus, EqualizerPreset, EqualizerState, HeadphoneCapabilities, HeadphoneState,
    MdrProtocolVersion, NoiseControlMode, NoiseControlState,
};
use crate::protocol::transport::{
    BluetoothTransport, HeadphoneSession, ProtocolError, SOUNDCORE_DEFAULT_UUID,
};

pub const OUTBOUND_HEADER: [u8; 5] = [0x08, 0xEE, 0x00, 0x00, 0x00];
pub const INBOUND_HEADER: [u8; 5] = [0x09, 0xFF, 0x00, 0x00, 0x01];

pub const GROUP_INFO: u8 = 0x01;
pub const GROUP_EQ: u8 = 0x02;
pub const GROUP_ANC: u8 = 0x06;

pub const CMD_GET_STATE: u8 = 0x01;
pub const CMD_GET_BATTERY: u8 = 0x03;
pub const CMD_BATTERY_NOTIFY: u8 = 0x04;
pub const CMD_EQ_STATE: u8 = 0x01;
pub const CMD_SET_EQ: u8 = 0x81;
pub const CMD_ANC_STATE: u8 = 0x01;
pub const CMD_SET_SOUND_MODE: u8 = 0x81;

pub const MIN_FRAME_SIZE: usize = 10;
pub const MAX_FRAME_SIZE: usize = 2048;
pub const STATE_OFFSET_BATTERY_LEVEL: usize = 0;
pub const STATE_OFFSET_BATTERY_CHARGING: usize = 1;
pub const STATE_OFFSET_EQ_BANDS: usize = 16;
pub const STATE_OFFSET_SOUND_MODE: usize = 37;
pub const STATE_PAYLOAD_MIN_OVER_EAR_SIZE: usize = 45;

const EQ_BAND_COUNT: usize = 8;
const EQ_STEP_COUNT: i32 = 13;
const EQ_MIN_DB: i32 = -6;
const EQ_MAX_DB: i32 = 6;
const EQ_ZERO_WIRE: i32 = 120;
const EQ_UNITS_PER_DB: i32 = 10;

pub fn supported_preset_codes() -> Vec<i32> {
    [
        EqualizerPreset::Off,
        EqualizerPreset::BassBoost,
        EqualizerPreset::TrebleBoost,
        EqualizerPreset::Vocal,
        EqualizerPreset::Bright,
        EqualizerPreset::Excited,
        EqualizerPreset::Mellow,
        EqualizerPreset::Relaxed,
        EqualizerPreset::Speech,
        EqualizerPreset::Manual,
    ]
    .into_iter()
    .map(|p| p.wire_byte() as i32)
    .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoundcorePacket {
    pub group: u8,
    pub command: u8,
    pub payload: Vec<u8>,
    pub is_inbound: bool,
}

pub fn checksum(bytes: &[u8]) -> u8 {
    let mut sum: u8 = 0;
    for &b in bytes {
        sum = sum.wrapping_add(b);
    }
    sum
}

pub fn encode_packet(packet: &SoundcorePacket) -> Vec<u8> {
    let header = if packet.is_inbound {
        &INBOUND_HEADER
    } else {
        &OUTBOUND_HEADER
    };
    let total_len = MIN_FRAME_SIZE + packet.payload.len();
    let mut out = Vec::with_capacity(total_len);
    out.extend_from_slice(header);
    out.push(packet.group);
    out.push(packet.command);
    out.push((total_len & 0xFF) as u8);
    out.push(((total_len >> 8) & 0xFF) as u8);
    out.extend_from_slice(&packet.payload);
    let csum = checksum(&out);
    out.push(csum);
    out
}

pub fn decode_packet(bytes: &[u8]) -> Option<SoundcorePacket> {
    if bytes.len() < MIN_FRAME_SIZE {
        return None;
    }
    let is_inbound = if bytes.starts_with(&INBOUND_HEADER) {
        true
    } else if bytes.starts_with(&OUTBOUND_HEADER) {
        false
    } else {
        return None;
    };
    let total_len = (bytes[7] as usize) | ((bytes[8] as usize) << 8);
    if total_len < MIN_FRAME_SIZE || bytes.len() != total_len {
        return None;
    }
    if bytes[total_len - 1] != checksum(&bytes[..total_len - 1]) {
        return None;
    }
    Some(SoundcorePacket {
        group: bytes[5],
        command: bytes[6],
        payload: bytes[9..total_len - 1].to_vec(),
        is_inbound,
    })
}

pub fn get_state() -> Vec<u8> {
    encode_packet(&SoundcorePacket {
        group: GROUP_INFO,
        command: CMD_GET_STATE,
        payload: Vec::new(),
        is_inbound: false,
    })
}

pub fn get_battery() -> Vec<u8> {
    encode_packet(&SoundcorePacket {
        group: GROUP_INFO,
        command: CMD_GET_BATTERY,
        payload: Vec::new(),
        is_inbound: false,
    })
}

pub fn set_sound_mode(state: &NoiseControlState) -> Vec<u8> {
    let payload = match state.mode {
        NoiseControlMode::NoiseCancelling => vec![0x00, 0x00, 0x00, 0x00],
        NoiseControlMode::AmbientSound => vec![0x01, 0x00, 0x01, 0x00],
        NoiseControlMode::Off => vec![0x02, 0x00, 0x00, 0x00],
    };
    encode_packet(&SoundcorePacket {
        group: GROUP_ANC,
        command: CMD_SET_SOUND_MODE,
        payload,
        is_inbound: false,
    })
}

pub fn set_custom_eq(bands: &[i32]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(2 + EQ_BAND_COUNT);
    payload.push(0xFE);
    payload.push(0xFE);
    for i in 0..EQ_BAND_COUNT {
        let db = bands.get(i).copied().unwrap_or(0).clamp(EQ_MIN_DB, EQ_MAX_DB);
        payload.push((db * EQ_UNITS_PER_DB + EQ_ZERO_WIRE).clamp(60, 180) as u8);
    }
    encode_packet(&SoundcorePacket {
        group: GROUP_EQ,
        command: CMD_SET_EQ,
        payload,
        is_inbound: false,
    })
}

pub fn preset_bands(preset: EqualizerPreset) -> Vec<i32> {
    match preset {
        EqualizerPreset::Off => vec![0, 0, 0, 0, 0, 0, 0, 0],
        EqualizerPreset::Bright => vec![-1, 0, 0, 1, 2, 3, 4, 4],
        EqualizerPreset::Excited => vec![3, 2, 0, -1, 0, 2, 3, 4],
        EqualizerPreset::Mellow => vec![2, 1, 0, 0, -1, -1, -2, -2],
        EqualizerPreset::Relaxed => vec![1, 1, 0, 0, 0, -1, -1, -2],
        EqualizerPreset::Vocal => vec![-2, -1, 1, 3, 3, 2, 0, -1],
        EqualizerPreset::TrebleBoost => vec![-1, -1, 0, 0, 1, 2, 4, 5],
        EqualizerPreset::BassBoost => vec![5, 4, 3, 1, 0, 0, 0, 0],
        EqualizerPreset::Speech => vec![-3, -2, 0, 2, 3, 2, 0, -2],
        _ => vec![0, 0, 0, 0, 0, 0, 0, 0],
    }
}

pub fn set_preset_eq(preset: EqualizerPreset) -> Vec<u8> {
    set_custom_eq(&preset_bands(preset))
}

pub fn decode_sound_mode_byte(b: u8) -> Option<NoiseControlMode> {
    match b {
        0x00 => Some(NoiseControlMode::NoiseCancelling),
        0x01 => Some(NoiseControlMode::AmbientSound),
        0x02 => Some(NoiseControlMode::Off),
        _ => None,
    }
}

pub fn encode_sound_mode_byte(mode: NoiseControlMode) -> u8 {
    match mode {
        NoiseControlMode::NoiseCancelling => 0x00,
        NoiseControlMode::AmbientSound => 0x01,
        NoiseControlMode::Off => 0x02,
    }
}

pub fn apply_packet(state: &HeadphoneState, packet: &SoundcorePacket) -> HeadphoneState {
    let p = &packet.payload;
    let mut next = state.clone();
    match (packet.group, packet.command) {
        (GROUP_INFO, CMD_GET_STATE) => {
            if !p.is_empty() {
                next = apply_battery_update(&next, p);
            }
            if p.len() >= STATE_OFFSET_EQ_BANDS + EQ_BAND_COUNT {
                let bands = parse_eq_bands(p, STATE_OFFSET_EQ_BANDS);
                next.equalizer = updated_equalizer(&next.equalizer, bands);
            }
            if p.len() > STATE_OFFSET_SOUND_MODE {
                if let Some(mode) = decode_sound_mode_byte(p[STATE_OFFSET_SOUND_MODE]) {
                    next.noise_control.mode = mode;
                }
            }
        }
        (GROUP_INFO, CMD_GET_BATTERY | CMD_BATTERY_NOTIFY) => {
            next = apply_battery_update(&next, p);
        }
        (GROUP_ANC, CMD_ANC_STATE | CMD_SET_SOUND_MODE) => {
            if let Some(&b) = p.first() {
                if let Some(mode) = decode_sound_mode_byte(b) {
                    next.noise_control.mode = mode;
                }
            }
        }
        (GROUP_EQ, CMD_EQ_STATE | CMD_SET_EQ) => {
            let offset = if p.len() >= 2 + EQ_BAND_COUNT {
                2
            } else if p.len() >= EQ_BAND_COUNT {
                0
            } else {
                return next;
            };
            let bands = parse_eq_bands(p, offset);
            next.equalizer = updated_equalizer(&next.equalizer, bands);
        }
        _ => {}
    }
    next
}

fn apply_battery_update(state: &HeadphoneState, payload: &[u8]) -> HeadphoneState {
    if payload.is_empty() {
        return state.clone();
    }
    let raw = payload[STATE_OFFSET_BATTERY_LEVEL] as i32;
    let level = if (0..=5).contains(&raw) {
        raw * 20
    } else {
        raw.clamp(0, 100)
    };
    let chg = if payload.len() >= 2 && payload[STATE_OFFSET_BATTERY_CHARGING] != 0 {
        ChargingStatus::Charging
    } else {
        ChargingStatus::NotCharging
    };
    let mut next = state.clone();
    next.battery.level = Some(level);
    next.battery.charging_status = chg;
    next
}

fn parse_eq_bands(payload: &[u8], offset: usize) -> Vec<i32> {
    (0..EQ_BAND_COUNT)
        .map(|i| {
            let raw = payload[offset + i] as i32;
            ((raw - EQ_ZERO_WIRE) / EQ_UNITS_PER_DB).clamp(EQ_MIN_DB, EQ_MAX_DB)
        })
        .collect()
}

fn updated_equalizer(current: &EqualizerState, bands: Vec<i32>) -> EqualizerState {
    let resolved = if let Some(cp) = current.preset() {
        if preset_bands(cp) == bands {
            cp
        } else if bands.iter().all(|&b| b == 0) {
            EqualizerPreset::Off
        } else {
            EqualizerPreset::ALL
                .into_iter()
                .find(|p| !p.is_customizable() && preset_bands(*p) == bands)
                .unwrap_or(EqualizerPreset::Manual)
        }
    } else if bands.iter().all(|&b| b == 0) {
        EqualizerPreset::Off
    } else {
        EqualizerPreset::ALL
            .into_iter()
            .find(|p| !p.is_customizable() && preset_bands(*p) == bands)
            .unwrap_or(EqualizerPreset::Manual)
    };
    EqualizerState {
        preset_code: resolved.wire_byte() as i32,
        clear_bass: 0,
        bands,
        available_preset_codes: if current.available_preset_codes.is_empty() {
            supported_preset_codes()
        } else {
            current.available_preset_codes.clone()
        },
        band_count: EQ_BAND_COUNT as i32,
        step_count: EQ_STEP_COUNT,
        has_clear_bass: false,
    }
}

pub struct SoundcoreStreamDecoder {
    buffer: Mutex<Vec<u8>>,
}

impl Default for SoundcoreStreamDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl SoundcoreStreamDecoder {
    pub fn new() -> Self {
        Self {
            buffer: Mutex::new(Vec::with_capacity(512)),
        }
    }

    pub fn feed(&self, chunk: &[u8]) -> Vec<SoundcorePacket> {
        let mut buf = self.buffer.lock().unwrap();
        buf.extend_from_slice(chunk);
        let mut packets = Vec::new();

        loop {
            let hdr_idx = buf.windows(5).position(|w| w == INBOUND_HEADER || w == OUTBOUND_HEADER);
            let Some(idx) = hdr_idx else {
                let keep = buf.len().min(4);
                let drop_count = buf.len() - keep;
                if drop_count > 0 {
                    buf.drain(0..drop_count);
                }
                break;
            };
            if idx > 0 {
                buf.drain(0..idx);
            }
            if buf.len() < MIN_FRAME_SIZE {
                break;
            }
            let total_len = (buf[7] as usize) | ((buf[8] as usize) << 8);
            if !(MIN_FRAME_SIZE..=MAX_FRAME_SIZE).contains(&total_len) {
                buf.remove(0);
                continue;
            }
            if buf.len() < total_len {
                break;
            }
            if let Some(pkt) = decode_packet(&buf[..total_len]) {
                packets.push(pkt);
                buf.drain(0..total_len);
            } else {
                buf.remove(0);
            }
        }
        packets
    }

    pub fn clear(&self) {
        self.buffer.lock().unwrap().clear();
    }
}

pub struct SoundcoreSession {
    transport: Arc<dyn BluetoothTransport>,
    service_uuid: String,
    write_mutex: AsyncMutex<()>,
    state: Mutex<HeadphoneState>,
    initial_sync_tx: Mutex<Option<oneshot::Sender<()>>>,
    reader_active: Arc<AtomicBool>,
    reader_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    decoder: SoundcoreStreamDecoder,
    state_tx: broadcast::Sender<HeadphoneState>,
    disconnect_tx: broadcast::Sender<()>,
}

impl SoundcoreSession {
    pub fn new(transport: Arc<dyn BluetoothTransport>, service_uuid: Option<String>) -> Arc<Self> {
        let (state_tx, _) = broadcast::channel(16);
        let (disconnect_tx, _) = broadcast::channel(8);
        Arc::new(Self {
            transport,
            service_uuid: service_uuid.unwrap_or_else(|| SOUNDCORE_DEFAULT_UUID.to_string()),
            write_mutex: AsyncMutex::new(()),
            state: Mutex::new(default_soundcore_state("", "Soundcore Headphones")),
            initial_sync_tx: Mutex::new(None),
            reader_active: Arc::new(AtomicBool::new(false)),
            reader_handle: Mutex::new(None),
            decoder: SoundcoreStreamDecoder::new(),
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
                        for pkt in session.decoder.feed(&chunk) {
                            session.handle_packet(pkt);
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

    fn handle_packet(&self, packet: SoundcorePacket) {
        let mut sync_completed_now = false;
        let updated = {
            let mut st = self.state.lock().unwrap();
            let applied = enforce_defaults(apply_packet(&st, &packet));
            let changed = applied != *st;
            *st = applied.clone();
            if packet.group == GROUP_INFO && packet.command == CMD_GET_STATE {
                if let Some(tx) = self.initial_sync_tx.lock().unwrap().take() {
                    let _ = tx.send(());
                    sync_completed_now = true;
                }
            }
            if !sync_completed_now && changed {
                Some(applied)
            } else {
                None
            }
        };
        if let Some(next) = updated {
            let _ = self.state_tx.send(next);
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

fn default_soundcore_state(address: &str, device_name: &str) -> HeadphoneState {
    HeadphoneState {
        model_name: device_name.to_string(),
        firmware_version: "Soundcore SPP".to_string(),
        bluetooth_address: address.to_string(),
        protocol_version: MdrProtocolVersion::Soundcore,
        equalizer: EqualizerState {
            preset_code: EqualizerPreset::Off.wire_byte() as i32,
            clear_bass: 0,
            bands: vec![0; EQ_BAND_COUNT],
            available_preset_codes: supported_preset_codes(),
            band_count: EQ_BAND_COUNT as i32,
            step_count: EQ_STEP_COUNT,
            has_clear_bass: false,
        },
        custom_capabilities: Some(HeadphoneCapabilities::soundcore(false)),
        ..Default::default()
    }
}

fn enforce_defaults(mut state: HeadphoneState) -> HeadphoneState {
    let dual = state.battery.is_dual_battery();
    state.protocol_version = MdrProtocolVersion::Soundcore;
    state.custom_capabilities = Some(HeadphoneCapabilities::soundcore(dual));
    if state.equalizer.available_preset_codes.is_empty() {
        state.equalizer.available_preset_codes = supported_preset_codes();
    }
    state.equalizer.band_count = EQ_BAND_COUNT as i32;
    state.equalizer.step_count = EQ_STEP_COUNT;
    state.equalizer.has_clear_bass = false;
    state
}

#[async_trait::async_trait]
impl HeadphoneSession for Arc<SoundcoreSession> {
    fn protocol_version(&self) -> MdrProtocolVersion {
        MdrProtocolVersion::Soundcore
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
        let transport = Arc::clone(&self.transport);
        let addr = address.to_string();
        let uuid = self.service_uuid.clone();
        tokio::task::spawn_blocking(move || transport.connect(&addr, &uuid))
            .await
            .map_err(|e| ProtocolError::Io(e.to_string()))??;

        let (tx, rx) = oneshot::channel();
        {
            let mut st = self.state.lock().unwrap();
            *st = default_soundcore_state(
                address,
                if device_name.trim().is_empty() {
                    "Soundcore Headphones"
                } else {
                    device_name.trim()
                },
            );
            *self.initial_sync_tx.lock().unwrap() = Some(tx);
        }

        self.start_reader();
        self.write_bytes(get_state()).await?;
        let _ = tokio::time::timeout(Duration::from_millis(1500), rx).await;
        *self.initial_sync_tx.lock().unwrap() = None;

        let mut st = self.state.lock().unwrap();
        *st = enforce_defaults(st.clone());
        Ok(st.clone())
    }

    async fn poll_battery(&self) -> Result<(), ProtocolError> {
        if !self.is_connected() {
            return Ok(());
        }
        self.write_bytes(get_battery()).await
    }

    async fn set_noise_control(&self, state: &NoiseControlState) -> Result<(), ProtocolError> {
        self.write_bytes(set_sound_mode(state)).await?;
        let next = {
            let mut st = self.state.lock().unwrap();
            st.noise_control.mode = state.mode;
            *st = enforce_defaults(st.clone());
            st.clone()
        };
        let _ = self.state_tx.send(next);
        Ok(())
    }

    async fn set_eq_preset(&self, preset: EqualizerPreset) -> Result<(), ProtocolError> {
        self.write_bytes(set_preset_eq(preset)).await?;
        let bands = preset_bands(preset);
        let next = {
            let mut st = self.state.lock().unwrap();
            st.equalizer.preset_code = preset.wire_byte() as i32;
            st.equalizer.bands = bands;
            *st = enforce_defaults(st.clone());
            st.clone()
        };
        let _ = self.state_tx.send(next);
        Ok(())
    }

    async fn set_eq_custom(&self, _clear_bass: i32, bands: &[i32]) -> Result<(), ProtocolError> {
        let normalized: Vec<i32> = (0..EQ_BAND_COUNT)
            .map(|i| bands.get(i).copied().unwrap_or(0).clamp(-6, 6))
            .collect();
        self.write_bytes(set_custom_eq(&normalized)).await?;
        let next = {
            let mut st = self.state.lock().unwrap();
            st.equalizer.preset_code = EqualizerPreset::Manual.wire_byte() as i32;
            st.equalizer.bands = normalized;
            *st = enforce_defaults(st.clone());
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
