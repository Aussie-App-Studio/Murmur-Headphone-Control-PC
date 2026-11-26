use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MdrProtocolVersion {
    #[serde(rename = "V1")]
    V1,
    #[serde(rename = "V2")]
    V2,
    #[serde(rename = "BOSE_BMAP")]
    BoseBmap,
    #[serde(rename = "SOUNDCORE")]
    Soundcore,
    #[serde(rename = "GALAXY_BUDS")]
    GalaxyBuds,
    #[serde(rename = "AIRPODS_AAP")]
    AirPodsAap,
    #[serde(rename = "GENERIC_BT")]
    GenericBt,
}

impl MdrProtocolVersion {
    pub fn label(self) -> &'static str {
        match self {
            Self::V1 => "MDR V1",
            Self::V2 => "MDR V2",
            Self::BoseBmap => "Bose BMAP",
            Self::Soundcore => "Soundcore",
            Self::GalaxyBuds => "Galaxy Buds",
            Self::AirPodsAap => "Apple / Beats BLE",
            Self::GenericBt => "Bluetooth Audio",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NoiseControlMode {
    #[serde(rename = "NOISE_CANCELLING")]
    NoiseCancelling,
    #[serde(rename = "AMBIENT_SOUND")]
    AmbientSound,
    #[serde(rename = "OFF")]
    Off,
}

impl NoiseControlMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::NoiseCancelling => "Noise Cancelling",
            Self::AmbientSound => "Ambient Sound",
            Self::Off => "Off",
        }
    }

    pub fn next_in_cycle(self, cycle: NcAmbButtonCycle) -> Self {
        match cycle {
            NcAmbButtonCycle::NcAmbient => {
                if self == Self::NoiseCancelling {
                    Self::AmbientSound
                } else {
                    Self::NoiseCancelling
                }
            }
            NcAmbButtonCycle::NcOff => {
                if self == Self::NoiseCancelling {
                    Self::Off
                } else {
                    Self::NoiseCancelling
                }
            }
            NcAmbButtonCycle::AmbientOff => {
                if self == Self::AmbientSound {
                    Self::Off
                } else {
                    Self::AmbientSound
                }
            }
            NcAmbButtonCycle::NcAmbientOff => match self {
                Self::NoiseCancelling => Self::AmbientSound,
                Self::AmbientSound => Self::Off,
                Self::Off => Self::NoiseCancelling,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoiseControlState {
    pub mode: NoiseControlMode,
    pub ambient_level: i32,
    pub focus_on_voice: bool,
}

impl Default for NoiseControlState {
    fn default() -> Self {
        Self {
            mode: NoiseControlMode::NoiseCancelling,
            ambient_level: 10,
            focus_on_voice: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NcAmbButtonCycle {
    NcAmbientOff,
    NcAmbient,
    NcOff,
    AmbientOff,
}

impl NcAmbButtonCycle {
    pub const ALL: [Self; 4] = [
        Self::NcAmbientOff,
        Self::NcAmbient,
        Self::NcOff,
        Self::AmbientOff,
    ];

    pub fn wire_value(self) -> u8 {
        match self {
            Self::NcAmbientOff => 0x01,
            Self::NcAmbient => 0x02,
            Self::NcOff => 0x03,
            Self::AmbientOff => 0x04,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::NcAmbientOff => "NC -> Ambient -> Off",
            Self::NcAmbient => "NC <-> Ambient (Skip Off)",
            Self::NcOff => "NC <-> Off",
            Self::AmbientOff => "Ambient <-> Off",
        }
    }

    pub fn from_byte(b: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.wire_value() == b)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AudioCodec {
    Unsettled,
    Sbc,
    Aac,
    Ldac,
    AptX,
    AptXHd,
    Lc3,
    Other,
}

impl AudioCodec {
    pub fn wire_byte(self) -> u8 {
        match self {
            Self::Unsettled => 0x00,
            Self::Sbc => 0x01,
            Self::Aac => 0x02,
            Self::Ldac => 0x10,
            Self::AptX => 0x20,
            Self::AptXHd => 0x21,
            Self::Lc3 => 0x30,
            Self::Other => 0xFF,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Unsettled => "—",
            Self::Sbc => "SBC",
            Self::Aac => "AAC",
            Self::Ldac => "LDAC",
            Self::AptX => "aptX",
            Self::AptXHd => "aptX HD",
            Self::Lc3 => "LC3",
            Self::Other => "Other",
        }
    }

    pub fn is_high_draw(self) -> bool {
        matches!(self, Self::Ldac | Self::AptXHd)
    }

    pub fn from_byte(b: u8) -> Self {
        match b {
            0x00 => Self::Unsettled,
            0x01 => Self::Sbc,
            0x02 => Self::Aac,
            0x10 => Self::Ldac,
            0x20 => Self::AptX,
            0x21 => Self::AptXHd,
            0x30 => Self::Lc3,
            _ => Self::Other,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChargingStatus {
    #[serde(rename = "NOT_CHARGING")]
    NotCharging,
    #[serde(rename = "CHARGING")]
    Charging,
    #[serde(rename = "UNKNOWN")]
    Unknown,
    #[serde(rename = "CHARGED")]
    Charged,
}

impl ChargingStatus {
    pub fn wire_byte(self) -> u8 {
        match self {
            Self::NotCharging => 0x00,
            Self::Charging => 0x01,
            Self::Unknown => 0x02,
            Self::Charged => 0x03,
        }
    }

    pub fn is_plugged_in(self) -> bool {
        matches!(self, Self::Charging | Self::Charged)
    }

    pub fn from_byte(b: u8) -> Self {
        match b {
            0x00 => Self::NotCharging,
            0x01 => Self::Charging,
            0x03 => Self::Charged,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatteryUnit {
    pub level: i32,
    pub charging_status: ChargingStatus,
}

impl BatteryUnit {
    pub fn new(level: i32, charging_status: ChargingStatus) -> Self {
        Self {
            level,
            charging_status,
        }
    }

    pub fn is_charging(&self) -> bool {
        self.charging_status == ChargingStatus::Charging
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct BatteryState {
    pub level: Option<i32>,
    pub charging_status: ChargingStatus,
    pub left: Option<BatteryUnit>,
    pub right: Option<BatteryUnit>,
    pub case_battery: Option<BatteryUnit>,
}

impl Default for ChargingStatus {
    fn default() -> Self {
        Self::NotCharging
    }
}

impl BatteryState {
    pub fn is_charging(&self) -> bool {
        self.charging_status == ChargingStatus::Charging
            || self.left.as_ref().is_some_and(|u| u.is_charging())
            || self.right.as_ref().is_some_and(|u| u.is_charging())
    }

    pub fn is_dual_battery(&self) -> bool {
        self.left.is_some() || self.right.is_some()
    }

    pub fn effective_level(&self) -> Option<i32> {
        if self.is_dual_battery() {
            [self.left.as_ref().map(|u| u.level), self.right.as_ref().map(|u| u.level)]
                .into_iter()
                .flatten()
                .min()
                .or(self.level)
        } else {
            self.level
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EqualizerPreset {
    Off,
    Bright,
    Excited,
    Mellow,
    Relaxed,
    Vocal,
    TrebleBoost,
    BassBoost,
    Speech,
    Manual,
    Custom1,
    Custom2,
}

impl EqualizerPreset {
    pub const ALL: [Self; 12] = [
        Self::Off,
        Self::Bright,
        Self::Excited,
        Self::Mellow,
        Self::Relaxed,
        Self::Vocal,
        Self::TrebleBoost,
        Self::BassBoost,
        Self::Speech,
        Self::Manual,
        Self::Custom1,
        Self::Custom2,
    ];

    pub fn wire_byte(self) -> u8 {
        match self {
            Self::Off => 0x00,
            Self::Bright => 0x10,
            Self::Excited => 0x11,
            Self::Mellow => 0x12,
            Self::Relaxed => 0x13,
            Self::Vocal => 0x14,
            Self::TrebleBoost => 0x15,
            Self::BassBoost => 0x16,
            Self::Speech => 0x17,
            Self::Manual => 0xA0,
            Self::Custom1 => 0xA1,
            Self::Custom2 => 0xA2,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Bright => "Bright",
            Self::Excited => "Excited",
            Self::Mellow => "Mellow",
            Self::Relaxed => "Relaxed",
            Self::Vocal => "Vocal",
            Self::TrebleBoost => "Treble Boost",
            Self::BassBoost => "Bass Boost",
            Self::Speech => "Speech",
            Self::Manual => "Manual",
            Self::Custom1 => "Custom 1",
            Self::Custom2 => "Custom 2",
        }
    }

    pub fn is_customizable(self) -> bool {
        matches!(self, Self::Manual | Self::Custom1 | Self::Custom2)
    }

    pub fn from_byte(b: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.wire_byte() == b)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EqualizerState {
    pub preset_code: i32,
    pub clear_bass: i32,
    pub bands: Vec<i32>,
    pub available_preset_codes: Vec<i32>,
    pub band_count: i32,
    pub step_count: i32,
    pub has_clear_bass: bool,
}

impl Default for EqualizerState {
    fn default() -> Self {
        Self {
            preset_code: EqualizerPreset::Off.wire_byte() as i32,
            clear_bass: 0,
            bands: vec![0, 0, 0, 0, 0],
            available_preset_codes: Vec::new(),
            band_count: 5,
            step_count: 21,
            has_clear_bass: true,
        }
    }
}

impl EqualizerState {
    pub fn preset(&self) -> Option<EqualizerPreset> {
        EqualizerPreset::from_byte((self.preset_code & 0xFF) as u8)
    }

    pub fn max_gain(&self) -> i32 {
        (self.step_count / 2).max(1)
    }

    pub fn min_gain(&self) -> i32 {
        -self.max_gain()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConnectionPriority {
    SoundQuality,
    StableConnection,
}

impl ConnectionPriority {
    pub const ALL: [Self; 2] = [Self::SoundQuality, Self::StableConnection];

    pub fn wire_byte(self) -> u8 {
        match self {
            Self::SoundQuality => 0x00,
            Self::StableConnection => 0x01,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::SoundQuality => "Prioritize Sound Quality",
            Self::StableConnection => "Prioritize Stable Connection",
        }
    }

    pub fn from_byte(b: u8) -> Self {
        if b == 0x01 {
            Self::StableConnection
        } else {
            Self::SoundQuality
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpeakToChatSensitivity {
    Auto,
    High,
    Low,
}

impl SpeakToChatSensitivity {
    pub const ALL: [Self; 3] = [Self::Auto, Self::High, Self::Low];

    pub fn wire_byte(self) -> u8 {
        match self {
            Self::Auto => 0x00,
            Self::High => 0x01,
            Self::Low => 0x02,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::High => "High",
            Self::Low => "Low",
        }
    }

    pub fn from_byte(b: u8) -> Self {
        match b {
            0x01 => Self::High,
            0x02 => Self::Low,
            _ => Self::Auto,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpeakToChatTimeout {
    Short5s,
    Standard15s,
    Long30s,
    Never,
}

impl SpeakToChatTimeout {
    pub const ALL: [Self; 4] = [
        Self::Short5s,
        Self::Standard15s,
        Self::Long30s,
        Self::Never,
    ];

    pub fn wire_byte(self) -> u8 {
        match self {
            Self::Short5s => 0x00,
            Self::Standard15s => 0x01,
            Self::Long30s => 0x02,
            Self::Never => 0x03,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Short5s => "Short (~5s)",
            Self::Standard15s => "Standard (~15s)",
            Self::Long30s => "Long (~30s)",
            Self::Never => "Don't end automatically",
        }
    }

    pub fn from_byte(b: u8) -> Self {
        match b {
            0x00 => Self::Short5s,
            0x02 => Self::Long30s,
            0x03 => Self::Never,
            _ => Self::Standard15s,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AutoPowerOff {
    After5Min,
    After15Min,
    After30Min,
    After1Hour,
    After3Hours,
    WhenRemoved,
    Never,
}

impl AutoPowerOff {
    pub const ALL: [Self; 7] = [
        Self::After5Min,
        Self::After15Min,
        Self::After30Min,
        Self::After1Hour,
        Self::After3Hours,
        Self::WhenRemoved,
        Self::Never,
    ];

    pub fn wire_byte(self) -> u8 {
        match self {
            Self::After5Min => 0x00,
            Self::After30Min => 0x01,
            Self::After1Hour => 0x02,
            Self::After3Hours => 0x03,
            Self::After15Min => 0x04,
            Self::WhenRemoved => 0x10,
            Self::Never => 0x11,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::After5Min => "After 5 minutes",
            Self::After15Min => "After 15 minutes",
            Self::After30Min => "After 30 minutes",
            Self::After1Hour => "After 1 hour",
            Self::After3Hours => "After 3 hours",
            Self::WhenRemoved => "When headphones are removed",
            Self::Never => "Do not turn off",
        }
    }

    pub fn from_byte(b: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.wire_byte() == b)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VoiceAssistant {
    DeviceDefault,
    GoogleAssistant,
    Alexa,
    Tencent,
    Sony,
    SetElsewhere,
    Off,
}

impl VoiceAssistant {
    pub const ALL: [Self; 7] = [
        Self::DeviceDefault,
        Self::GoogleAssistant,
        Self::Alexa,
        Self::Tencent,
        Self::Sony,
        Self::SetElsewhere,
        Self::Off,
    ];

    pub fn wire_byte(self) -> u8 {
        match self {
            Self::DeviceDefault => 0x30,
            Self::GoogleAssistant => 0x31,
            Self::Alexa => 0x32,
            Self::Tencent => 0x33,
            Self::Sony => 0x34,
            Self::SetElsewhere => 0x3F,
            Self::Off => 0xFF,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::DeviceDefault => "Device Voice Assistant",
            Self::GoogleAssistant => "Google Assistant",
            Self::Alexa => "Amazon Alexa",
            Self::Tencent => "Tencent Xiaowei",
            Self::Sony => "Sony Voice Assistant",
            Self::SetElsewhere => "Configured on another device",
            Self::Off => "Off",
        }
    }

    pub fn from_byte(b: u8) -> Self {
        Self::ALL
            .into_iter()
            .find(|v| v.wire_byte() == b)
            .unwrap_or(Self::Off)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AssignablePreset {
    AmbientSoundControl,
    VolumeControl,
    PlaybackControl,
    VoiceRecognition,
    GoogleAssistant,
    Alexa,
    Tencent,
    NoFunction,
}

impl AssignablePreset {
    pub const ALL: [Self; 8] = [
        Self::AmbientSoundControl,
        Self::VolumeControl,
        Self::PlaybackControl,
        Self::VoiceRecognition,
        Self::GoogleAssistant,
        Self::Alexa,
        Self::Tencent,
        Self::NoFunction,
    ];

    pub fn wire_byte(self) -> u8 {
        match self {
            Self::AmbientSoundControl => 0x00,
            Self::VolumeControl => 0x10,
            Self::PlaybackControl => 0x20,
            Self::VoiceRecognition => 0x30,
            Self::GoogleAssistant => 0x31,
            Self::Alexa => 0x32,
            Self::Tencent => 0x33,
            Self::NoFunction => 0xFF,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::AmbientSoundControl => "Ambient Sound Control",
            Self::VolumeControl => "Volume Control",
            Self::PlaybackControl => "Playback Control",
            Self::VoiceRecognition => "Voice Recognition",
            Self::GoogleAssistant => "Google Assistant",
            Self::Alexa => "Amazon Alexa",
            Self::Tencent => "Tencent Xiaowei",
            Self::NoFunction => "None",
        }
    }

    pub fn from_byte(b: u8) -> Self {
        Self::ALL
            .into_iter()
            .find(|v| v.wire_byte() == b)
            .unwrap_or(Self::NoFunction)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssignableSetting {
    pub key_side: i32,
    pub preset: AssignablePreset,
    pub available_presets: Vec<AssignablePreset>,
}

impl AssignableSetting {
    pub fn side_label(&self) -> String {
        match self.key_side {
            0 => "Left".to_string(),
            1 => "Right".to_string(),
            n => format!("Button {}", n + 1),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct QuickAccessService(pub u8);

impl QuickAccessService {
    pub fn label(self) -> String {
        match self.0 {
            0x00 => "Off".to_string(),
            0x01 => "Spotify Tap".to_string(),
            other => format!("Service {}", other),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneralSetting {
    pub id: i32,
    pub key: String,
    pub summary_key: String,
    pub is_on: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairedDevice {
    pub address: String,
    pub name: String,
    pub is_connected: bool,
    pub is_playing: bool,
    pub class_of_device: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeadsetAlert {
    pub message_type: i32,
    pub needs_answer: bool,
}

impl HeadsetAlert {
    pub fn message(&self) -> String {
        match self.message_type {
            0x00 => "Headphones will briefly reconnect to change connection mode.".to_string(),
            0x01 => "Headphones will briefly reconnect to update button assignment.".to_string(),
            0x06 | 0x07 => "Headphones will briefly reconnect to change Multipoint.".to_string(),
            0x08 | 0x2B => "Battery life will be shorter with this sound setting.".to_string(),
            0x0A | 0x0B => "Touch sensor control panel will be turned off.".to_string(),
            0x0C | 0x0D => "Touch sensor control panel will be turned on.".to_string(),
            0x16 | 0x17 | 0x18 => "Voice assistant setting will be updated.".to_string(),
            0x1C | 0x2A => "LDAC is unavailable with this connection setting.".to_string(),
            0x70 | 0x72 => "With two devices connected, LDAC runs at a lower bitrate.".to_string(),
            0x71 => "Prioritizing sound quality with two devices connected may reduce stability.".to_string(),
            other => format!("Headphones request confirmation (0x{:02X}).", other),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HeadphoneFinish {
    #[serde(rename = "OBSIDIAN_BLACK")]
    ObsidianBlack,
    #[serde(rename = "PLATINUM_SILVER")]
    PlatinumSilver,
    #[serde(rename = "MIDNIGHT_BLUE")]
    MidnightBlue,
    #[serde(rename = "SMOKY_PINK")]
    SmokyPink,
    #[serde(rename = "WARM_WHITE")]
    WarmWhite,
    #[serde(rename = "FOREST_GRAY")]
    ForestGray,
    #[serde(rename = "CRIMSON_RED")]
    CrimsonRed,
    #[serde(rename = "CHAMPAGNE_GOLD")]
    ChampagneGold,
    #[serde(rename = "VIOLET")]
    Violet,
}

impl Default for HeadphoneFinish {
    fn default() -> Self {
        Self::ObsidianBlack
    }
}

impl HeadphoneFinish {
    pub fn label(self) -> &'static str {
        match self {
            Self::ObsidianBlack => "Black",
            Self::PlatinumSilver => "Silver",
            Self::MidnightBlue => "Blue",
            Self::SmokyPink => "Pink",
            Self::WarmWhite => "White",
            Self::ForestGray => "Forest Gray",
            Self::CrimsonRed => "Red",
            Self::ChampagneGold => "Champagne Gold",
            Self::Violet => "Violet",
        }
    }

    pub fn shell_rgb(self) -> u32 {
        match self {
            Self::ObsidianBlack => 0x23262B,
            Self::PlatinumSilver => 0xD8D2C9,
            Self::MidnightBlue => 0x1E3048,
            Self::SmokyPink => 0xCCA9A7,
            Self::WarmWhite => 0xECEAE4,
            Self::ForestGray => 0x4F5A54,
            Self::CrimsonRed => 0x8F242C,
            Self::ChampagneGold => 0xCDBBA5,
            Self::Violet => 0x6D5D7E,
        }
    }

    pub fn cushion_rgb(self) -> u32 {
        match self {
            Self::ObsidianBlack => 0x16181C,
            Self::PlatinumSilver => 0xC7C0B6,
            Self::MidnightBlue => 0x152234,
            Self::SmokyPink => 0xBA9694,
            Self::WarmWhite => 0xDAD6CE,
            Self::ForestGray => 0x39413D,
            Self::CrimsonRed => 0x66181E,
            Self::ChampagneGold => 0xB2A08A,
            Self::Violet => 0x51445E,
        }
    }

    pub fn metal_rgb(self) -> u32 {
        match self {
            Self::ObsidianBlack => 0x7A7E87,
            Self::PlatinumSilver => 0xB8B1A7,
            Self::MidnightBlue => 0x607898,
            Self::SmokyPink => 0xD4B9B0,
            Self::WarmWhite => 0xB8B4AC,
            Self::ForestGray => 0x8D9690,
            Self::CrimsonRed => 0xC0767B,
            Self::ChampagneGold => 0xD9C7B2,
            Self::Violet => 0xA294B2,
        }
    }

    pub fn from_sony_color_byte(code: i32) -> Self {
        match code & 0xFF {
            0x01 => Self::ObsidianBlack,
            0x02 | 0x03 | 0x09 | 0x0B => Self::PlatinumSilver,
            0x04 => Self::CrimsonRed,
            0x05 => Self::MidnightBlue,
            0x06 => Self::SmokyPink,
            0x07 => Self::WarmWhite,
            0x08 => Self::ForestGray,
            0x0A => Self::ChampagneGold,
            0x0C => Self::Violet,
            _ => Self::ObsidianBlack,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HeadphoneFinishOverride {
    Auto,
    ObsidianBlack,
    PlatinumSilver,
    MidnightBlue,
    SmokyPink,
    WarmWhite,
    ForestGray,
    CrimsonRed,
    ChampagneGold,
    Violet,
}

impl Default for HeadphoneFinishOverride {
    fn default() -> Self {
        Self::Auto
    }
}

impl HeadphoneFinishOverride {
    pub const ALL: [Self; 10] = [
        Self::Auto,
        Self::ObsidianBlack,
        Self::PlatinumSilver,
        Self::MidnightBlue,
        Self::SmokyPink,
        Self::WarmWhite,
        Self::ForestGray,
        Self::CrimsonRed,
        Self::ChampagneGold,
        Self::Violet,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto (From Headphones)",
            Self::ObsidianBlack => HeadphoneFinish::ObsidianBlack.label(),
            Self::PlatinumSilver => HeadphoneFinish::PlatinumSilver.label(),
            Self::MidnightBlue => HeadphoneFinish::MidnightBlue.label(),
            Self::SmokyPink => HeadphoneFinish::SmokyPink.label(),
            Self::WarmWhite => HeadphoneFinish::WarmWhite.label(),
            Self::ForestGray => HeadphoneFinish::ForestGray.label(),
            Self::CrimsonRed => HeadphoneFinish::CrimsonRed.label(),
            Self::ChampagneGold => HeadphoneFinish::ChampagneGold.label(),
            Self::Violet => HeadphoneFinish::Violet.label(),
        }
    }

    pub fn resolve(self, detected: HeadphoneFinish) -> HeadphoneFinish {
        match self {
            Self::Auto => detected,
            Self::ObsidianBlack => HeadphoneFinish::ObsidianBlack,
            Self::PlatinumSilver => HeadphoneFinish::PlatinumSilver,
            Self::MidnightBlue => HeadphoneFinish::MidnightBlue,
            Self::SmokyPink => HeadphoneFinish::SmokyPink,
            Self::WarmWhite => HeadphoneFinish::WarmWhite,
            Self::ForestGray => HeadphoneFinish::ForestGray,
            Self::CrimsonRed => HeadphoneFinish::CrimsonRed,
            Self::ChampagneGold => HeadphoneFinish::ChampagneGold,
            Self::Violet => HeadphoneFinish::Violet,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeadphoneCapabilities {
    pub supports_single_battery: bool,
    pub supports_dual_battery: bool,
    pub supports_case_battery: bool,
    pub supports_power_off: bool,
    pub supports_auto_power_off: bool,
    pub auto_power_off_subtype: i32,
    pub supports_codec: bool,
    pub supports_equalizer: bool,
    pub supports_noise_control: bool,
    pub nc_control_subtype: i32,
    pub supports_nc_amb_button_cycle: bool,
    pub supports_connection_priority: bool,
    pub supports_dsee: bool,
    pub supports_wearing_detection: bool,
    pub supports_assignable_settings: bool,
    pub supports_voice_assistant: bool,
    pub supports_speak_to_chat: bool,
    pub supports_quick_access: bool,
    pub supports_touch_panel: bool,
    pub supports_multipoint: bool,
    pub supports_paired_devices: bool,
    pub supports_voice_guidance: bool,
}

impl HeadphoneCapabilities {
    pub fn from_sony(
        protocol_version: MdrProtocolVersion,
        t1_functions: &std::collections::HashSet<i32>,
        t2_functions: &std::collections::HashSet<i32>,
        has_table_2: bool,
        general_settings: &[GeneralSetting],
        paired_devices: &[PairedDevice],
        assignable_settings: &[AssignableSetting],
    ) -> Self {
        if protocol_version == MdrProtocolVersion::V1 {
            return Self::from_v1(t1_functions, assignable_settings);
        }
        let assume_default = t1_functions.is_empty();
        let nc_in_t1 = t1_functions
            .iter()
            .any(|&it| (0x61..=0x68).contains(&it) || (0x6B..=0x6F).contains(&it));
        let nc_subtype = if assume_default
            || t1_functions.contains(&0x68)
            || t1_functions.iter().any(|&it| (0x6B..=0x6D).contains(&it))
        {
            0x17
        } else {
            0x11
        };
        let apo_subtype = if t1_functions.contains(&0x26) && !t1_functions.contains(&0x25) {
            0x04
        } else {
            0x05
        };

        Self {
            supports_single_battery: assume_default || t1_functions.contains(&0x20),
            supports_dual_battery: t1_functions.contains(&0x21),
            supports_case_battery: t1_functions.contains(&0x22),
            supports_power_off: assume_default || t1_functions.contains(&0x23),
            supports_auto_power_off: assume_default
                || t1_functions.contains(&0x25)
                || t1_functions.contains(&0x26),
            auto_power_off_subtype: apo_subtype,
            supports_codec: assume_default || t1_functions.contains(&0x12),
            supports_equalizer: assume_default
                || t1_functions.contains(&0x50)
                || t1_functions.contains(&0x51),
            supports_noise_control: assume_default || nc_in_t1,
            nc_control_subtype: nc_subtype,
            supports_nc_amb_button_cycle: assume_default || t1_functions.contains(&0x69),
            supports_connection_priority: assume_default || t1_functions.contains(&0xE1),
            supports_dsee: assume_default || t1_functions.contains(&0xE2),
            supports_wearing_detection: assume_default || t1_functions.contains(&0xF1),
            supports_assignable_settings: t1_functions.contains(&0xF3)
                || !assignable_settings.is_empty(),
            supports_voice_assistant: assume_default || t1_functions.contains(&0xF4),
            supports_speak_to_chat: assume_default || t1_functions.contains(&0xFC),
            supports_quick_access: assume_default || t1_functions.contains(&0xFD),
            supports_touch_panel: general_settings
                .iter()
                .any(|g| g.key == "TOUCH_PANEL_SETTING")
                || assume_default,
            supports_multipoint: general_settings
                .iter()
                .any(|g| g.key == "MULTIPOINT_SETTING")
                || assume_default,
            supports_paired_devices: (has_table_2
                && (t2_functions.is_empty()
                    || t2_functions.contains(&0x31)
                    || t2_functions.contains(&0x32)
                    || t2_functions.contains(&0x33)))
                || !paired_devices.is_empty(),
            supports_voice_guidance: has_table_2
                && (t2_functions.is_empty() || t2_functions.contains(&0x41)),
        }
    }

    fn from_v1(
        t1_functions: &std::collections::HashSet<i32>,
        assignable_settings: &[AssignableSetting],
    ) -> Self {
        let assume_default = t1_functions.is_empty();
        let nc_in_t1 = t1_functions.contains(&0x31)
            || t1_functions.contains(&0x32)
            || t1_functions.contains(&0x33);
        let nc_subtype =
            if assume_default || t1_functions.contains(&0x32) || t1_functions.contains(&0x33) {
                0x02
            } else {
                0x01
            };
        Self {
            supports_single_battery: assume_default || t1_functions.contains(&0x21),
            supports_dual_battery: t1_functions.contains(&0x22),
            supports_case_battery: t1_functions.contains(&0x23),
            supports_power_off: assume_default || t1_functions.contains(&0x24),
            supports_auto_power_off: assume_default || t1_functions.contains(&0x25),
            auto_power_off_subtype: 0x01,
            supports_codec: assume_default || t1_functions.contains(&0x13),
            supports_equalizer: assume_default
                || t1_functions.contains(&0x51)
                || t1_functions.contains(&0x53),
            supports_noise_control: assume_default || nc_in_t1,
            nc_control_subtype: nc_subtype,
            supports_nc_amb_button_cycle: false,
            supports_connection_priority: assume_default || t1_functions.contains(&0x82),
            supports_dsee: assume_default || t1_functions.contains(&0x72),
            supports_wearing_detection: assume_default || t1_functions.contains(&0xD2),
            supports_assignable_settings: t1_functions.contains(&0xB1)
                || !assignable_settings.is_empty(),
            supports_voice_assistant: false,
            supports_speak_to_chat: false,
            supports_quick_access: false,
            supports_touch_panel: false,
            supports_multipoint: false,
            supports_paired_devices: false,
            supports_voice_guidance: assume_default || t1_functions.contains(&0xC1),
        }
    }

    pub fn bose_bmap() -> Self {
        Self {
            supports_single_battery: true,
            supports_dual_battery: false,
            supports_case_battery: false,
            supports_power_off: false,
            supports_auto_power_off: true,
            auto_power_off_subtype: 0,
            supports_codec: true,
            supports_equalizer: false,
            supports_noise_control: true,
            nc_control_subtype: 0,
            supports_nc_amb_button_cycle: false,
            supports_connection_priority: false,
            supports_dsee: false,
            supports_wearing_detection: false,
            supports_assignable_settings: false,
            supports_voice_assistant: false,
            supports_speak_to_chat: false,
            supports_quick_access: false,
            supports_touch_panel: false,
            supports_multipoint: true,
            supports_paired_devices: true,
            supports_voice_guidance: true,
        }
    }

    pub fn soundcore(has_dual_battery: bool) -> Self {
        Self {
            supports_single_battery: !has_dual_battery,
            supports_dual_battery: has_dual_battery,
            supports_case_battery: false,
            supports_power_off: false,
            supports_auto_power_off: false,
            auto_power_off_subtype: 0,
            supports_codec: true,
            supports_equalizer: true,
            supports_noise_control: true,
            nc_control_subtype: 0,
            supports_nc_amb_button_cycle: false,
            supports_connection_priority: false,
            supports_dsee: false,
            supports_wearing_detection: false,
            supports_assignable_settings: false,
            supports_voice_assistant: false,
            supports_speak_to_chat: false,
            supports_quick_access: false,
            supports_touch_panel: false,
            supports_multipoint: false,
            supports_paired_devices: false,
            supports_voice_guidance: false,
        }
    }

    pub fn galaxy_buds() -> Self {
        Self {
            supports_single_battery: false,
            supports_dual_battery: true,
            supports_case_battery: true,
            supports_power_off: false,
            supports_auto_power_off: false,
            auto_power_off_subtype: 0,
            supports_codec: true,
            supports_equalizer: true,
            supports_noise_control: true,
            nc_control_subtype: 0,
            supports_nc_amb_button_cycle: false,
            supports_connection_priority: false,
            supports_dsee: false,
            supports_wearing_detection: false,
            supports_assignable_settings: false,
            supports_voice_assistant: false,
            supports_speak_to_chat: false,
            supports_quick_access: false,
            supports_touch_panel: true,
            supports_multipoint: true,
            supports_paired_devices: false,
            supports_voice_guidance: false,
        }
    }

    pub fn airpods_aap(is_over_ear: bool) -> Self {
        Self {
            supports_single_battery: is_over_ear,
            supports_dual_battery: !is_over_ear,
            supports_case_battery: !is_over_ear,
            supports_power_off: false,
            supports_auto_power_off: false,
            auto_power_off_subtype: 0,
            supports_codec: true,
            supports_equalizer: false,
            supports_noise_control: true,
            nc_control_subtype: 0,
            supports_nc_amb_button_cycle: false,
            supports_connection_priority: false,
            supports_dsee: false,
            supports_wearing_detection: true,
            supports_assignable_settings: false,
            supports_voice_assistant: false,
            supports_speak_to_chat: true,
            supports_quick_access: false,
            supports_touch_panel: false,
            supports_multipoint: false,
            supports_paired_devices: false,
            supports_voice_guidance: false,
        }
    }

    pub fn airpods_ble_readonly(is_over_ear: bool) -> Self {
        Self {
            supports_single_battery: is_over_ear,
            supports_dual_battery: !is_over_ear,
            supports_case_battery: !is_over_ear,
            supports_power_off: false,
            supports_auto_power_off: false,
            auto_power_off_subtype: 0,
            supports_codec: true,
            supports_equalizer: false,
            supports_noise_control: false,
            nc_control_subtype: 0,
            supports_nc_amb_button_cycle: false,
            supports_connection_priority: false,
            supports_dsee: false,
            supports_wearing_detection: false,
            supports_assignable_settings: false,
            supports_voice_assistant: false,
            supports_speak_to_chat: false,
            supports_quick_access: false,
            supports_touch_panel: false,
            supports_multipoint: false,
            supports_paired_devices: false,
            supports_voice_guidance: false,
        }
    }

    pub fn generic_bluetooth() -> Self {
        Self {
            supports_single_battery: true,
            supports_dual_battery: false,
            supports_case_battery: false,
            supports_power_off: false,
            supports_auto_power_off: false,
            auto_power_off_subtype: 0,
            supports_codec: true,
            supports_equalizer: false,
            supports_noise_control: false,
            nc_control_subtype: 0,
            supports_nc_amb_button_cycle: false,
            supports_connection_priority: false,
            supports_dsee: false,
            supports_wearing_detection: false,
            supports_assignable_settings: false,
            supports_voice_assistant: false,
            supports_speak_to_chat: false,
            supports_quick_access: false,
            supports_touch_panel: false,
            supports_multipoint: false,
            supports_paired_devices: false,
            supports_voice_guidance: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeadphoneState {
    pub model_name: String,
    pub firmware_version: String,
    pub hardware_color_code: Option<i32>,
    pub detected_finish: HeadphoneFinish,
    pub bluetooth_address: String,
    pub protocol_version: MdrProtocolVersion,
    pub has_table_2: bool,
    pub t1_functions: std::collections::HashSet<i32>,
    pub t2_functions: std::collections::HashSet<i32>,

    pub battery: BatteryState,
    pub codec: AudioCodec,

    pub noise_control: NoiseControlState,
    pub nc_amb_button_cycle: NcAmbButtonCycle,

    pub equalizer: EqualizerState,
    pub dsee_extreme: bool,
    pub connection_priority: ConnectionPriority,

    pub speak_to_chat: bool,
    pub speak_to_chat_sensitivity: SpeakToChatSensitivity,
    pub speak_to_chat_timeout: SpeakToChatTimeout,

    pub wearing_detection: bool,
    pub auto_power_off: AutoPowerOff,
    pub auto_power_off_last_timer_byte: u8,

    pub voice_assistant: VoiceAssistant,
    pub voice_assistant_options: Vec<VoiceAssistant>,
    pub assignable_settings: Vec<AssignableSetting>,
    pub quick_access_double_tap: QuickAccessService,
    pub quick_access_triple_tap: QuickAccessService,
    pub quick_access_double_options: Vec<QuickAccessService>,
    pub quick_access_triple_options: Vec<QuickAccessService>,

    pub voice_guidance_enabled: bool,
    pub voice_guidance_language_code: i32,

    pub general_settings: Vec<GeneralSetting>,
    pub paired_devices: Vec<PairedDevice>,
    pub pairing_mode: bool,
    pub source_switch_enabled: bool,
    pub pending_alert: Option<HeadsetAlert>,
    pub custom_capabilities: Option<HeadphoneCapabilities>,
}

impl Default for HeadphoneState {
    fn default() -> Self {
        Self {
            model_name: "WH-1000XM5".to_string(),
            firmware_version: String::new(),
            hardware_color_code: None,
            detected_finish: HeadphoneFinish::ObsidianBlack,
            bluetooth_address: String::new(),
            protocol_version: MdrProtocolVersion::V2,
            has_table_2: false,
            t1_functions: std::collections::HashSet::new(),
            t2_functions: std::collections::HashSet::new(),
            battery: BatteryState::default(),
            codec: AudioCodec::Unsettled,
            noise_control: NoiseControlState::default(),
            nc_amb_button_cycle: NcAmbButtonCycle::NcAmbientOff,
            equalizer: EqualizerState::default(),
            dsee_extreme: false,
            connection_priority: ConnectionPriority::SoundQuality,
            speak_to_chat: false,
            speak_to_chat_sensitivity: SpeakToChatSensitivity::Auto,
            speak_to_chat_timeout: SpeakToChatTimeout::Standard15s,
            wearing_detection: true,
            auto_power_off: AutoPowerOff::WhenRemoved,
            auto_power_off_last_timer_byte: 0x00,
            voice_assistant: VoiceAssistant::DeviceDefault,
            voice_assistant_options: vec![
                VoiceAssistant::DeviceDefault,
                VoiceAssistant::GoogleAssistant,
                VoiceAssistant::Alexa,
                VoiceAssistant::Off,
            ],
            assignable_settings: Vec::new(),
            quick_access_double_tap: QuickAccessService(0x00),
            quick_access_triple_tap: QuickAccessService(0x00),
            quick_access_double_options: Vec::new(),
            quick_access_triple_options: Vec::new(),
            voice_guidance_enabled: true,
            voice_guidance_language_code: 0x01,
            general_settings: Vec::new(),
            paired_devices: Vec::new(),
            pairing_mode: false,
            source_switch_enabled: true,
            pending_alert: None,
            custom_capabilities: None,
        }
    }
}

impl HeadphoneState {
    pub fn touch_panel_enabled(&self) -> bool {
        self.general_settings
            .iter()
            .find(|g| g.key == "TOUCH_PANEL_SETTING")
            .map(|g| g.is_on)
            .unwrap_or(true)
    }

    pub fn multipoint_enabled(&self) -> bool {
        self.general_settings
            .iter()
            .find(|g| g.key == "MULTIPOINT_SETTING")
            .map(|g| g.is_on)
            .unwrap_or(true)
    }

    pub fn touch_panel_setting_id(&self) -> Option<i32> {
        self.general_settings
            .iter()
            .find(|g| g.key == "TOUCH_PANEL_SETTING")
            .map(|g| g.id)
    }

    pub fn multipoint_setting_id(&self) -> Option<i32> {
        self.general_settings
            .iter()
            .find(|g| g.key == "MULTIPOINT_SETTING")
            .map(|g| g.id)
    }

    pub fn capabilities(&self) -> HeadphoneCapabilities {
        if let Some(custom) = &self.custom_capabilities {
            return custom.clone();
        }
        HeadphoneCapabilities::from_sony(
            self.protocol_version,
            &self.t1_functions,
            &self.t2_functions,
            self.has_table_2,
            &self.general_settings,
            &self.paired_devices,
            &self.assignable_settings,
        )
    }

    pub fn device_profile(&self) -> crate::catalog::DeviceProfile {
        crate::catalog::resolve_device_profile(&self.model_name, self.battery.is_dual_battery())
    }

    pub fn form_factor(&self) -> crate::catalog::HeadphoneFormFactor {
        self.device_profile().form_factor
    }
}

// MARK: - App Preferences, Vault, Offline Queue, Battery History

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AppThemeMode {
    System,
    StudioDark,
    Light,
}

impl Default for AppThemeMode {
    fn default() -> Self {
        Self::System
    }
}

impl AppThemeMode {
    pub const ALL: [Self; 3] = [Self::System, Self::StudioDark, Self::Light];

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System (Windows)",
            Self::StudioDark => "Studio Dark",
            Self::Light => "Light",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HeadphoneAge {
    NewUnder1Y,
    Moderate1To2Y,
    Daily2To3Y,
    Heavy3YPlus,
}

impl Default for HeadphoneAge {
    fn default() -> Self {
        Self::NewUnder1Y
    }
}

impl HeadphoneAge {
    pub const ALL: [Self; 4] = [
        Self::NewUnder1Y,
        Self::Moderate1To2Y,
        Self::Daily2To3Y,
        Self::Heavy3YPlus,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::NewUnder1Y => "New (< 1 year)",
            Self::Moderate1To2Y => "Moderate (1–2 years)",
            Self::Daily2To3Y => "Daily Driver (2–3 years)",
            Self::Heavy3YPlus => "Well-Loved (3+ years)",
        }
    }

    pub fn capacity_multiplier(self) -> f64 {
        match self {
            Self::NewUnder1Y => 1.00,
            Self::Moderate1To2Y => 0.92,
            Self::Daily2To3Y => 0.85,
            Self::Heavy3YPlus => 0.78,
        }
    }

    pub fn subtitle_for(self, baseline: &crate::catalog::BatteryBaseline) -> String {
        let mult = self.capacity_multiplier();
        let anc_h = (baseline.anc_hours * mult).round().max(1.0) as i32;
        let off_h = (baseline.off_hours * mult).round().max(1.0) as i32;
        format!("~{}h ANC · ~{}h Off", anc_h, off_h)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomEqProfile {
    pub id: String,
    pub name: String,
    pub clear_bass: i32,
    pub bands: Vec<i32>,
}

impl CustomEqProfile {
    pub fn default_profiles() -> Vec<Self> {
        vec![
            Self {
                id: "sony_balanced".into(),
                name: "Balanced Clarity".into(),
                clear_bass: -1,
                bands: vec![-3, 0, 2, 3, 2],
            },
            Self {
                id: "warm_studio".into(),
                name: "Warm Studio".into(),
                clear_bass: 2,
                bands: vec![-1, 1, 2, 1, 1],
            },
            Self {
                id: "podcast_vocal".into(),
                name: "Podcast & Vocal".into(),
                clear_bass: -4,
                bands: vec![-3, 2, 4, 3, 0],
            },
            Self {
                id: "sub_bass".into(),
                name: "Sub-Bass Punch".into(),
                clear_bass: 6,
                bands: vec![-2, 0, 1, 2, 3],
            },
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmbientScene {
    pub id: String,
    pub name: String,
    pub level: i32,
    pub focus_on_voice: bool,
}

impl AmbientScene {
    pub fn defaults() -> Vec<Self> {
        vec![
            Self {
                id: "street".into(),
                name: "Street 10".into(),
                level: 10,
                focus_on_voice: false,
            },
            Self {
                id: "office".into(),
                name: "Office 14".into(),
                level: 14,
                focus_on_voice: false,
            },
            Self {
                id: "convo".into(),
                name: "Conversation 20".into(),
                level: 20,
                focus_on_voice: true,
            },
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsVaultSnapshot {
    pub saved_at_ms: i64,
    pub noise_mode: NoiseControlMode,
    pub ambient_level: i32,
    pub focus_on_voice: bool,
    pub nc_amb_button_cycle_wire: i32,
    pub eq_preset_code: i32,
    pub eq_clear_bass: i32,
    pub eq_bands: Vec<i32>,
    pub dsee_extreme: bool,
    pub connection_priority_wire: i32,
    pub speak_to_chat: bool,
    pub speak_to_chat_sensitivity_wire: i32,
    pub speak_to_chat_timeout_wire: i32,
    pub wearing_detection: bool,
    pub touch_panel_enabled: bool,
    pub auto_power_off_wire: i32,
    pub voice_assistant_wire: i32,
    pub quick_access_double_wire: i32,
    pub quick_access_triple_wire: i32,
    pub multipoint_enabled: bool,
    pub voice_guidance_enabled: bool,
}

impl SettingsVaultSnapshot {
    pub fn from_state(state: &HeadphoneState, now_ms: i64) -> Self {
        Self {
            saved_at_ms: now_ms,
            noise_mode: state.noise_control.mode,
            ambient_level: state.noise_control.ambient_level,
            focus_on_voice: state.noise_control.focus_on_voice,
            nc_amb_button_cycle_wire: state.nc_amb_button_cycle.wire_value() as i32,
            eq_preset_code: state.equalizer.preset_code,
            eq_clear_bass: state.equalizer.clear_bass,
            eq_bands: state.equalizer.bands.clone(),
            dsee_extreme: state.dsee_extreme,
            connection_priority_wire: state.connection_priority.wire_byte() as i32,
            speak_to_chat: state.speak_to_chat,
            speak_to_chat_sensitivity_wire: state.speak_to_chat_sensitivity.wire_byte() as i32,
            speak_to_chat_timeout_wire: state.speak_to_chat_timeout.wire_byte() as i32,
            wearing_detection: state.wearing_detection,
            touch_panel_enabled: state.touch_panel_enabled(),
            auto_power_off_wire: state.auto_power_off.wire_byte() as i32,
            voice_assistant_wire: state.voice_assistant.wire_byte() as i32,
            quick_access_double_wire: state.quick_access_double_tap.0 as i32,
            quick_access_triple_wire: state.quick_access_triple_tap.0 as i32,
            multipoint_enabled: state.multipoint_enabled(),
            voice_guidance_enabled: state.voice_guidance_enabled,
        }
    }

    pub fn diffs_from(&self, state: &HeadphoneState) -> Vec<String> {
        let caps = state.capabilities();
        let mut diffs = Vec::new();
        if caps.supports_noise_control && state.noise_control.mode != self.noise_mode {
            diffs.push("Noise Control".to_string());
        }
        if caps.supports_equalizer
            && (state.equalizer.preset_code != self.eq_preset_code
                || (self.eq_preset_code == 0xA0
                    && (state.equalizer.clear_bass != self.eq_clear_bass
                        || state.equalizer.bands != self.eq_bands)))
        {
            diffs.push("Equalizer".to_string());
        }
        if caps.supports_dsee && state.dsee_extreme != self.dsee_extreme {
            diffs.push("DSEE Extreme".to_string());
        }
        if caps.supports_speak_to_chat && state.speak_to_chat != self.speak_to_chat {
            diffs.push("Speak-to-Chat".to_string());
        }
        if caps.supports_wearing_detection && state.wearing_detection != self.wearing_detection {
            diffs.push("Wearing Detection".to_string());
        }
        if caps.supports_touch_panel && state.touch_panel_enabled() != self.touch_panel_enabled {
            diffs.push("Touch Panel".to_string());
        }
        if caps.supports_auto_power_off
            && (state.auto_power_off.wire_byte() as i32) != self.auto_power_off_wire
        {
            diffs.push("Auto Power-Off".to_string());
        }
        if caps.supports_nc_amb_button_cycle
            && (state.nc_amb_button_cycle.wire_value() as i32) != self.nc_amb_button_cycle_wire
        {
            diffs.push("NC/AMB Button".to_string());
        }
        if caps.supports_connection_priority
            && (state.connection_priority.wire_byte() as i32) != self.connection_priority_wire
        {
            diffs.push("Connection Priority".to_string());
        }
        if caps.supports_multipoint && state.multipoint_enabled() != self.multipoint_enabled {
            diffs.push("Multipoint".to_string());
        }
        diffs
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedHeadphoneSnapshot {
    pub last_seen_ms: i64,
    pub model_name: String,
    pub firmware_version: String,
    pub battery_level: Option<i32>,
    pub is_charging: bool,
    pub codec_label: String,
    pub noise_control: NoiseControlState,
    pub equalizer: EqualizerState,
    pub dsee_extreme: bool,
    pub speak_to_chat: bool,
    pub wearing_detection: bool,
    pub touch_panel_enabled: bool,
    pub multipoint_enabled: bool,
    pub detected_finish: HeadphoneFinish,
    pub bluetooth_address: String,
    pub protocol_version: MdrProtocolVersion,
    pub has_table_2: bool,
    pub left_battery: Option<BatteryUnit>,
    pub right_battery: Option<BatteryUnit>,
    pub case_battery: Option<BatteryUnit>,
}

impl CachedHeadphoneSnapshot {
    pub fn from_state(state: &HeadphoneState, now_ms: i64) -> Self {
        Self {
            last_seen_ms: now_ms,
            model_name: state.model_name.clone(),
            firmware_version: state.firmware_version.clone(),
            battery_level: state.battery.effective_level(),
            is_charging: state.battery.is_charging(),
            codec_label: state.codec.label().to_string(),
            noise_control: state.noise_control.clone(),
            equalizer: state.equalizer.clone(),
            dsee_extreme: state.dsee_extreme,
            speak_to_chat: state.speak_to_chat,
            wearing_detection: state.wearing_detection,
            touch_panel_enabled: state.touch_panel_enabled(),
            multipoint_enabled: state.multipoint_enabled(),
            detected_finish: state.detected_finish,
            bluetooth_address: state.bluetooth_address.clone(),
            protocol_version: state.protocol_version,
            has_table_2: state.has_table_2,
            left_battery: state.battery.left.clone(),
            right_battery: state.battery.right.clone(),
            case_battery: state.battery.case_battery.clone(),
        }
    }

    pub fn to_headphone_state(&self) -> HeadphoneState {
        let mut st = HeadphoneState {
            model_name: self.model_name.clone(),
            firmware_version: self.firmware_version.clone(),
            bluetooth_address: self.bluetooth_address.clone(),
            protocol_version: self.protocol_version,
            has_table_2: self.has_table_2,
            detected_finish: self.detected_finish,
            battery: BatteryState {
                level: self.battery_level,
                charging_status: if self.is_charging {
                    ChargingStatus::Charging
                } else {
                    ChargingStatus::NotCharging
                },
                left: self.left_battery.clone(),
                right: self.right_battery.clone(),
                case_battery: self.case_battery.clone(),
            },
            noise_control: self.noise_control.clone(),
            equalizer: self.equalizer.clone(),
            dsee_extreme: self.dsee_extreme,
            speak_to_chat: self.speak_to_chat,
            wearing_detection: self.wearing_detection,
            general_settings: vec![
                GeneralSetting {
                    id: 0xD1,
                    key: "TOUCH_PANEL_SETTING".to_string(),
                    summary_key: String::new(),
                    is_on: self.touch_panel_enabled,
                },
                GeneralSetting {
                    id: 0xD2,
                    key: "MULTIPOINT_SETTING".to_string(),
                    summary_key: String::new(),
                    is_on: self.multipoint_enabled,
                },
            ],
            ..Default::default()
        };
        st.custom_capabilities = match self.protocol_version {
            MdrProtocolVersion::BoseBmap => Some(HeadphoneCapabilities::bose_bmap()),
            MdrProtocolVersion::Soundcore => Some(HeadphoneCapabilities::soundcore(
                st.battery.is_dual_battery(),
            )),
            MdrProtocolVersion::GalaxyBuds => Some(HeadphoneCapabilities::galaxy_buds()),
            MdrProtocolVersion::AirPodsAap => Some(HeadphoneCapabilities::airpods_aap(
                st.form_factor() == crate::catalog::HeadphoneFormFactor::OverEar,
            )),
            MdrProtocolVersion::GenericBt => Some(HeadphoneCapabilities::generic_bluetooth()),
            MdrProtocolVersion::V1 | MdrProtocolVersion::V2 => None,
        };
        st
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingOfflineChanges {
    pub noise_control: Option<NoiseControlState>,
    pub eq_preset_code: Option<i32>,
    pub eq_clear_bass: Option<i32>,
    pub eq_bands: Option<Vec<i32>>,
    pub dsee_extreme: Option<bool>,
    pub connection_priority_wire: Option<i32>,
    pub speak_to_chat: Option<bool>,
    pub speak_to_chat_sensitivity_wire: Option<i32>,
    pub speak_to_chat_timeout_wire: Option<i32>,
    pub wearing_detection: Option<bool>,
    pub touch_panel_enabled: Option<bool>,
    pub auto_power_off_wire: Option<i32>,
    pub nc_amb_button_cycle_wire: Option<i32>,
    pub voice_assistant_wire: Option<i32>,
    pub quick_access_double_wire: Option<i32>,
    pub quick_access_triple_wire: Option<i32>,
    pub multipoint_enabled: Option<bool>,
    pub voice_guidance_enabled: Option<bool>,
}

impl PendingOfflineChanges {
    pub fn pending_labels(&self) -> Vec<&'static str> {
        let mut labels = Vec::new();
        if self.noise_control.is_some() {
            labels.push("Noise Control");
        }
        if self.eq_preset_code.is_some() || self.eq_clear_bass.is_some() || self.eq_bands.is_some()
        {
            labels.push("Equalizer");
        }
        if self.dsee_extreme.is_some() {
            labels.push("DSEE Extreme");
        }
        if self.connection_priority_wire.is_some() {
            labels.push("Connection Priority");
        }
        if self.speak_to_chat.is_some()
            || self.speak_to_chat_sensitivity_wire.is_some()
            || self.speak_to_chat_timeout_wire.is_some()
        {
            labels.push("Speak-to-Chat");
        }
        if self.wearing_detection.is_some() {
            labels.push("Wear Detection");
        }
        if self.touch_panel_enabled.is_some() {
            labels.push("Touch Panel");
        }
        if self.auto_power_off_wire.is_some() {
            labels.push("Auto Power-Off");
        }
        if self.nc_amb_button_cycle_wire.is_some() {
            labels.push("NC/AMB Button");
        }
        if self.voice_assistant_wire.is_some() {
            labels.push("Voice Assistant");
        }
        if self.quick_access_double_wire.is_some() || self.quick_access_triple_wire.is_some() {
            labels.push("Quick Access");
        }
        if self.multipoint_enabled.is_some() {
            labels.push("Multipoint");
        }
        if self.voice_guidance_enabled.is_some() {
            labels.push("Voice Guidance");
        }
        labels
    }

    pub fn pending_count(&self) -> usize {
        self.pending_labels().len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending_count() == 0
    }

    pub fn apply_overlay(&self, base: &HeadphoneState) -> HeadphoneState {
        if self.is_empty() {
            return base.clone();
        }
        let mut next = base.clone();
        let set_gs = |settings: &mut Vec<GeneralSetting>, key: &str, default_id: i32, on: bool| {
            if let Some(item) = settings.iter_mut().find(|g| g.key == key) {
                item.is_on = on;
            } else {
                settings.push(GeneralSetting {
                    id: default_id,
                    key: key.to_string(),
                    summary_key: String::new(),
                    is_on: on,
                });
            }
        };
        if let Some(tp) = self.touch_panel_enabled {
            set_gs(&mut next.general_settings, "TOUCH_PANEL_SETTING", 0xD1, tp);
        }
        if let Some(mp) = self.multipoint_enabled {
            set_gs(&mut next.general_settings, "MULTIPOINT_SETTING", 0xD2, mp);
        }
        if let Some(nc) = &self.noise_control {
            next.noise_control = nc.clone();
        }
        if let Some(code) = self.eq_preset_code {
            next.equalizer.preset_code = code;
        }
        if let Some(cb) = self.eq_clear_bass {
            next.equalizer.clear_bass = cb;
        }
        if let Some(bands) = &self.eq_bands {
            next.equalizer.bands = bands.clone();
        }
        if let Some(dsee) = self.dsee_extreme {
            next.dsee_extreme = dsee;
        }
        if let Some(w) = self.connection_priority_wire {
            next.connection_priority = ConnectionPriority::from_byte((w & 0xFF) as u8);
        }
        if let Some(stc) = self.speak_to_chat {
            next.speak_to_chat = stc;
        }
        if let Some(w) = self.speak_to_chat_sensitivity_wire {
            next.speak_to_chat_sensitivity = SpeakToChatSensitivity::from_byte((w & 0xFF) as u8);
        }
        if let Some(w) = self.speak_to_chat_timeout_wire {
            next.speak_to_chat_timeout = SpeakToChatTimeout::from_byte((w & 0xFF) as u8);
        }
        if let Some(wd) = self.wearing_detection {
            next.wearing_detection = wd;
        }
        if let Some(w) = self.auto_power_off_wire {
            if let Some(mode) = AutoPowerOff::from_byte((w & 0xFF) as u8) {
                next.auto_power_off = mode;
            }
        }
        if let Some(w) = self.nc_amb_button_cycle_wire {
            if let Some(cycle) = NcAmbButtonCycle::from_byte((w & 0xFF) as u8) {
                next.nc_amb_button_cycle = cycle;
            }
        }
        if let Some(w) = self.voice_assistant_wire {
            next.voice_assistant = VoiceAssistant::from_byte((w & 0xFF) as u8);
        }
        if let Some(w) = self.quick_access_double_wire {
            next.quick_access_double_tap = QuickAccessService((w & 0xFF) as u8);
        }
        if let Some(w) = self.quick_access_triple_wire {
            next.quick_access_triple_tap = QuickAccessService((w & 0xFF) as u8);
        }
        if let Some(vg) = self.voice_guidance_enabled {
            next.voice_guidance_enabled = vg;
        }
        next
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatterySample {
    pub timestamp_ms: i64,
    pub level: i32,
    pub is_charging: bool,
    pub nc_active: bool,
    pub high_power_dsp: bool,
    pub device_address: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BatteryProjection {
    pub remaining_minutes: Option<i32>,
    pub estimated_empty_at_ms: Option<i64>,
    pub drain_rate_per_hour: f64,
    pub is_calibrated: bool,
    pub valid_transitions: i32,
    pub calibration_progress: f32,
}

impl Default for BatteryProjection {
    fn default() -> Self {
        Self {
            remaining_minutes: None,
            estimated_empty_at_ms: None,
            drain_rate_per_hour: 3.33,
            is_calibrated: false,
            valid_transitions: 0,
            calibration_progress: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionStatus {
    Offline,
    Connecting,
    Connected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FallbackNotice {
    pub title: String,
    pub reason: String,
    pub intended_protocol: MdrProtocolVersion,
    pub can_retry_control_channel: bool,
}

impl FallbackNotice {
    pub fn for_fallback(
        device_name: &str,
        intended_protocol: MdrProtocolVersion,
        failure_detail: Option<&str>,
    ) -> Self {
        let detail = failure_detail
            .filter(|s| !s.trim().is_empty())
            .map(|s| format!(" ({})", s.trim()))
            .unwrap_or_default();

        match intended_protocol {
            MdrProtocolVersion::AirPodsAap => Self {
                title: "Apple / Beats · BLE Battery Mode".to_string(),
                reason: format!(
                    "Reading live Left / Right / Case battery and charging status over Windows BLE{}. Active Noise Control requires Classic L2CAP (PSM 0x1001), which Windows restricts to kernel drivers.",
                    detail
                ),
                intended_protocol,
                can_retry_control_channel: false,
            },
            MdrProtocolVersion::BoseBmap
            | MdrProtocolVersion::Soundcore
            | MdrProtocolVersion::GalaxyBuds => Self {
                title: "Fallback Mode · Read-Only Telemetry".to_string(),
                reason: format!(
                    "Could not open the {} control channel on {}{}. Make sure the headphones are powered on and connected to Windows, then click Retry.",
                    intended_protocol.label(),
                    device_name,
                    detail
                ),
                intended_protocol,
                can_retry_control_channel: true,
            },
            MdrProtocolVersion::V1 | MdrProtocolVersion::V2 => Self {
                title: "Fallback Mode · Read-Only Telemetry".to_string(),
                reason: format!(
                    "Sony control channel failed on {}{}. Showing cached/standard status only.",
                    device_name, detail
                ),
                intended_protocol,
                can_retry_control_channel: true,
            },
            MdrProtocolVersion::GenericBt => Self {
                title: "Standard Bluetooth Mode · Read-Only".to_string(),
                reason: format!(
                    "{} has no proprietary control driver in Murmur (such as ANC or EQ). Showing standard connection status.",
                    device_name
                ),
                intended_protocol,
                can_retry_control_channel: false,
            },
        }
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SimulatedProfile {
    Wh1000Xm5,
    Wh1000Xm4,
    WhCh720n,
    Wf1000Xm5,
    Wh1000Xm3,
    Wf1000Xm3,
    BoseQcUltra,
    SoundcoreQ45,
    GalaxyBuds2Pro,
}

#[cfg(test)]
impl Default for SimulatedProfile {
    fn default() -> Self {
        Self::Wh1000Xm5
    }
}

#[cfg(test)]
impl SimulatedProfile {
    pub const ALL: [Self; 9] = [
        Self::Wh1000Xm5,
        Self::Wh1000Xm4,
        Self::WhCh720n,
        Self::Wf1000Xm5,
        Self::Wh1000Xm3,
        Self::Wf1000Xm3,
        Self::BoseQcUltra,
        Self::SoundcoreQ45,
        Self::GalaxyBuds2Pro,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Wh1000Xm5 => "WH-1000XM5",
            Self::Wh1000Xm4 => "WH-1000XM4",
            Self::WhCh720n => "WH-CH720N",
            Self::Wf1000Xm5 => "WF-1000XM5",
            Self::Wh1000Xm3 => "WH-1000XM3 (V1)",
            Self::Wf1000Xm3 => "WF-1000XM3 (V1)",
            Self::BoseQcUltra => "Bose QC Ultra",
            Self::SoundcoreQ45 => "Soundcore Q45",
            Self::GalaxyBuds2Pro => "Galaxy Buds2 Pro",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vault_snapshot_camel_case_roundtrip_and_diffs() {
        let mut hp = HeadphoneState::default();
        hp.noise_control.mode = NoiseControlMode::NoiseCancelling;
        hp.dsee_extreme = false;

        let snap = SettingsVaultSnapshot::from_state(&hp, 12345);
        let json = serde_json::to_string(&snap).unwrap();
        assert!(json.contains("\"savedAtMs\":12345"));
        assert!(json.contains("\"noiseMode\":\"NOISE_CANCELLING\""));
        let decoded: SettingsVaultSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, snap);

        hp.noise_control.mode = NoiseControlMode::AmbientSound;
        hp.dsee_extreme = true;
        let diffs = snap.diffs_from(&hp);
        assert!(diffs.contains(&"Noise Control".to_string()));
        assert!(diffs.contains(&"DSEE Extreme".to_string()));
    }

    #[test]
    fn pending_offline_changes_overlay() {
        let base = HeadphoneState::default();
        let pending = PendingOfflineChanges {
            noise_control: Some(NoiseControlState {
                mode: NoiseControlMode::AmbientSound,
                ambient_level: 16,
                focus_on_voice: true,
            }),
            dsee_extreme: Some(true),
            touch_panel_enabled: Some(false),
            ..Default::default()
        };
        assert_eq!(pending.pending_count(), 3);
        let overlaid = pending.apply_overlay(&base);
        assert_eq!(overlaid.noise_control.mode, NoiseControlMode::AmbientSound);
        assert_eq!(overlaid.noise_control.ambient_level, 16);
        assert!(overlaid.noise_control.focus_on_voice);
        assert!(overlaid.dsee_extreme);
        assert!(!overlaid.touch_panel_enabled());
    }
}
