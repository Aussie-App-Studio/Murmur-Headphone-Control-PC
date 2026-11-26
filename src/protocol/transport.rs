use thiserror::Error;
use tokio::sync::broadcast;

use crate::models::{AutoPowerOff, EqualizerPreset, HeadphoneState, MdrProtocolVersion, NoiseControlState};
use crate::protocol::sony_v1;
use crate::protocol::sony_v2;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("Bluetooth I/O error: {0}")]
    Io(String),
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Timeout: {0}")]
    Timeout(String),
    #[error("Not connected")]
    NotConnected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BondedDevice {
    pub name: String,
    pub address: String,
    pub is_connected: bool,
    pub class_of_device: u32,
    pub detected_protocol: MdrProtocolVersion,
    pub preferred_service_uuid: Option<String>,
}

impl BondedDevice {
    pub fn preferred_protocols(&self) -> Vec<MdrProtocolVersion> {
        match self.detected_protocol {
            MdrProtocolVersion::V1 => vec![MdrProtocolVersion::V1, MdrProtocolVersion::V2],
            MdrProtocolVersion::V2 => vec![MdrProtocolVersion::V2, MdrProtocolVersion::V1],
            other => vec![other],
        }
    }
}

const SONY_NAME_PREFIXES: &[&str] = &[
    "WH-", "WH1000", "WF-", "WF1000", "ULT WEAR", "LINKBUDS", "INZONE", "WI-", "MDR-",
    "FLOAT RUN", "H.EAR",
];
const SONY_V1_MARKERS: &[&str] = &[
    "WH-1000XM3",
    "WH1000XM3",
    "WH-1000XM2",
    "WH1000XM2",
    "MDR-1000X",
    "MDR1000X",
    "WH-H900N",
    "WHH900N",
    "H.EAR ON 2",
    "WF-1000XM3",
    "WF1000XM3",
    "WI-1000X",
    "WI1000X",
    "WH-XB900N",
    "WHXB900N",
];
const BOSE_NAME_MARKERS: &[&str] = &[
    "BOSE",
    "QC35",
    "QC45",
    "QC ULTRA",
    "QUIETCOMFORT",
    "NC 700",
    "NC700",
];
const SOUNDCORE_NAME_MARKERS: &[&str] = &[
    "SOUNDCORE",
    "LIFE Q",
    "SPACE Q",
    "SPACE ONE",
    "SPACE A40",
    "LIBERTY 4",
    "LIBERTY 3",
];

pub const SOUNDCORE_DEFAULT_UUID: &str = "0cf12d31-fac3-4553-bd80-d6832e7b3028";
pub const GALAXY_BUDS_UUID: &str = "2E73A4AD-332D-41FC-90E2-16BEF06523F2";
pub const AIRPODS_UUID: &str = "74EC2172-0BAD-4D01-8F77-997B2BE0722A";
pub const SPP_UUID: &str = "00001101-0000-1000-8000-00805F9B34FB";

pub fn service_uuid_for_protocol(proto: MdrProtocolVersion) -> &'static str {
    match proto {
        MdrProtocolVersion::V1 => sony_v1::SERVICE_UUID_V1,
        MdrProtocolVersion::V2 => sony_v2::SERVICE_UUID_V2,
        MdrProtocolVersion::BoseBmap => SPP_UUID,
        MdrProtocolVersion::Soundcore => SOUNDCORE_DEFAULT_UUID,
        MdrProtocolVersion::GalaxyBuds => GALAXY_BUDS_UUID,
        MdrProtocolVersion::AirPodsAap => AIRPODS_UUID,
        MdrProtocolVersion::GenericBt => SPP_UUID,
    }
}

/// Classifies a paired Windows Bluetooth device by name & class of device.
/// Returns `None` for non-audio devices (mice, keyboards, phones) unless they match a known headphone name.
pub fn classify_device(
    name: &str,
    address: &str,
    is_connected: bool,
    class_of_device: u32,
) -> Option<BondedDevice> {
    if name.to_uppercase().starts_with("LE_") {
        return None;
    }
    let upper = name.to_uppercase();
    let is_sony_v1 = SONY_V1_MARKERS.iter().any(|m| upper.contains(m));
    let is_sony = is_sony_v1 || SONY_NAME_PREFIXES.iter().any(|p| upper.contains(p));
    let is_bose = BOSE_NAME_MARKERS.iter().any(|m| upper.contains(m));
    let is_soundcore = SOUNDCORE_NAME_MARKERS.iter().any(|m| upper.contains(m));
    let is_galaxy_buds = upper.contains("GALAXY BUDS");
    let is_apple_audio =
        upper.contains("AIRPODS") || upper.contains("BEATS") || upper.contains("POWERBEATS");

    // Major Device Class 0x04 = Audio/Video (bits 8..12 => (cod & 0x1F00) == 0x0400)
    let is_audio_cod = (class_of_device & 0x1F00) == 0x0400;

    let (proto, uuid) = if is_sony_v1 {
        (MdrProtocolVersion::V1, Some(sony_v1::SERVICE_UUID_V1.to_string()))
    } else if is_sony {
        (MdrProtocolVersion::V2, Some(sony_v2::SERVICE_UUID_V2.to_string()))
    } else if is_apple_audio {
        (MdrProtocolVersion::AirPodsAap, Some(AIRPODS_UUID.to_string()))
    } else if is_galaxy_buds {
        (MdrProtocolVersion::GalaxyBuds, Some(GALAXY_BUDS_UUID.to_string()))
    } else if is_soundcore {
        (MdrProtocolVersion::Soundcore, Some(SOUNDCORE_DEFAULT_UUID.to_string()))
    } else if is_bose {
        (MdrProtocolVersion::BoseBmap, Some(SPP_UUID.to_string()))
    } else if is_audio_cod {
        (MdrProtocolVersion::GenericBt, None)
    } else {
        return None;
    };

    let display_name = if name.trim().is_empty() {
        "Bluetooth Headphones".to_string()
    } else {
        name.trim().to_string()
    };

    Some(BondedDevice {
        name: display_name,
        address: address.to_uppercase(),
        is_connected,
        class_of_device,
        detected_protocol: proto,
        preferred_service_uuid: uuid,
    })
}

/// Platform-agnostic synchronous RFCOMM / stream transport trait.
/// Blocking `read`/`write`/`connect` calls are run inside Tokio's blocking pool or dedicated reader thread.
pub trait BluetoothTransport: Send + Sync {
    fn is_connected(&self) -> bool;
    fn connect(&self, address: &str, service_uuid: &str) -> Result<(), ProtocolError>;
    fn read(&self, buf: &mut [u8]) -> Result<usize, ProtocolError>;
    fn write(&self, bytes: &[u8]) -> Result<(), ProtocolError>;
    fn close(&self);
}

#[async_trait::async_trait]
#[allow(dead_code)]
pub trait HeadphoneSession: Send + Sync {
    fn protocol_version(&self) -> MdrProtocolVersion;
    fn is_connected(&self) -> bool;
    fn subscribe_state(&self) -> broadcast::Receiver<HeadphoneState>;
    fn subscribe_disconnect(&self) -> broadcast::Receiver<()>;

    async fn connect_and_sync(
        &self,
        address: &str,
        device_name: &str,
    ) -> Result<HeadphoneState, ProtocolError>;
    async fn poll_battery(&self) -> Result<(), ProtocolError>;

    async fn set_noise_control(&self, _state: &NoiseControlState) -> Result<(), ProtocolError> {
        Ok(())
    }
    async fn set_eq_preset(&self, _preset: EqualizerPreset) -> Result<(), ProtocolError> {
        Ok(())
    }
    async fn set_eq_custom(&self, _clear_bass: i32, _bands: &[i32]) -> Result<(), ProtocolError> {
        Ok(())
    }
    async fn set_auto_power_off(&self, _mode: AutoPowerOff) -> Result<(), ProtocolError> {
        Ok(())
    }
    async fn set_voice_guidance(&self, _enabled: bool) -> Result<(), ProtocolError> {
        Ok(())
    }
    async fn set_touch_panel_enabled(&self, _enabled: BooleanParam) -> Result<(), ProtocolError> {
        Ok(())
    }
    async fn set_multipoint_enabled(&self, _enabled: BooleanParam) -> Result<(), ProtocolError> {
        Ok(())
    }
    fn close(&self);
}

pub type BooleanParam = bool;
