use crate::models::{
    AssignablePreset, AssignableSetting, AudioCodec, AutoPowerOff, BatteryUnit, ChargingStatus,
    ConnectionPriority, GeneralSetting, HeadphoneFinish, HeadphoneState,
    HeadsetAlert, NcAmbButtonCycle, NoiseControlMode, NoiseControlState, PairedDevice,
    QuickAccessService, SpeakToChatSensitivity, SpeakToChatTimeout, VoiceAssistant,
};

pub const SERVICE_UUID_V2: &str = "956C7B26-D49A-4BA8-B03F-B17D393CB6E2";

// Table 1 command opcodes
const T1_CONNECT_GET_PROTOCOL: u8 = 0x00;
const T1_CONNECT_RET_PROTOCOL: u8 = 0x01;
const T1_CONNECT_GET_DEVICE_INFO: u8 = 0x04;
const T1_CONNECT_RET_DEVICE_INFO: u8 = 0x05;
const T1_CONNECT_GET_SUPPORT: u8 = 0x06;
const T1_CONNECT_RET_SUPPORT: u8 = 0x07;
const T1_COMMON_GET_STATUS: u8 = 0x12;
const T1_COMMON_RET_STATUS: u8 = 0x13;
const T1_COMMON_NTFY_STATUS: u8 = 0x15;
const T1_POWER_GET_STATUS: u8 = 0x22;
const T1_POWER_RET_STATUS: u8 = 0x23;
const T1_POWER_SET_STATUS: u8 = 0x24;
const T1_POWER_NTFY_STATUS: u8 = 0x25;
const T1_POWER_GET_PARAM: u8 = 0x26;
const T1_POWER_RET_PARAM: u8 = 0x27;
const T1_POWER_SET_PARAM: u8 = 0x28;
const T1_POWER_NTFY_PARAM: u8 = 0x29;
const T1_EQ_GET_CAPABILITY: u8 = 0x50;
const T1_EQ_RET_CAPABILITY: u8 = 0x51;
const T1_EQ_GET_PARAM: u8 = 0x56;
const T1_EQ_RET_PARAM: u8 = 0x57;
const T1_EQ_SET_PARAM: u8 = 0x58;
const T1_EQ_NTFY_PARAM: u8 = 0x59;
const T1_NC_GET_PARAM: u8 = 0x66;
const T1_NC_RET_PARAM: u8 = 0x67;
const T1_NC_SET_PARAM: u8 = 0x68;
const T1_NC_NTFY_PARAM: u8 = 0x69;
const T1_ALERT_SET_STATUS: u8 = 0x94;
const T1_ALERT_SET_PARAM: u8 = 0x98;
const T1_ALERT_NTFY_PARAM: u8 = 0x99;
const T1_GS_GET_CAPABILITY: u8 = 0xD0;
const T1_GS_RET_CAPABILITY: u8 = 0xD1;
const T1_GS_GET_PARAM: u8 = 0xD6;
const T1_GS_RET_PARAM: u8 = 0xD7;
const T1_GS_SET_PARAM: u8 = 0xD8;
const T1_GS_NTFY_PARAM: u8 = 0xD9;
const T1_AUDIO_GET_PARAM: u8 = 0xE6;
const T1_AUDIO_RET_PARAM: u8 = 0xE7;
const T1_AUDIO_SET_PARAM: u8 = 0xE8;
const T1_AUDIO_NTFY_PARAM: u8 = 0xE9;
const T1_SYS_GET_CAPABILITY: u8 = 0xF0;
const T1_SYS_RET_CAPABILITY: u8 = 0xF1;
const T1_SYS_GET_PARAM: u8 = 0xF6;
const T1_SYS_RET_PARAM: u8 = 0xF7;
const T1_SYS_SET_PARAM: u8 = 0xF8;
const T1_SYS_NTFY_PARAM: u8 = 0xF9;
const T1_SYS_GET_EXT_PARAM: u8 = 0xFA;
const T1_SYS_RET_EXT_PARAM: u8 = 0xFB;
const T1_SYS_SET_EXT_PARAM: u8 = 0xFC;
const T1_SYS_NTFY_EXT_PARAM: u8 = 0xFD;

// Table 2 command opcodes
const T2_CONNECT_GET_SUPPORT: u8 = 0x06;
const T2_CONNECT_RET_SUPPORT: u8 = 0x07;
const T2_PERI_GET_STATUS: u8 = 0x32;
const T2_PERI_RET_STATUS: u8 = 0x33;
const T2_PERI_SET_STATUS: u8 = 0x34;
const T2_PERI_NTFY_STATUS: u8 = 0x35;
const T2_PERI_GET_PARAM: u8 = 0x36;
const T2_PERI_RET_PARAM: u8 = 0x37;
const T2_PERI_SET_PARAM: u8 = 0x38;
const T2_PERI_NTFY_PARAM: u8 = 0x39;
const T2_PERI_SET_EXT_PARAM: u8 = 0x3C;
const T2_PERI_NTFY_EXT_PARAM: u8 = 0x3D;
const T2_VG_GET_PARAM: u8 = 0x46;
const T2_VG_RET_PARAM: u8 = 0x47;
const T2_VG_SET_PARAM: u8 = 0x48;
const T2_VG_NTFY_PARAM: u8 = 0x49;

// MARK: - Query Builders

pub fn get_protocol_info() -> Vec<u8> {
    vec![T1_CONNECT_GET_PROTOCOL, 0x00]
}
pub fn get_model_name() -> Vec<u8> {
    vec![T1_CONNECT_GET_DEVICE_INFO, 0x01]
}
pub fn get_firmware_version() -> Vec<u8> {
    vec![T1_CONNECT_GET_DEVICE_INFO, 0x02]
}
pub fn get_series_and_color_info() -> Vec<u8> {
    vec![T1_CONNECT_GET_DEVICE_INFO, 0x03]
}
pub fn get_support_functions_t1() -> Vec<u8> {
    vec![T1_CONNECT_GET_SUPPORT, 0x00]
}
pub fn get_support_functions_t2() -> Vec<u8> {
    vec![T2_CONNECT_GET_SUPPORT, 0x00]
}
pub fn enable_alerts() -> Vec<u8> {
    vec![T1_ALERT_SET_STATUS, 0x00, 0x00]
}

pub fn get_battery() -> Vec<u8> {
    vec![T1_POWER_GET_STATUS, 0x00]
}
pub fn get_left_right_battery() -> Vec<u8> {
    vec![T1_POWER_GET_STATUS, 0x01]
}
pub fn get_cradle_battery() -> Vec<u8> {
    vec![T1_POWER_GET_STATUS, 0x02]
}
pub fn get_codec() -> Vec<u8> {
    vec![T1_COMMON_GET_STATUS, 0x02]
}
pub fn get_noise_control(subtype: i32) -> Vec<u8> {
    vec![T1_NC_GET_PARAM, subtype as u8]
}
pub fn get_nc_amb_button_cycle() -> Vec<u8> {
    vec![T1_NC_GET_PARAM, 0x30]
}
pub fn get_eq_capability() -> Vec<u8> {
    vec![T1_EQ_GET_CAPABILITY, 0x00, 0x01]
}
pub fn get_eq() -> Vec<u8> {
    vec![T1_EQ_GET_PARAM, 0x00]
}
pub fn get_dsee() -> Vec<u8> {
    vec![T1_AUDIO_GET_PARAM, 0x01]
}
pub fn get_connection_priority() -> Vec<u8> {
    vec![T1_AUDIO_GET_PARAM, 0x00]
}
pub fn get_auto_power_off(subtype: i32) -> Vec<u8> {
    vec![T1_POWER_GET_PARAM, subtype as u8]
}
pub fn get_wearing_detection() -> Vec<u8> {
    vec![T1_SYS_GET_PARAM, 0x01]
}
pub fn get_assignable_settings_capability() -> Vec<u8> {
    vec![T1_SYS_GET_CAPABILITY, 0x03]
}
pub fn get_assignable_settings() -> Vec<u8> {
    vec![T1_SYS_GET_PARAM, 0x03]
}
pub fn get_voice_assistant_capability() -> Vec<u8> {
    vec![T1_SYS_GET_CAPABILITY, 0x04]
}
pub fn get_voice_assistant() -> Vec<u8> {
    vec![T1_SYS_GET_PARAM, 0x04]
}
pub fn get_speak_to_chat() -> Vec<u8> {
    vec![T1_SYS_GET_PARAM, 0x0C]
}
pub fn get_speak_to_chat_config() -> Vec<u8> {
    vec![T1_SYS_GET_EXT_PARAM, 0x0C]
}
pub fn get_quick_access_capability() -> Vec<u8> {
    vec![T1_SYS_GET_CAPABILITY, 0x0D]
}
pub fn get_quick_access() -> Vec<u8> {
    vec![T1_SYS_GET_PARAM, 0x0D]
}
pub fn get_general_setting_capability(slot_id: i32) -> Vec<u8> {
    vec![T1_GS_GET_CAPABILITY, slot_id as u8, 0x01]
}
pub fn get_general_setting(slot_id: i32) -> Vec<u8> {
    vec![T1_GS_GET_PARAM, slot_id as u8]
}

// Table 2 queries
pub fn get_voice_guidance() -> Vec<u8> {
    vec![T2_VG_GET_PARAM, 0x01]
}
pub fn get_pairing_status() -> Vec<u8> {
    vec![T2_PERI_GET_STATUS, 0x02]
}
pub fn get_paired_devices() -> Vec<u8> {
    vec![T2_PERI_GET_PARAM, 0x02]
}
pub fn get_source_switch() -> Vec<u8> {
    vec![T2_PERI_GET_PARAM, 0x01]
}

// MARK: - Setter Builders

pub fn set_noise_control(state: &NoiseControlState, dragging: bool, subtype: i32) -> Vec<u8> {
    let effect_byte = if state.mode == NoiseControlMode::Off {
        0x00
    } else {
        0x01
    };
    let voice_byte = if state.focus_on_voice { 0x01 } else { 0x00 };
    let level_byte = state.ambient_level.clamp(1, 20) as u8;

    if subtype == 0x11 {
        let nc_value = if state.mode == NoiseControlMode::NoiseCancelling {
            0x02
        } else {
            0x00
        };
        let asm_amount = if state.mode == NoiseControlMode::AmbientSound {
            level_byte
        } else {
            0x00
        };
        return vec![
            T1_NC_SET_PARAM,
            0x11,
            effect_byte,
            0x02,
            nc_value,
            0x01,
            voice_byte,
            asm_amount,
        ];
    }

    let settled_byte = if dragging { 0x00 } else { 0x01 };
    let type_byte = if state.mode == NoiseControlMode::AmbientSound {
        0x01
    } else {
        0x00
    };
    vec![
        T1_NC_SET_PARAM,
        0x17,
        settled_byte,
        effect_byte,
        type_byte,
        voice_byte,
        level_byte,
    ]
}

pub fn set_nc_amb_button_cycle(cycle: NcAmbButtonCycle) -> Vec<u8> {
    vec![T1_NC_SET_PARAM, 0x30, cycle.wire_value()]
}

pub fn set_eq_preset(preset_code: i32) -> Vec<u8> {
    vec![T1_EQ_SET_PARAM, 0x00, preset_code as u8, 0x00]
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
    let mut clamped = Vec::with_capacity(bands.len() + 1);
    if has_clear_bass {
        clamped.push(clear_bass);
    }
    clamped.extend_from_slice(bands);

    let mut out = Vec::with_capacity(4 + clamped.len());
    out.push(T1_EQ_SET_PARAM);
    out.push(0x00);
    out.push(preset_code as u8);
    out.push(clamped.len() as u8);
    for v in clamped {
        out.push((v.clamp(-max_gain, max_gain) + zero_offset) as u8);
    }
    out
}

pub fn set_dsee(enabled: bool) -> Vec<u8> {
    vec![T1_AUDIO_SET_PARAM, 0x01, if enabled { 0x01 } else { 0x00 }]
}

pub fn set_connection_priority(priority: ConnectionPriority) -> Vec<u8> {
    vec![T1_AUDIO_SET_PARAM, 0x00, priority.wire_byte()]
}

pub fn set_speak_to_chat(enabled: bool) -> Vec<u8> {
    vec![
        T1_SYS_SET_PARAM,
        0x0C,
        if enabled { 0x00 } else { 0x01 },
        0x01,
    ]
}

pub fn set_speak_to_chat_config(
    sensitivity: SpeakToChatSensitivity,
    timeout: SpeakToChatTimeout,
) -> Vec<u8> {
    vec![
        T1_SYS_SET_EXT_PARAM,
        0x0C,
        sensitivity.wire_byte(),
        timeout.wire_byte(),
    ]
}

pub fn set_wearing_detection(enabled: bool) -> Vec<u8> {
    vec![T1_SYS_SET_PARAM, 0x01, if enabled { 0x00 } else { 0x01 }]
}

pub fn set_auto_power_off(mode: AutoPowerOff, last_timer_byte: u8, subtype: i32) -> Vec<u8> {
    vec![
        T1_POWER_SET_PARAM,
        subtype as u8,
        mode.wire_byte(),
        last_timer_byte,
    ]
}

pub fn set_assignable_settings(presets: &[AssignablePreset]) -> Vec<u8> {
    let mut out = Vec::with_capacity(3 + presets.len());
    out.push(T1_SYS_SET_PARAM);
    out.push(0x03);
    out.push(presets.len() as u8);
    for p in presets {
        out.push(p.wire_byte());
    }
    out
}

pub fn set_voice_assistant(assistant: VoiceAssistant) -> Vec<u8> {
    vec![T1_SYS_SET_PARAM, 0x04, assistant.wire_byte()]
}

pub fn set_quick_access(double_tap: QuickAccessService, triple_tap: QuickAccessService) -> Vec<u8> {
    vec![
        T1_SYS_SET_PARAM,
        0x0D,
        0x02,
        double_tap.0,
        triple_tap.0,
    ]
}

pub fn set_general_setting(slot_id: i32, enabled: bool) -> Vec<u8> {
    vec![
        T1_GS_SET_PARAM,
        slot_id as u8,
        0x00,
        if enabled { 0x00 } else { 0x01 },
    ]
}

pub fn respond_to_alert(message_type: i32, accept: bool) -> Vec<u8> {
    vec![
        T1_ALERT_SET_PARAM,
        0x00,
        message_type as u8,
        if accept { 0x01 } else { 0x00 },
    ]
}

pub fn power_off_headphones() -> Vec<u8> {
    vec![T1_POWER_SET_STATUS, 0x03, 0x01]
}

// Table 2 setters
pub fn set_voice_guidance(enabled: bool, language_code: i32) -> Vec<u8> {
    vec![
        T2_VG_SET_PARAM,
        0x01,
        if enabled { 0x00 } else { 0x01 },
        language_code as u8,
    ]
}

pub fn set_pairing_mode(enabled: bool) -> Vec<u8> {
    vec![
        T2_PERI_SET_STATUS,
        0x02,
        if enabled { 0x01 } else { 0x00 },
        0x00,
    ]
}

pub fn switch_audio_source(mac_address: &str) -> Vec<u8> {
    let mut out = vec![T2_PERI_SET_EXT_PARAM, 0x01];
    out.extend_from_slice(&ascii_mac_bytes(mac_address));
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceAction {
    Disconnect = 0x00,
    Connect = 0x01,
    Unpair = 0x02,
}

pub fn perform_device_action(action: DeviceAction, mac_address: &str) -> Vec<u8> {
    let mut out = vec![T2_PERI_SET_EXT_PARAM, 0x02, action as u8];
    out.extend_from_slice(&ascii_mac_bytes(mac_address));
    out
}

pub fn set_source_switch(enabled: bool) -> Vec<u8> {
    vec![
        T2_PERI_SET_PARAM,
        0x01,
        if enabled { 0x01 } else { 0x00 },
    ]
}

fn ascii_mac_bytes(address: &str) -> [u8; 17] {
    let normalized = address.to_uppercase().replace('-', ":");
    let mut out = [b'0'; 17];
    for (i, b) in normalized.bytes().take(17).enumerate() {
        out[i] = b;
    }
    out
}

// MARK: - Parsers

pub fn apply_table1_message(state: &HeadphoneState, payload: &[u8]) -> HeadphoneState {
    if payload.len() < 2 {
        return state.clone();
    }
    let op = payload[0];
    let sub = payload[1];
    let mut r = ByteCursor::new(payload, 2);
    let mut next = state.clone();

    match (op, sub) {
        (T1_CONNECT_RET_PROTOCOL, _) => {
            if r.remaining() >= 6 {
                r.skip(4);
                let _ = r.u8();
                let t2 = r.u8() == Some(0x00);
                next.has_table_2 = t2;
            }
        }
        (T1_CONNECT_RET_DEVICE_INFO, 0x01) => {
            if let Some(name) = r.lp_string() {
                next.model_name = name;
            }
        }
        (T1_CONNECT_RET_DEVICE_INFO, 0x02) => {
            if let Some(fw) = r.lp_string() {
                next.firmware_version = fw;
            }
        }
        (T1_CONNECT_RET_DEVICE_INFO, 0x03) => {
            if r.u8().is_some() {
                if let Some(color) = r.u8() {
                    next.hardware_color_code = Some(color as i32);
                    next.detected_finish = HeadphoneFinish::from_sony_color_byte(color as i32);
                }
            }
        }
        (T1_CONNECT_RET_SUPPORT, _) => {
            if let Some(funcs) = r.support_functions_v2() {
                next.t1_functions = funcs;
            }
        }
        (T1_COMMON_RET_STATUS | T1_COMMON_NTFY_STATUS, 0x02) => {
            if let Some(code) = r.u8() {
                next.codec = AudioCodec::from_byte(code);
            }
        }
        (T1_POWER_RET_STATUS | T1_POWER_NTFY_STATUS, 0x00) => {
            if let Some(lvl) = r.u8() {
                let chg = r.u8().unwrap_or(0x00);
                next.battery.level = Some((lvl as i32).clamp(0, 100));
                next.battery.charging_status = ChargingStatus::from_byte(chg);
            }
        }
        (T1_POWER_RET_STATUS | T1_POWER_NTFY_STATUS, 0x01) => {
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
        (T1_POWER_RET_STATUS | T1_POWER_NTFY_STATUS, 0x02) => {
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
        (T1_POWER_RET_PARAM | T1_POWER_NTFY_PARAM, 0x04 | 0x05) => {
            if let Some(mode_byte) = r.u8() {
                let last_timer = r.u8().unwrap_or(0);
                if let Some(mode) = AutoPowerOff::from_byte(mode_byte) {
                    next.auto_power_off = mode;
                }
                next.auto_power_off_last_timer_byte = last_timer;
            }
        }
        (T1_EQ_RET_CAPABILITY, 0x00) => {
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
        (T1_EQ_RET_PARAM | T1_EQ_NTFY_PARAM, 0x00) => {
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
        (T1_NC_RET_PARAM | T1_NC_NTFY_PARAM, 0x17) => {
            if r.remaining() >= 5 {
                let _ = r.u8();
                let on = r.u8() == Some(0x01);
                let ambient = r.u8() == Some(0x01);
                let voice = r.u8() == Some(0x01);
                let level = r.u8().unwrap_or(1).clamp(0, 20) as i32;
                let mode = if !on {
                    NoiseControlMode::Off
                } else if ambient {
                    NoiseControlMode::AmbientSound
                } else {
                    NoiseControlMode::NoiseCancelling
                };
                next.noise_control = NoiseControlState {
                    mode,
                    ambient_level: if level == 0 {
                        next.noise_control.ambient_level.max(1)
                    } else {
                        level
                    },
                    focus_on_voice: voice,
                };
            }
        }
        (T1_NC_RET_PARAM | T1_NC_NTFY_PARAM, 0x11) => {
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
        (T1_NC_RET_PARAM | T1_NC_NTFY_PARAM, 0x30) => {
            if let Some(b) = r.u8() {
                if let Some(cycle) = NcAmbButtonCycle::from_byte(b) {
                    next.nc_amb_button_cycle = cycle;
                }
            }
        }
        (T1_ALERT_NTFY_PARAM, 0x00) => {
            if let Some(msg_type) = r.u8() {
                let action = r.u8().unwrap_or(0);
                next.pending_alert = Some(HeadsetAlert {
                    message_type: msg_type as i32,
                    needs_answer: action == 0x01,
                });
            }
        }
        (T1_GS_RET_CAPABILITY, slot) => {
            if r.remaining() >= 3 {
                let _ = r.u8();
                let _ = r.u8();
                if let Some(key) = r.lp_string() {
                    let summary = r.lp_string().unwrap_or_default();
                    let slot_id = slot as i32;
                    if let Some(existing) =
                        next.general_settings.iter_mut().find(|g| g.id == slot_id)
                    {
                        existing.key = key;
                        existing.summary_key = summary;
                    } else {
                        next.general_settings.push(GeneralSetting {
                            id: slot_id,
                            key,
                            summary_key: summary,
                            is_on: false,
                        });
                        next.general_settings.sort_by_key(|g| g.id);
                    }
                }
            }
        }
        (T1_GS_RET_PARAM | T1_GS_NTFY_PARAM, slot) => {
            if r.remaining() >= 2 {
                let _ = r.u8();
                let is_on = r.u8() == Some(0x00);
                let slot_id = slot as i32;
                for g in &mut next.general_settings {
                    if g.id == slot_id {
                        g.is_on = is_on;
                    }
                }
            }
        }
        (T1_AUDIO_RET_PARAM | T1_AUDIO_NTFY_PARAM, 0x00) => {
            if let Some(b) = r.u8() {
                next.connection_priority = ConnectionPriority::from_byte(b);
            }
        }
        (T1_AUDIO_RET_PARAM | T1_AUDIO_NTFY_PARAM, 0x01) => {
            next.dsee_extreme = r.u8() == Some(0x01);
        }
        (T1_SYS_RET_CAPABILITY, 0x03) => {
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
        (T1_SYS_RET_CAPABILITY, 0x04) => {
            if r.u8().is_some() {
                if let Some(count) = r.u8() {
                    let mut opts = Vec::new();
                    for _ in 0..count {
                        if let Some(b) = r.u8() {
                            let va = VoiceAssistant::from_byte(b);
                            if !opts.contains(&va) {
                                opts.push(va);
                            }
                        }
                    }
                    next.voice_assistant_options = opts;
                }
            }
        }
        (T1_SYS_RET_CAPABILITY, 0x0D) => {
            if r.u8().is_some() && r.u8().is_some() {
                if let Some(n) = r.u8() {
                    for _ in 0..n {
                        let Some(action) = r.u8() else { break };
                        let _ = r.u8();
                        let Some(count) = r.u8() else { break };
                        let mut opts = Vec::new();
                        for _ in 0..count {
                            if let Some(b) = r.u8() {
                                opts.push(QuickAccessService(b));
                            }
                        }
                        if action == 0x01 {
                            next.quick_access_double_options = opts;
                        } else if action == 0x02 {
                            next.quick_access_triple_options = opts;
                        }
                    }
                }
            }
        }
        (T1_SYS_RET_PARAM | T1_SYS_NTFY_PARAM, 0x01) => {
            next.wearing_detection = r.u8() == Some(0x00);
        }
        (T1_SYS_RET_PARAM | T1_SYS_NTFY_PARAM, 0x03) => {
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
        (T1_SYS_RET_PARAM | T1_SYS_NTFY_PARAM, 0x04) => {
            if let Some(b) = r.u8() {
                next.voice_assistant = VoiceAssistant::from_byte(b);
            }
        }
        (T1_SYS_RET_PARAM | T1_SYS_NTFY_PARAM, 0x0C) => {
            next.speak_to_chat = r.u8() == Some(0x00);
        }
        (T1_SYS_RET_EXT_PARAM | T1_SYS_NTFY_EXT_PARAM, 0x0C) => {
            if let Some(b) = r.u8() {
                next.speak_to_chat_sensitivity = SpeakToChatSensitivity::from_byte(b);
            }
            if let Some(b) = r.u8() {
                next.speak_to_chat_timeout = SpeakToChatTimeout::from_byte(b);
            }
        }
        (T1_SYS_RET_PARAM | T1_SYS_NTFY_PARAM, 0x0D) => {
            if let Some(count) = r.u8() {
                if count >= 1 {
                    if let Some(b) = r.u8() {
                        next.quick_access_double_tap = QuickAccessService(b);
                    }
                }
                if count >= 2 {
                    if let Some(b) = r.u8() {
                        next.quick_access_triple_tap = QuickAccessService(b);
                    }
                }
            }
        }
        _ => {}
    }

    next
}

pub fn apply_table2_message(state: &HeadphoneState, payload: &[u8]) -> HeadphoneState {
    if payload.len() < 2 {
        return state.clone();
    }
    let op = payload[0];
    let sub = payload[1];
    let mut r = ByteCursor::new(payload, 2);
    let mut next = state.clone();

    match (op, sub) {
        (T2_CONNECT_RET_SUPPORT, _) => {
            if let Some(funcs) = r.support_functions_v2() {
                next.t2_functions = funcs;
            }
        }
        (T2_VG_RET_PARAM | T2_VG_NTFY_PARAM, 0x01) => {
            next.voice_guidance_enabled = r.u8() == Some(0x00);
            if r.remaining() > 0 {
                if let Some(lang) = r.u8() {
                    next.voice_guidance_language_code = lang as i32;
                }
            }
        }
        (T2_PERI_RET_STATUS | T2_PERI_NTFY_STATUS, 0x00 | 0x02) => {
            next.pairing_mode = r.u8() == Some(0x01);
        }
        (T2_PERI_RET_PARAM | T2_PERI_NTFY_PARAM, 0x02) => {
            if let Some(n) = r.u8() {
                let mut raw = Vec::with_capacity(n as usize);
                let mut ok = true;
                for _ in 0..n {
                    let Some(addr) = r.ascii(17) else {
                        ok = false;
                        break;
                    };
                    let Some(status) = r.u8() else {
                        ok = false;
                        break;
                    };
                    let Some(cod) = r.u24() else {
                        ok = false;
                        break;
                    };
                    let Some(name) = r.lp_string() else {
                        ok = false;
                        break;
                    };
                    raw.push((addr, status, cod, name));
                }
                if ok {
                    let playing_idx = r.u8().unwrap_or(0);
                    next.paired_devices = raw
                        .into_iter()
                        .map(|(addr, status, cod, name)| PairedDevice {
                            address: addr,
                            name,
                            is_connected: status != 0,
                            is_playing: status != 0 && status == playing_idx,
                            class_of_device: cod as i32,
                        })
                        .collect();
                }
            }
        }
        (T2_PERI_RET_PARAM | T2_PERI_NTFY_PARAM, 0x01) => {
            next.source_switch_enabled = r.u8().unwrap_or(0) != 0;
        }
        (T2_PERI_NTFY_EXT_PARAM, 0x01) => {
            if r.u8().is_some() {
                if let Some(target) = r.ascii(17) {
                    for dev in &mut next.paired_devices {
                        dev.is_playing = dev.address.eq_ignore_ascii_case(&target);
                    }
                }
            }
        }
        _ => {}
    }

    next
}

struct ByteCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ByteCursor<'a> {
    fn new(bytes: &'a [u8], offset: usize) -> Self {
        Self { bytes, offset }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    fn skip(&mut self, n: usize) {
        self.offset = (self.offset + n).min(self.bytes.len());
    }

    fn u8(&mut self) -> Option<u8> {
        if self.offset >= self.bytes.len() {
            return None;
        }
        let v = self.bytes[self.offset];
        self.offset += 1;
        Some(v)
    }

    fn u24(&mut self) -> Option<u32> {
        if self.offset + 3 > self.bytes.len() {
            return None;
        }
        let v = ((self.bytes[self.offset] as u32) << 16)
            | ((self.bytes[self.offset + 1] as u32) << 8)
            | (self.bytes[self.offset + 2] as u32);
        self.offset += 3;
        Some(v)
    }

    fn ascii(&mut self, n: usize) -> Option<String> {
        if self.offset + n > self.bytes.len() {
            return None;
        }
        let s = String::from_utf8_lossy(&self.bytes[self.offset..self.offset + n]).to_string();
        self.offset += n;
        Some(s)
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

    fn support_functions_v2(&mut self) -> Option<std::collections::HashSet<i32>> {
        let n = self.u8()? as usize;
        let mut out = std::collections::HashSet::with_capacity(n);
        for _ in 0..n {
            let fn_id = self.u8()? as i32;
            let _ = self.u8()?;
            out.insert(fn_id);
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_v2_battery_and_nc_and_eq() {
        let mut st = HeadphoneState::default();
        st = apply_table1_message(&st, &[0x23, 0x00, 82, 0x01]);
        assert_eq!(st.battery.level, Some(82));
        assert_eq!(st.battery.charging_status, ChargingStatus::Charging);

        st = apply_table1_message(&st, &[0x67, 0x17, 0x01, 0x01, 0x01, 0x01, 14]);
        assert_eq!(st.noise_control.mode, NoiseControlMode::AmbientSound);
        assert_eq!(st.noise_control.ambient_level, 14);
        assert!(st.noise_control.focus_on_voice);

        let eq_cmd = set_eq_custom(3, &[1, 2, 0, -1, 4], 0xA0, 21, true);
        assert_eq!(eq_cmd, vec![0x58, 0x00, 0xA0, 6, 13, 11, 12, 10, 9, 14]);
    }
}
