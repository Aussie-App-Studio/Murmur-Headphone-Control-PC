use crate::models::{
    AssignablePreset, AssignableSetting, AudioCodec, AutoPowerOff, BatteryUnit, ChargingStatus,
    ConnectionPriority, HeadphoneFinish, HeadphoneState, MdrProtocolVersion, NoiseControlMode,
    NoiseControlState,
};

pub const SERVICE_UUID_V1: &str = "96CC203E-5068-46AD-B32D-E316F5E069BA";

const V1_CONNECT_GET_PROTOCOL: u8 = 0x00;
const V1_CONNECT_RET_PROTOCOL: u8 = 0x01;
const V1_CONNECT_GET_DEVICE_INFO: u8 = 0x04;
const V1_CONNECT_RET_DEVICE_INFO: u8 = 0x05;
const V1_CONNECT_GET_SUPPORT: u8 = 0x06;
const V1_CONNECT_RET_SUPPORT: u8 = 0x07;

const V1_COMMON_GET_BATTERY: u8 = 0x10;
const V1_COMMON_RET_BATTERY: u8 = 0x11;
const V1_COMMON_NTFY_BATTERY: u8 = 0x12;
const V1_COMMON_GET_AUDIO_CODEC: u8 = 0x14;
const V1_COMMON_RET_AUDIO_CODEC: u8 = 0x15;
const V1_COMMON_NTFY_AUDIO_CODEC: u8 = 0x16;
const V1_COMMON_SET_POWER_OFF: u8 = 0x22;

const V1_EQ_GET_CAPABILITY: u8 = 0x50;
const V1_EQ_RET_CAPABILITY: u8 = 0x51;
const V1_EQ_GET_PARAM: u8 = 0x56;
const V1_EQ_RET_PARAM: u8 = 0x57;
const V1_EQ_SET_PARAM: u8 = 0x58;
const V1_EQ_NTFY_PARAM: u8 = 0x59;

const V1_NC_GET_PARAM: u8 = 0x66;
const V1_NC_RET_PARAM: u8 = 0x67;
const V1_NC_SET_PARAM: u8 = 0x68;
const V1_NC_NTFY_PARAM: u8 = 0x69;

const V1_AUDIO_GET_PARAM: u8 = 0x76;
const V1_AUDIO_RET_PARAM: u8 = 0x77;
const V1_AUDIO_SET_PARAM: u8 = 0x78;
const V1_AUDIO_NTFY_PARAM: u8 = 0x79;

const V1_SYS_GET_CAPABILITY: u8 = 0xA0;
const V1_SYS_RET_CAPABILITY: u8 = 0xA1;
const V1_SYS_GET_PARAM: u8 = 0xA6;
const V1_SYS_RET_PARAM: u8 = 0xA7;
const V1_SYS_SET_PARAM: u8 = 0xA8;
const V1_SYS_NTFY_PARAM: u8 = 0xA9;

const V1_VG_GET_PARAM: u8 = 0xB6;
const V1_VG_RET_PARAM: u8 = 0xB7;
const V1_VG_SET_PARAM: u8 = 0xB8;
const V1_VG_NTFY_PARAM: u8 = 0xB9;

// MARK: - Query Builders

pub fn get_protocol_info() -> Vec<u8> {
    vec![V1_CONNECT_GET_PROTOCOL, 0x00]
}
pub fn get_model_name() -> Vec<u8> {
    vec![V1_CONNECT_GET_DEVICE_INFO, 0x01]
}
pub fn get_firmware_version() -> Vec<u8> {
    vec![V1_CONNECT_GET_DEVICE_INFO, 0x02]
}
pub fn get_series_and_color_info() -> Vec<u8> {
    vec![V1_CONNECT_GET_DEVICE_INFO, 0x03]
}
pub fn get_support_functions() -> Vec<u8> {
    vec![V1_CONNECT_GET_SUPPORT, 0x00]
}
pub fn get_battery() -> Vec<u8> {
    vec![V1_COMMON_GET_BATTERY, 0x00]
}
pub fn get_left_right_battery() -> Vec<u8> {
    vec![V1_COMMON_GET_BATTERY, 0x01]
}
pub fn get_cradle_battery() -> Vec<u8> {
    vec![V1_COMMON_GET_BATTERY, 0x02]
}
pub fn get_codec() -> Vec<u8> {
    vec![V1_COMMON_GET_AUDIO_CODEC, 0x00]
}
pub fn get_noise_control(subtype: i32) -> Vec<u8> {
    vec![V1_NC_GET_PARAM, subtype as u8]
}
pub fn get_eq_capability() -> Vec<u8> {
    vec![V1_EQ_GET_CAPABILITY, 0x01, 0x01]
}
pub fn get_eq() -> Vec<u8> {
    vec![V1_EQ_GET_PARAM, 0x01]
}
pub fn get_connection_priority() -> Vec<u8> {
    vec![V1_AUDIO_GET_PARAM, 0x01]
}
pub fn get_dsee() -> Vec<u8> {
    vec![V1_AUDIO_GET_PARAM, 0x02]
}
pub fn get_auto_power_off() -> Vec<u8> {
    vec![V1_SYS_GET_PARAM, 0x01]
}
pub fn get_wearing_detection() -> Vec<u8> {
    vec![V1_SYS_GET_PARAM, 0x02]
}
pub fn get_assignable_settings_capability() -> Vec<u8> {
    vec![V1_SYS_GET_CAPABILITY, 0x03]
}
pub fn get_assignable_settings() -> Vec<u8> {
    vec![V1_SYS_GET_PARAM, 0x03]
}
pub fn get_voice_guidance() -> Vec<u8> {
    vec![V1_VG_GET_PARAM, 0x01]
}

// MARK: - Setter Builders

pub fn set_noise_control(state: &NoiseControlState, subtype: i32) -> Vec<u8> {
    let effect_byte = if state.mode == NoiseControlMode::Off {
        0x00
    } else {
        0x01
    };
    if subtype == 0x01 {
        let nc_on = if state.mode == NoiseControlMode::NoiseCancelling {
            0x01
        } else {
            0x00
        };
        return vec![V1_NC_SET_PARAM, 0x01, effect_byte, nc_on];
    }
    let nc_value = if state.mode == NoiseControlMode::NoiseCancelling {
        0x02
    } else {
        0x00
    };
    let voice_byte = if state.focus_on_voice { 0x01 } else { 0x00 };
    let asm_amount = if state.mode == NoiseControlMode::AmbientSound {
        state.ambient_level.clamp(1, 20) as u8
    } else {
        0x00
    };
    vec![
        V1_NC_SET_PARAM,
        0x02,
        effect_byte,
        0x02,
        nc_value,
        0x01,
        voice_byte,
        asm_amount,
    ]
}

pub fn set_eq_preset(preset_code: i32) -> Vec<u8> {
    vec![V1_EQ_SET_PARAM, 0x01, preset_code as u8, 0x00]
}

pub fn set_eq_custom(
    clear_bass: i32,
    bands: &[i32],
    preset_code: i32,
    step_count: i32,
    has_clear_bass: bool,
) -> Vec<u8> {
    let zero_offset = step_count / 2;
    let max_gain = zero_offset.max(1);
    let mut values = Vec::with_capacity(bands.len() + 1);
    if has_clear_bass {
        values.push(clear_bass);
    }
    values.extend_from_slice(bands);
    let mut out = Vec::with_capacity(4 + values.len());
    out.push(V1_EQ_SET_PARAM);
    out.push(0x01);
    out.push(preset_code as u8);
    out.push(values.len() as u8);
    for v in values {
        out.push((v.clamp(-max_gain, max_gain) + zero_offset) as u8);
    }
    out
}

pub fn set_connection_priority(priority: ConnectionPriority) -> Vec<u8> {
    vec![V1_AUDIO_SET_PARAM, 0x01, priority.wire_byte()]
}

pub fn set_dsee(enabled: bool) -> Vec<u8> {
    vec![V1_AUDIO_SET_PARAM, 0x02, if enabled { 0x01 } else { 0x00 }]
}

pub fn set_auto_power_off(mode: AutoPowerOff, last_timer_byte: u8) -> Vec<u8> {
    vec![V1_SYS_SET_PARAM, 0x01, mode.wire_byte(), last_timer_byte]
}

pub fn set_wearing_detection(enabled: bool) -> Vec<u8> {
    vec![V1_SYS_SET_PARAM, 0x02, if enabled { 0x00 } else { 0x01 }]
}

pub fn set_assignable_settings(presets: &[AssignablePreset]) -> Vec<u8> {
    let mut out = Vec::with_capacity(3 + presets.len());
    out.push(V1_SYS_SET_PARAM);
    out.push(0x03);
    out.push(presets.len() as u8);
    for p in presets {
        out.push(p.wire_byte());
    }
    out
}

pub fn set_voice_guidance(enabled: bool, language_code: i32) -> Vec<u8> {
    vec![
        V1_VG_SET_PARAM,
        0x01,
        if enabled { 0x00 } else { 0x01 },
        language_code as u8,
    ]
}

pub fn power_off_headphones() -> Vec<u8> {
    vec![V1_COMMON_SET_POWER_OFF, 0x00, 0x01]
}

// MARK: - Parser

pub fn apply_table1_message(state: &HeadphoneState, payload: &[u8]) -> HeadphoneState {
    if payload.len() < 2 {
        return state.clone();
    }
    let op = payload[0];
    let sub = payload[1];
    let mut r = V1ByteCursor::new(payload, 2);
    let mut next = state.clone();

    match (op, sub) {
        (V1_CONNECT_RET_PROTOCOL, _) => {
            next.protocol_version = MdrProtocolVersion::V1;
            next.has_table_2 = false;
        }
        (V1_CONNECT_RET_DEVICE_INFO, 0x01) => {
            if let Some(name) = r.lp_string() {
                next.model_name = name;
            }
        }
        (V1_CONNECT_RET_DEVICE_INFO, 0x02) => {
            if let Some(fw) = r.lp_string() {
                next.firmware_version = fw;
            }
        }
        (V1_CONNECT_RET_DEVICE_INFO, 0x03) => {
            if r.u8().is_some() {
                if let Some(color) = r.u8() {
                    next.hardware_color_code = Some(color as i32);
                    next.detected_finish = HeadphoneFinish::from_sony_color_byte(color as i32);
                }
            }
        }
        (V1_CONNECT_RET_SUPPORT, _) => {
            if let Some(count) = r.u8() {
                let mut funcs = std::collections::HashSet::with_capacity(count as usize);
                for _ in 0..count {
                    if let Some(fn_id) = r.u8() {
                        funcs.insert(fn_id as i32);
                    }
                }
                next.t1_functions = funcs;
            }
        }
        (V1_COMMON_RET_BATTERY | V1_COMMON_NTFY_BATTERY, 0x00) => {
            if let Some(lvl) = r.u8() {
                let chg = r.u8().unwrap_or(0x00);
                next.battery.level = Some((lvl as i32).clamp(0, 100));
                next.battery.charging_status = ChargingStatus::from_byte(chg);
            }
        }
        (V1_COMMON_RET_BATTERY | V1_COMMON_NTFY_BATTERY, 0x01) => {
            if let (Some(l_lvl), Some(l_chg), Some(r_lvl), Some(r_chg)) = (
                r.u8(),
                Some(r.u8().unwrap_or(0)),
                r.u8(),
                Some(r.u8().unwrap_or(0)),
            ) {
                let left_unit = if l_lvl <= 100 {
                    Some(BatteryUnit::new(l_lvl as i32, ChargingStatus::from_byte(l_chg)))
                } else {
                    None
                };
                let right_unit = if r_lvl <= 100 {
                    Some(BatteryUnit::new(r_lvl as i32, ChargingStatus::from_byte(r_chg)))
                } else {
                    None
                };
                next.battery.left = left_unit.clone();
                next.battery.right = right_unit.clone();
                next.battery.level = next.battery.effective_level();
                next.battery.charging_status = if left_unit.as_ref().is_some_and(|u| u.is_charging())
                    || right_unit.as_ref().is_some_and(|u| u.is_charging())
                {
                    ChargingStatus::Charging
                } else if left_unit
                    .as_ref()
                    .is_some_and(|u| u.charging_status == ChargingStatus::Charged)
                    && right_unit
                        .as_ref()
                        .is_some_and(|u| u.charging_status == ChargingStatus::Charged)
                {
                    ChargingStatus::Charged
                } else {
                    ChargingStatus::NotCharging
                };
            }
        }
        (V1_COMMON_RET_BATTERY | V1_COMMON_NTFY_BATTERY, 0x02) => {
            if let Some(case_lvl) = r.u8() {
                let case_chg = r.u8().unwrap_or(0);
                next.battery.case_battery = if case_lvl <= 100 {
                    Some(BatteryUnit::new(
                        case_lvl as i32,
                        ChargingStatus::from_byte(case_chg),
                    ))
                } else {
                    None
                };
            }
        }
        (V1_COMMON_RET_AUDIO_CODEC | V1_COMMON_NTFY_AUDIO_CODEC, 0x00) => {
            if let Some(code) = r.u8() {
                next.codec = AudioCodec::from_byte(code);
            }
        }
        (V1_EQ_RET_CAPABILITY, 0x01) => {
            if r.remaining() >= 3 {
                if let (Some(band_raw), Some(step_raw), Some(preset_count)) =
                    (r.u8(), r.u8(), r.u8())
                {
                    let step_count = (step_raw as i32).max(3);
                    let mut presets = Vec::with_capacity(preset_count as usize);
                    for _ in 0..preset_count {
                        let Some(id) = r.u8() else { break };
                        let _ = r.lp_string();
                        presets.push(id as i32);
                    }
                    let has_cb = band_raw == 6 || band_raw == 11;
                    let actual_bands =
                        (if has_cb { band_raw as i32 - 1 } else { band_raw as i32 }).max(1);
                    let resized = if next.equalizer.bands.len() == actual_bands as usize {
                        next.equalizer.bands.clone()
                    } else {
                        (0..actual_bands as usize)
                            .map(|idx| *next.equalizer.bands.get(idx).unwrap_or(&0))
                            .collect()
                    };
                    next.equalizer.available_preset_codes = presets;
                    next.equalizer.band_count = actual_bands;
                    next.equalizer.step_count = step_count;
                    next.equalizer.has_clear_bass = has_cb;
                    next.equalizer.bands = resized;
                }
            }
        }
        (V1_EQ_RET_PARAM | V1_EQ_NTFY_PARAM, 0x01) => {
            if let (Some(preset), Some(count)) = (r.u8(), r.u8()) {
                let zero_offset = next.equalizer.step_count / 2;
                let count_usize = count as usize;
                if count_usize >= 1 && r.remaining() >= count_usize {
                    let has_cb = match count_usize {
                        6 | 11 => true,
                        5 | 10 => false,
                        _ => next.equalizer.has_clear_bass && count_usize > 1,
                    };
                    let cb = if has_cb {
                        r.u8().map(|v| v as i32).unwrap_or(zero_offset) - zero_offset
                    } else {
                        0
                    };
                    let actual_count = if has_cb { count_usize - 1 } else { count_usize };
                    let mut bands = Vec::with_capacity(actual_count);
                    for _ in 0..actual_count {
                        bands.push(r.u8().map(|v| v as i32).unwrap_or(zero_offset) - zero_offset);
                    }
                    next.equalizer.preset_code = preset as i32;
                    next.equalizer.clear_bass = cb;
                    next.equalizer.bands = bands;
                    next.equalizer.band_count = actual_count as i32;
                    next.equalizer.has_clear_bass = has_cb;
                } else {
                    next.equalizer.preset_code = preset as i32;
                }
            }
        }
        (V1_NC_RET_PARAM | V1_NC_NTFY_PARAM, 0x01) => {
            let effect = r.u8() == Some(0x01);
            let nc_on = r.u8().unwrap_or(0) != 0;
            next.noise_control.mode = if !effect {
                NoiseControlMode::Off
            } else if nc_on {
                NoiseControlMode::NoiseCancelling
            } else {
                NoiseControlMode::AmbientSound
            };
        }
        (V1_NC_RET_PARAM | V1_NC_NTFY_PARAM, 0x02) => {
            if r.remaining() >= 6 {
                let effect = r.u8() == Some(0x01);
                let _ = r.u8();
                let nc_value = r.u8().unwrap_or(0);
                let _ = r.u8();
                let asm_id = r.u8().unwrap_or(0);
                let asm_amount = r.u8().unwrap_or(0).clamp(0, 20) as i32;
                let mode = if !effect {
                    NoiseControlMode::Off
                } else if nc_value != 0 && asm_amount == 0 {
                    NoiseControlMode::NoiseCancelling
                } else {
                    NoiseControlMode::AmbientSound
                };
                next.noise_control = NoiseControlState {
                    mode,
                    ambient_level: if asm_amount == 0 {
                        next.noise_control.ambient_level.max(1)
                    } else {
                        asm_amount
                    },
                    focus_on_voice: asm_id == 0x01,
                };
            }
        }
        (V1_AUDIO_RET_PARAM | V1_AUDIO_NTFY_PARAM, 0x01) => {
            if let Some(b) = r.u8() {
                next.connection_priority = ConnectionPriority::from_byte(b);
            }
        }
        (V1_AUDIO_RET_PARAM | V1_AUDIO_NTFY_PARAM, 0x02) => {
            next.dsee_extreme = r.u8() == Some(0x01);
        }
        (V1_SYS_RET_PARAM | V1_SYS_NTFY_PARAM, 0x01) => {
            if let Some(mode_byte) = r.u8() {
                let last_timer = r.u8().unwrap_or(0);
                if let Some(mode) = AutoPowerOff::from_byte(mode_byte) {
                    next.auto_power_off = mode;
                }
                next.auto_power_off_last_timer_byte = last_timer;
            }
        }
        (V1_SYS_RET_PARAM | V1_SYS_NTFY_PARAM, 0x02) => {
            next.wearing_detection = r.u8() == Some(0x00);
        }
        (V1_SYS_RET_CAPABILITY, 0x03) => {
            if let Some(key_count) = r.u8() {
                let mut updated = next.assignable_settings.clone();
                for idx in 0..key_count as usize {
                    let Some(key_side_u8) = r.u8() else { break };
                    let key_side = key_side_u8 as i32;
                    let default_preset = r
                        .u8()
                        .map(AssignablePreset::from_byte)
                        .unwrap_or(AssignablePreset::NoFunction);
                    let Some(opt_count) = r.u8() else { break };
                    let mut opts = Vec::new();
                    for _ in 0..opt_count {
                        if let Some(b) = r.u8() {
                            let p = AssignablePreset::from_byte(b);
                            if !opts.contains(&p) {
                                opts.push(p);
                            }
                        }
                    }
                    if let Some(pos) = updated.iter().position(|s| s.key_side == key_side) {
                        updated[pos].available_presets = opts;
                    } else if idx < updated.len() {
                        updated[idx].key_side = key_side;
                        updated[idx].available_presets = opts;
                    } else {
                        updated.push(AssignableSetting {
                            key_side,
                            preset: default_preset,
                            available_presets: opts,
                        });
                    }
                }
                updated.sort_by_key(|s| s.key_side);
                next.assignable_settings = updated;
            }
        }
        (V1_SYS_RET_PARAM | V1_SYS_NTFY_PARAM, 0x03) => {
            if let Some(count) = r.u8() {
                let mut updated = next.assignable_settings.clone();
                for idx in 0..count as usize {
                    let Some(preset_byte) = r.u8() else { break };
                    let preset = AssignablePreset::from_byte(preset_byte);
                    if idx < updated.len() {
                        updated[idx].preset = preset;
                    } else {
                        updated.push(AssignableSetting {
                            key_side: idx as i32,
                            preset,
                            available_presets: Vec::new(),
                        });
                    }
                }
                next.assignable_settings = updated;
            }
        }
        (V1_VG_RET_PARAM | V1_VG_NTFY_PARAM, 0x01) => {
            next.voice_guidance_enabled = r.u8() == Some(0x00);
            if r.remaining() > 0 {
                if let Some(lang) = r.u8() {
                    next.voice_guidance_language_code = lang as i32;
                }
            }
        }
        _ => {}
    }

    next
}

struct V1ByteCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> V1ByteCursor<'a> {
    fn new(bytes: &'a [u8], offset: usize) -> Self {
        Self { bytes, offset }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    fn u8(&mut self) -> Option<u8> {
        if self.offset >= self.bytes.len() {
            return None;
        }
        let v = self.bytes[self.offset];
        self.offset += 1;
        Some(v)
    }

    fn lp_string(&mut self) -> Option<String> {
        let len = self.u8()? as usize;
        if self.offset + len > self.bytes.len() {
            return None;
        }
        let s = String::from_utf8_lossy(&self.bytes[self.offset..self.offset + len]).to_string();
        self.offset += len;
        Some(s)
    }
}
