use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::{broadcast, oneshot, Mutex as AsyncMutex};

use crate::models::{
    AutoPowerOff, HeadphoneCapabilities, HeadphoneState, MdrProtocolVersion, NoiseControlMode,
    NoiseControlState, PairedDevice,
};
use crate::protocol::transport::{BluetoothTransport, HeadphoneSession, ProtocolError, SPP_UUID};

pub const OP_SET: u8 = 0x00;
pub const OP_GET: u8 = 0x01;
pub const OP_SETGET: u8 = 0x02;
pub const OP_STATUS: u8 = 0x03;
pub const OP_ERROR: u8 = 0x04;

pub const BLOCK_PRODUCT_INFO: u8 = 0x00;
pub const FUNC_FIRMWARE_VERSION: u8 = 0x05;

pub const BLOCK_SETTINGS: u8 = 0x01;
pub const FUNC_DEVICE_NAME: u8 = 0x02;
pub const FUNC_VOICE_PROMPTS: u8 = 0x03;
pub const FUNC_AUTO_OFF: u8 = 0x04;
pub const FUNC_NOISE_CONTROL: u8 = 0x06;

pub const BLOCK_STATUS: u8 = 0x02;
pub const FUNC_BATTERY: u8 = 0x02;

pub const BLOCK_DEVICE_MANAGEMENT: u8 = 0x04;
pub const FUNC_PAIRED_DEVICES: u8 = 0x04;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoseBmapFrame {
    pub block: u8,
    pub func: u8,
    pub op: u8,
    pub payload: Vec<u8>,
}

pub fn encode_frame(frame: &BoseBmapFrame) -> Result<Vec<u8>, ProtocolError> {
    if frame.payload.len() > 0xFF {
        return Err(ProtocolError::Protocol(format!(
            "BMAP payload exceeds 255 bytes: {}",
            frame.payload.len()
        )));
    }
    let mut out = Vec::with_capacity(4 + frame.payload.len());
    out.push(frame.block);
    out.push(frame.func);
    out.push(frame.op);
    out.push(frame.payload.len() as u8);
    out.extend_from_slice(&frame.payload);
    Ok(out)
}

pub fn get_firmware_version() -> Vec<u8> {
    encode_frame(&BoseBmapFrame {
        block: BLOCK_PRODUCT_INFO,
        func: FUNC_FIRMWARE_VERSION,
        op: OP_GET,
        payload: Vec::new(),
    })
    .unwrap()
}

pub fn get_device_name() -> Vec<u8> {
    encode_frame(&BoseBmapFrame {
        block: BLOCK_SETTINGS,
        func: FUNC_DEVICE_NAME,
        op: OP_GET,
        payload: Vec::new(),
    })
    .unwrap()
}

pub fn get_voice_prompts() -> Vec<u8> {
    encode_frame(&BoseBmapFrame {
        block: BLOCK_SETTINGS,
        func: FUNC_VOICE_PROMPTS,
        op: OP_GET,
        payload: Vec::new(),
    })
    .unwrap()
}

pub fn get_auto_off() -> Vec<u8> {
    encode_frame(&BoseBmapFrame {
        block: BLOCK_SETTINGS,
        func: FUNC_AUTO_OFF,
        op: OP_GET,
        payload: Vec::new(),
    })
    .unwrap()
}

pub fn get_noise_control() -> Vec<u8> {
    encode_frame(&BoseBmapFrame {
        block: BLOCK_SETTINGS,
        func: FUNC_NOISE_CONTROL,
        op: OP_GET,
        payload: Vec::new(),
    })
    .unwrap()
}

pub fn get_battery() -> Vec<u8> {
    encode_frame(&BoseBmapFrame {
        block: BLOCK_STATUS,
        func: FUNC_BATTERY,
        op: OP_GET,
        payload: Vec::new(),
    })
    .unwrap()
}

pub fn get_paired_devices() -> Vec<u8> {
    encode_frame(&BoseBmapFrame {
        block: BLOCK_DEVICE_MANAGEMENT,
        func: FUNC_PAIRED_DEVICES,
        op: OP_GET,
        payload: Vec::new(),
    })
    .unwrap()
}

pub fn set_noise_control(state: &NoiseControlState) -> Vec<u8> {
    let level_byte = match state.mode {
        NoiseControlMode::Off => 0x00,
        NoiseControlMode::NoiseCancelling => 0x01,
        NoiseControlMode::AmbientSound => 0x03,
    };
    encode_frame(&BoseBmapFrame {
        block: BLOCK_SETTINGS,
        func: FUNC_NOISE_CONTROL,
        op: OP_SETGET,
        payload: vec![level_byte],
    })
    .unwrap()
}

pub fn set_auto_off(mode: AutoPowerOff) -> Vec<u8> {
    let minutes_byte = match mode {
        AutoPowerOff::Never => 0x00,
        AutoPowerOff::After5Min => 5,
        AutoPowerOff::After15Min => 15,
        AutoPowerOff::After30Min => 30,
        AutoPowerOff::After1Hour => 60,
        AutoPowerOff::After3Hours => 180,
        _ => 20,
    };
    encode_frame(&BoseBmapFrame {
        block: BLOCK_SETTINGS,
        func: FUNC_AUTO_OFF,
        op: OP_SETGET,
        payload: vec![minutes_byte],
    })
    .unwrap()
}

pub fn set_voice_prompts(enabled: bool) -> Vec<u8> {
    encode_frame(&BoseBmapFrame {
        block: BLOCK_SETTINGS,
        func: FUNC_VOICE_PROMPTS,
        op: OP_SETGET,
        payload: vec![if enabled { 0x01 } else { 0x00 }],
    })
    .unwrap()
}

pub fn apply_frame(state: &HeadphoneState, frame: &BoseBmapFrame) -> HeadphoneState {
    if frame.op != OP_STATUS && frame.op != 0x06 {
        return state.clone();
    }
    let mut next = state.clone();
    match (frame.block, frame.func) {
        (BLOCK_PRODUCT_INFO, FUNC_FIRMWARE_VERSION) => {
            let fw = String::from_utf8_lossy(&frame.payload)
                .trim_end_matches('\0')
                .trim()
                .to_string();
            if !fw.is_empty() {
                next.firmware_version = fw;
            }
        }
        (BLOCK_SETTINGS, FUNC_DEVICE_NAME) => {
            let name = String::from_utf8_lossy(&frame.payload)
                .trim_end_matches('\0')
                .trim()
                .to_string();
            if !name.is_empty() {
                next.model_name = name;
            }
        }
        (BLOCK_SETTINGS, FUNC_VOICE_PROMPTS) => {
            if let Some(&raw) = frame.payload.first() {
                next.voice_guidance_enabled = raw != 0;
            }
        }
        (BLOCK_SETTINGS, FUNC_AUTO_OFF) => {
            if let Some(&minutes) = frame.payload.first() {
                next.auto_power_off = match minutes {
                    0 => AutoPowerOff::Never,
                    5 => AutoPowerOff::After5Min,
                    15 => AutoPowerOff::After15Min,
                    30 => AutoPowerOff::After30Min,
                    60 => AutoPowerOff::After1Hour,
                    180 => AutoPowerOff::After3Hours,
                    _ => AutoPowerOff::After30Min,
                };
            }
        }
        (BLOCK_SETTINGS, FUNC_NOISE_CONTROL) => {
            if let Some(&raw) = frame.payload.first() {
                match raw {
                    0x00 => next.noise_control.mode = NoiseControlMode::Off,
                    0x01 => next.noise_control.mode = NoiseControlMode::NoiseCancelling,
                    0x03 => next.noise_control.mode = NoiseControlMode::AmbientSound,
                    _ => {}
                }
            }
        }
        (BLOCK_STATUS, FUNC_BATTERY) => {
            if let Some(&raw) = frame.payload.first() {
                next.battery.level = Some((raw as i32).clamp(0, 100));
            }
        }
        (BLOCK_DEVICE_MANAGEMENT, FUNC_PAIRED_DEVICES) => {
            if let Some(devices) = parse_paired_devices(&frame.payload) {
                next.paired_devices = devices;
            }
        }
        _ => {}
    }
    next
}

fn parse_paired_devices(payload: &[u8]) -> Option<Vec<PairedDevice>> {
    if payload.is_empty() || (payload.len() == 1 && payload[0] == 0) {
        return Some(Vec::new());
    }
    let (offset, connected_count) = if payload.len() >= 7 && (payload.len() - 1).is_multiple_of(6) {
        (1, payload[0] as usize)
    } else if payload.len() >= 6 && payload.len().is_multiple_of(6) {
        (0, 0)
    } else {
        return None;
    };

    let count = (payload.len() - offset) / 6;
    let mut devices = Vec::with_capacity(count);
    for i in 0..count {
        let base = offset + i * 6;
        let mac = format!(
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            payload[base],
            payload[base + 1],
            payload[base + 2],
            payload[base + 3],
            payload[base + 4],
            payload[base + 5]
        );
        devices.push(PairedDevice {
            address: mac.clone(),
            name: mac,
            is_connected: i < connected_count,
            is_playing: false,
            class_of_device: 0,
        });
    }
    Some(devices)
}

pub struct BoseBmapStreamDecoder {
    buffer: Mutex<Vec<u8>>,
}

impl Default for BoseBmapStreamDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl BoseBmapStreamDecoder {
    pub fn new() -> Self {
        Self {
            buffer: Mutex::new(Vec::with_capacity(256)),
        }
    }

    pub fn feed(&self, chunk: &[u8]) -> Vec<BoseBmapFrame> {
        let mut buf = self.buffer.lock().unwrap();
        buf.extend_from_slice(chunk);
        let mut frames = Vec::new();
        while buf.len() >= 4 {
            let payload_len = buf[3] as usize;
            let total_len = 4 + payload_len;
            if buf.len() < total_len {
                break;
            }
            let frame = BoseBmapFrame {
                block: buf[0],
                func: buf[1],
                op: buf[2],
                payload: buf[4..total_len].to_vec(),
            };
            buf.drain(0..total_len);
            frames.push(frame);
        }
        frames
    }

    pub fn clear(&self) {
        self.buffer.lock().unwrap().clear();
    }
}

struct PendingWaiter {
    id: u64,
    block: u8,
    func: u8,
    tx: oneshot::Sender<BoseBmapFrame>,
}

pub struct BoseBmapSession {
    transport: Arc<dyn BluetoothTransport>,
    service_uuid: String,
    write_mutex: AsyncMutex<()>,
    state: Mutex<HeadphoneState>,
    synced: AtomicBool,
    reader_active: Arc<AtomicBool>,
    reader_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    decoder: BoseBmapStreamDecoder,
    waiters: Mutex<(u64, Vec<PendingWaiter>)>,
    state_tx: broadcast::Sender<HeadphoneState>,
    disconnect_tx: broadcast::Sender<()>,
}

impl BoseBmapSession {
    pub fn new(transport: Arc<dyn BluetoothTransport>, service_uuid: Option<String>) -> Arc<Self> {
        let (state_tx, _) = broadcast::channel(32);
        let (disconnect_tx, _) = broadcast::channel(8);
        Arc::new(Self {
            transport,
            service_uuid: service_uuid.unwrap_or_else(|| SPP_UUID.to_string()),
            write_mutex: AsyncMutex::new(()),
            state: Mutex::new(HeadphoneState {
                model_name: "Bose Headphones".to_string(),
                protocol_version: MdrProtocolVersion::BoseBmap,
                custom_capabilities: Some(HeadphoneCapabilities::bose_bmap()),
                ..Default::default()
            }),
            synced: AtomicBool::new(false),
            reader_active: Arc::new(AtomicBool::new(false)),
            reader_handle: Mutex::new(None),
            decoder: BoseBmapStreamDecoder::new(),
            waiters: Mutex::new((1, Vec::new())),
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
            session.synced.store(false, Ordering::SeqCst);
            session.waiters.lock().unwrap().1.clear();
            let _ = session.disconnect_tx.send(());
        });
        *self.reader_handle.lock().unwrap() = Some(handle);
    }

    fn handle_frame(&self, frame: BoseBmapFrame) {
        if frame.op == OP_STATUS || frame.op == 0x06 {
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
            if self.synced.load(Ordering::SeqCst) {
                if let Some(next) = emitted {
                    let _ = self.state_tx.send(next);
                }
            }
        }
        if frame.op == OP_STATUS || frame.op == 0x06 || frame.op == OP_ERROR {
            let mut waiters = self.waiters.lock().unwrap();
            if let Some(pos) = waiters
                .1
                .iter()
                .position(|w| w.block == frame.block && w.func == frame.func)
            {
                let w = waiters.1.remove(pos);
                let _ = w.tx.send(frame);
            }
        }
    }

    async fn query_and_await(
        &self,
        block: u8,
        func: u8,
        cmd: Vec<u8>,
        timeout: Duration,
    ) -> Result<Option<BoseBmapFrame>, ProtocolError> {
        if !self.transport.is_connected() {
            return Err(ProtocolError::NotConnected);
        }
        let (tx, rx) = oneshot::channel();
        let id = {
            let mut w = self.waiters.lock().unwrap();
            let id = w.0;
            w.0 += 1;
            w.1.push(PendingWaiter {
                id,
                block,
                func,
                tx,
            });
            id
        };

        {
            let _wg = self.write_mutex.lock().await;
            let transport = Arc::clone(&self.transport);
            tokio::task::spawn_blocking(move || transport.write(&cmd))
                .await
                .map_err(|e| ProtocolError::Io(e.to_string()))??;
        }

        let res = tokio::time::timeout(timeout, rx).await;
        if res.is_err() {
            let mut w = self.waiters.lock().unwrap();
            w.1.retain(|pw| pw.id != id);
        }
        Ok(res.ok().and_then(|r| r.ok()))
    }
}

#[async_trait::async_trait]
impl HeadphoneSession for Arc<BoseBmapSession> {
    fn protocol_version(&self) -> MdrProtocolVersion {
        MdrProtocolVersion::BoseBmap
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
        {
            let mut st = self.state.lock().unwrap();
            *st = HeadphoneState {
                model_name: if device_name.trim().is_empty() {
                    "Bose Headphones".to_string()
                } else {
                    device_name.trim().to_string()
                },
                bluetooth_address: address.to_string(),
                protocol_version: MdrProtocolVersion::BoseBmap,
                custom_capabilities: Some(HeadphoneCapabilities::bose_bmap()),
                ..Default::default()
            };
        }

        let transport = Arc::clone(&self.transport);
        let addr = address.to_string();
        let uuid = self.service_uuid.clone();
        tokio::task::spawn_blocking(move || transport.connect(&addr, &uuid))
            .await
            .map_err(|e| ProtocolError::Io(e.to_string()))??;

        self.start_reader();
        let t = Duration::from_millis(750);
        let _ = self.query_and_await(BLOCK_SETTINGS, FUNC_DEVICE_NAME, get_device_name(), t).await;
        let _ = self.query_and_await(BLOCK_PRODUCT_INFO, FUNC_FIRMWARE_VERSION, get_firmware_version(), t).await;
        let _ = self.query_and_await(BLOCK_STATUS, FUNC_BATTERY, get_battery(), t).await;
        let _ = self.query_and_await(BLOCK_SETTINGS, FUNC_NOISE_CONTROL, get_noise_control(), t).await;
        let _ = self.query_and_await(BLOCK_SETTINGS, FUNC_AUTO_OFF, get_auto_off(), t).await;
        let _ = self.query_and_await(BLOCK_SETTINGS, FUNC_VOICE_PROMPTS, get_voice_prompts(), t).await;
        let _ = self.query_and_await(BLOCK_DEVICE_MANAGEMENT, FUNC_PAIRED_DEVICES, get_paired_devices(), t).await;

        self.synced.store(true, Ordering::SeqCst);
        Ok(self.state.lock().unwrap().clone())
    }

    async fn poll_battery(&self) -> Result<(), ProtocolError> {
        if !self.is_connected() {
            return Ok(());
        }
        let _ = self
            .query_and_await(BLOCK_STATUS, FUNC_BATTERY, get_battery(), Duration::from_millis(750))
            .await;
        Ok(())
    }

    async fn set_noise_control(&self, state: &NoiseControlState) -> Result<(), ProtocolError> {
        self.query_and_await(
            BLOCK_SETTINGS,
            FUNC_NOISE_CONTROL,
            set_noise_control(state),
            Duration::from_millis(1000),
        )
        .await?;
        Ok(())
    }

    async fn set_auto_power_off(&self, mode: AutoPowerOff) -> Result<(), ProtocolError> {
        self.query_and_await(
            BLOCK_SETTINGS,
            FUNC_AUTO_OFF,
            set_auto_off(mode),
            Duration::from_millis(1000),
        )
        .await?;
        Ok(())
    }

    async fn set_voice_guidance(&self, enabled: bool) -> Result<(), ProtocolError> {
        self.query_and_await(
            BLOCK_SETTINGS,
            FUNC_VOICE_PROMPTS,
            set_voice_prompts(enabled),
            Duration::from_millis(1000),
        )
        .await?;
        Ok(())
    }

    fn close(&self) {
        self.synced.store(false, Ordering::SeqCst);
        self.reader_active.store(false, Ordering::SeqCst);
        if let Some(h) = self.reader_handle.lock().unwrap().take() {
            h.abort();
        }
        self.transport.close();
        self.decoder.clear();
        self.waiters.lock().unwrap().1.clear();
    }
}
