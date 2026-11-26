use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};

use crate::models::{
    AssignablePreset, AssignableSetting, AudioCodec, AutoPowerOff, BatteryState, BatteryUnit,
    ChargingStatus, ConnectionPriority, EqualizerState, GeneralSetting, HeadphoneCapabilities,
    HeadphoneFinish, HeadphoneState, MdrProtocolVersion, NcAmbButtonCycle, NoiseControlMode,
    NoiseControlState, PairedDevice, QuickAccessService, SimulatedProfile, SpeakToChatSensitivity,
    SpeakToChatTimeout, VoiceAssistant,
};
use crate::protocol::bose_bmap::{self, BoseBmapFrame, BoseBmapStreamDecoder};
use crate::protocol::galaxy_buds::{self, GalaxyBudsFrame, GalaxyBudsStreamDecoder};
use crate::protocol::mdr_frame::{encode_frame, MdrDataType, MdrFrame, MdrStreamDecoder};
use crate::protocol::soundcore::{self, SoundcorePacket, SoundcoreStreamDecoder};
use crate::protocol::transport::{BluetoothTransport, ProtocolError};

struct RxQueue {
    chunks: VecDeque<Vec<u8>>,
    closed: bool,
}

pub struct SimulatedSonyTransport {
    profile: SimulatedProfile,
    connected: AtomicBool,
    rx_queue: (Mutex<RxQueue>, Condvar),
    pending_buf: Mutex<(Vec<u8>, usize)>,
    decoder: MdrStreamDecoder,
    sim_tx_seq: Mutex<u8>,
    state: Mutex<HeadphoneState>,
}

impl SimulatedSonyTransport {
    pub fn new(profile: SimulatedProfile) -> Self {
        Self {
            profile,
            connected: AtomicBool::new(false),
            rx_queue: (
                Mutex::new(RxQueue {
                    chunks: VecDeque::new(),
                    closed: false,
                }),
                Condvar::new(),
            ),
            pending_buf: Mutex::new((Vec::new(), 0)),
            decoder: MdrStreamDecoder::new(),
            sim_tx_seq: Mutex::new(0),
            state: Mutex::new(initial_state_for(profile)),
        }
    }

    pub fn simulate_battery_drop(&self, delta_percent: i32) {
        let mut st = self.state.lock().unwrap();
        let ntfy_op: u8 = if st.protocol_version == MdrProtocolVersion::V1 {
            0x12
        } else {
            0x25
        };
        if st.battery.is_dual_battery() {
            let l = (st.battery.left.as_ref().map(|u| u.level).unwrap_or(82) - delta_percent)
                .clamp(1, 100);
            let r = (st.battery.right.as_ref().map(|u| u.level).unwrap_or(76) - delta_percent)
                .clamp(1, 100);
            st.battery.left = Some(BatteryUnit::new(l, ChargingStatus::NotCharging));
            st.battery.right = Some(BatteryUnit::new(r, ChargingStatus::NotCharging));
            st.battery.level = Some(l.min(r));
            st.battery.charging_status = ChargingStatus::NotCharging;
            drop(st);
            self.emit_table1(vec![ntfy_op, 0x01, l as u8, 0x00, r as u8, 0x00]);
        } else {
            let next = (st.battery.level.unwrap_or(78) - delta_percent).clamp(1, 100);
            st.battery.level = Some(next);
            st.battery.charging_status = ChargingStatus::NotCharging;
            drop(st);
            self.emit_table1(vec![ntfy_op, 0x00, next as u8, 0x00]);
        }
    }

    fn enqueue_rx(&self, data: Vec<u8>) {
        let (lock, cv) = &self.rx_queue;
        let mut q = lock.lock().unwrap();
        q.chunks.push_back(data);
        cv.notify_all();
    }

    fn emit_table1(&self, payload: Vec<u8>) {
        let mut seq = self.sim_tx_seq.lock().unwrap();
        let frame = MdrFrame {
            frame_type: MdrDataType::DataMdr,
            sequence: *seq,
            payload,
        };
        *seq = 1 - (*seq & 0x01);
        if let Ok(bytes) = encode_frame(&frame) {
            self.enqueue_rx(bytes);
        }
    }

    fn emit_table2(&self, payload: Vec<u8>) {
        let mut seq = self.sim_tx_seq.lock().unwrap();
        let frame = MdrFrame {
            frame_type: MdrDataType::DataMdrNo2,
            sequence: *seq,
            payload,
        };
        *seq = 1 - (*seq & 0x01);
        if let Ok(bytes) = encode_frame(&frame) {
            self.enqueue_rx(bytes);
        }
    }

    fn handle_v1_command(&self, table: MdrDataType, p: &[u8]) {
        if table != MdrDataType::DataMdr || p.len() < 2 {
            return;
        }
        let op = p[0];
        let sub = p[1];
        let mut st = self.state.lock().unwrap();
        match (op, sub) {
            (0x00, _) => self.emit_table1(vec![0x01, 0x00, 0x01, 0x00]),
            (0x04, 0x01) => self.emit_table1(lp_string_reply(0x05, 0x01, &st.model_name)),
            (0x04, 0x02) => self.emit_table1(lp_string_reply(0x05, 0x02, &st.firmware_version)),
            (0x04, 0x03) => self.emit_table1(vec![
                0x05,
                0x03,
                0x20,
                st.hardware_color_code.unwrap_or(0x01) as u8,
            ]),
            (0x06, _) => {
                let mut ids: Vec<i32> = st.t1_functions.iter().copied().collect();
                ids.sort();
                let mut out = vec![0x07, 0x00, ids.len() as u8];
                for id in ids {
                    out.push(id as u8);
                }
                self.emit_table1(out);
            }
            (0x10, 0x00) => self.emit_table1(vec![
                0x11,
                0x00,
                st.battery.level.unwrap_or(72) as u8,
                st.battery.charging_status.wire_byte(),
            ]),
            (0x10, 0x01) => {
                let l = st
                    .battery
                    .left
                    .clone()
                    .unwrap_or_else(|| BatteryUnit::new(80, ChargingStatus::NotCharging));
                let r = st
                    .battery
                    .right
                    .clone()
                    .unwrap_or_else(|| BatteryUnit::new(68, ChargingStatus::NotCharging));
                self.emit_table1(vec![
                    0x11,
                    0x01,
                    l.level as u8,
                    l.charging_status.wire_byte(),
                    r.level as u8,
                    r.charging_status.wire_byte(),
                ]);
            }
            (0x10, 0x02) => {
                let c = st
                    .battery
                    .case_battery
                    .clone()
                    .unwrap_or_else(|| BatteryUnit::new(90, ChargingStatus::NotCharging));
                self.emit_table1(vec![0x11, 0x02, c.level as u8, c.charging_status.wire_byte()]);
            }
            (0x14, 0x00) => self.emit_table1(vec![0x15, 0x00, st.codec.wire_byte()]),
            (0x22, 0x00) => {
                drop(st);
                self.close();
            }
            (0x50, 0x01) => self.emit_table1(build_eq_capability(&st.equalizer, 0x01)),
            (0x56, 0x01) => self.emit_table1(build_eq_reply(0x57, 0x01, &st.equalizer)),
            (0x58, 0x01) if p.len() >= 4 => {
                apply_eq_set(&mut st.equalizer, p);
                self.emit_table1(build_eq_reply(0x59, 0x01, &st.equalizer));
            }
            (0x66, 0x02) => self.emit_table1(build_v1_nc_reply(0x67, &st.noise_control)),
            (0x68, 0x02) if p.len() >= 8 => {
                let effect = p[2] == 0x01;
                let nc_val = p[4];
                let voice = p[6] == 0x01;
                let asm_amt = (p[7] as i32).clamp(0, 20);
                let mode = if !effect {
                    NoiseControlMode::Off
                } else if nc_val != 0 && asm_amt == 0 {
                    NoiseControlMode::NoiseCancelling
                } else {
                    NoiseControlMode::AmbientSound
                };
                let lvl = if asm_amt == 0 {
                    st.noise_control.ambient_level
                } else {
                    asm_amt
                };
                st.noise_control = NoiseControlState {
                    mode,
                    ambient_level: lvl,
                    focus_on_voice: voice,
                };
                self.emit_table1(build_v1_nc_reply(0x69, &st.noise_control));
            }
            (0x76, 0x01) => self.emit_table1(vec![0x77, 0x01, st.connection_priority.wire_byte()]),
            (0x78, 0x01) if p.len() >= 3 => {
                let prio = ConnectionPriority::from_byte(p[2]);
                let next_codec = if prio == ConnectionPriority::SoundQuality {
                    AudioCodec::Ldac
                } else {
                    AudioCodec::Aac
                };
                st.connection_priority = prio;
                st.codec = next_codec;
                self.emit_table1(vec![0x79, 0x01, prio.wire_byte()]);
                self.emit_table1(vec![0x16, 0x00, next_codec.wire_byte()]);
            }
            (0x76, 0x02) => self.emit_table1(vec![
                0x77,
                0x02,
                if st.dsee_extreme { 0x01 } else { 0x00 },
            ]),
            (0x78, 0x02) if p.len() >= 3 => {
                st.dsee_extreme = p[2] == 0x01;
                self.emit_table1(vec![0x79, 0x02, if st.dsee_extreme { 0x01 } else { 0x00 }]);
            }
            (0xA6, 0x01) => self.emit_table1(vec![
                0xA7,
                0x01,
                st.auto_power_off.wire_byte(),
                st.auto_power_off_last_timer_byte,
            ]),
            (0xA8, 0x01) if p.len() >= 4 => {
                if let Some(m) = AutoPowerOff::from_byte(p[2]) {
                    st.auto_power_off = m;
                }
                st.auto_power_off_last_timer_byte = p[3];
                self.emit_table1(vec![0xA9, 0x01, st.auto_power_off.wire_byte(), p[3]]);
            }
            (0xA6, 0x02) => self.emit_table1(vec![
                0xA7,
                0x02,
                if st.wearing_detection { 0x00 } else { 0x01 },
            ]),
            (0xA8, 0x02) if p.len() >= 3 => {
                st.wearing_detection = p[2] == 0x00;
                self.emit_table1(vec![0xA9, 0x02, p[2]]);
            }
            (0xB6, 0x01) => self.emit_table1(vec![
                0xB7,
                0x01,
                if st.voice_guidance_enabled { 0x00 } else { 0x01 },
                st.voice_guidance_language_code as u8,
            ]),
            (0xB8, 0x01) if p.len() >= 3 => {
                st.voice_guidance_enabled = p[2] == 0x00;
                self.emit_table1(vec![
                    0xB9,
                    0x01,
                    p[2],
                    st.voice_guidance_language_code as u8,
                ]);
            }
            _ => {}
        }
    }

    fn handle_v2_command(&self, table: MdrDataType, p: &[u8]) {
        if p.len() < 2 {
            return;
        }
        let op = p[0];
        let sub = p[1];
        let mut st = self.state.lock().unwrap();

        if table == MdrDataType::DataMdr {
            match (op, sub) {
                (0x00, _) => self.emit_table1(vec![0x01, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00]),
                (0x04, 0x01) => self.emit_table1(lp_string_reply(0x05, 0x01, &st.model_name)),
                (0x04, 0x02) => {
                    self.emit_table1(lp_string_reply(0x05, 0x02, &st.firmware_version))
                }
                (0x04, 0x03) => self.emit_table1(vec![
                    0x05,
                    0x03,
                    0x30,
                    st.hardware_color_code.unwrap_or(0x01) as u8,
                ]),
                (0x06, _) => {
                    let mut ids: Vec<i32> = st.t1_functions.iter().copied().collect();
                    ids.sort();
                    let mut out = vec![0x07, 0x00, ids.len() as u8];
                    for id in ids {
                        out.push(id as u8);
                        out.push(0x00);
                    }
                    self.emit_table1(out);
                }
                (0x12, 0x02) => self.emit_table1(vec![0x13, 0x02, st.codec.wire_byte()]),
                (0x22, 0x00) => self.emit_table1(vec![
                    0x23,
                    0x00,
                    st.battery.level.unwrap_or(78) as u8,
                    st.battery.charging_status.wire_byte(),
                ]),
                (0x22, 0x01) => {
                    let l = st
                        .battery
                        .left
                        .clone()
                        .unwrap_or_else(|| BatteryUnit::new(82, ChargingStatus::NotCharging));
                    let r = st
                        .battery
                        .right
                        .clone()
                        .unwrap_or_else(|| BatteryUnit::new(76, ChargingStatus::NotCharging));
                    self.emit_table1(vec![
                        0x23,
                        0x01,
                        l.level as u8,
                        l.charging_status.wire_byte(),
                        r.level as u8,
                        r.charging_status.wire_byte(),
                    ]);
                }
                (0x22, 0x02) => {
                    let c = st
                        .battery
                        .case_battery
                        .clone()
                        .unwrap_or_else(|| BatteryUnit::new(65, ChargingStatus::NotCharging));
                    self.emit_table1(vec![
                        0x23,
                        0x02,
                        c.level as u8,
                        c.charging_status.wire_byte(),
                    ]);
                }
                (0x24, 0x03) => {
                    drop(st);
                    self.close();
                }
                (0x26, 0x04 | 0x05) => self.emit_table1(vec![
                    0x27,
                    sub,
                    st.auto_power_off.wire_byte(),
                    st.auto_power_off_last_timer_byte,
                ]),
                (0x28, 0x04 | 0x05) if p.len() >= 4 => {
                    if let Some(m) = AutoPowerOff::from_byte(p[2]) {
                        st.auto_power_off = m;
                    }
                    st.auto_power_off_last_timer_byte = p[3];
                    self.emit_table1(vec![0x29, sub, st.auto_power_off.wire_byte(), p[3]]);
                }
                (0x50, 0x00) => self.emit_table1(build_eq_capability(&st.equalizer, 0x00)),
                (0x56, 0x00) => self.emit_table1(build_eq_reply(0x57, 0x00, &st.equalizer)),
                (0x58, 0x00) if p.len() >= 4 => {
                    apply_eq_set(&mut st.equalizer, p);
                    self.emit_table1(build_eq_reply(0x59, 0x00, &st.equalizer));
                }
                (0x66, 0x17 | 0x11) => {
                    self.emit_table1(build_v2_nc_reply(0x67, &st.noise_control, sub as i32))
                }
                (0x68, 0x17) if p.len() >= 7 => {
                    let on = p[3] == 0x01;
                    let ambient = p[4] == 0x01;
                    let voice = p[5] == 0x01;
                    let lvl = (p[6] as i32).clamp(1, 20);
                    let mode = if !on {
                        NoiseControlMode::Off
                    } else if ambient {
                        NoiseControlMode::AmbientSound
                    } else {
                        NoiseControlMode::NoiseCancelling
                    };
                    st.noise_control = NoiseControlState {
                        mode,
                        ambient_level: lvl,
                        focus_on_voice: voice,
                    };
                    self.emit_table1(build_v2_nc_reply(0x69, &st.noise_control, 0x17));
                }
                (0x68, 0x11) if p.len() >= 8 => {
                    let effect = p[2] == 0x01;
                    let nc_val = p[4];
                    let voice = p[6] == 0x01;
                    let asm_amt = (p[7] as i32).clamp(0, 20);
                    let mode = if !effect {
                        NoiseControlMode::Off
                    } else if nc_val != 0 && asm_amt == 0 {
                        NoiseControlMode::NoiseCancelling
                    } else {
                        NoiseControlMode::AmbientSound
                    };
                    let lvl = if asm_amt == 0 {
                        st.noise_control.ambient_level
                    } else {
                        asm_amt
                    };
                    st.noise_control = NoiseControlState {
                        mode,
                        ambient_level: lvl,
                        focus_on_voice: voice,
                    };
                    self.emit_table1(build_v2_nc_reply(0x69, &st.noise_control, 0x11));
                }
                (0x66, 0x30) => self.emit_table1(vec![0x67, 0x30, st.nc_amb_button_cycle.wire_value()]),
                (0x68, 0x30) if p.len() >= 3 => {
                    if let Some(cycle) = NcAmbButtonCycle::from_byte(p[2]) {
                        st.nc_amb_button_cycle = cycle;
                    }
                    self.emit_table1(vec![0x69, 0x30, st.nc_amb_button_cycle.wire_value()]);
                }
                (0xD0, slot) => {
                    if let Some(gs) = st.general_settings.iter().find(|g| g.id == slot as i32) {
                        let mut out = vec![0xD1, slot, 0x00, 0x01];
                        write_lp_string(&mut out, &gs.key);
                        write_lp_string(&mut out, &gs.summary_key);
                        self.emit_table1(out);
                    }
                }
                (0xD6, slot) => {
                    if let Some(gs) = st.general_settings.iter().find(|g| g.id == slot as i32) {
                        self.emit_table1(vec![0xD7, slot, 0x00, if gs.is_on { 0x00 } else { 0x01 }]);
                    }
                }
                (0xD8, slot) if p.len() >= 4 => {
                    let on = p[3] == 0x00;
                    for gs in &mut st.general_settings {
                        if gs.id == slot as i32 {
                            gs.is_on = on;
                        }
                    }
                    self.emit_table1(vec![0xD9, slot, 0x00, p[3]]);
                }
                (0xE6, 0x00) => self.emit_table1(vec![0xE7, 0x00, st.connection_priority.wire_byte()]),
                (0xE8, 0x00) if p.len() >= 3 => {
                    let prio = ConnectionPriority::from_byte(p[2]);
                    let next_codec = if prio == ConnectionPriority::SoundQuality {
                        AudioCodec::Ldac
                    } else {
                        AudioCodec::Aac
                    };
                    st.connection_priority = prio;
                    st.codec = next_codec;
                    self.emit_table1(vec![0xE9, 0x00, prio.wire_byte()]);
                    self.emit_table1(vec![0x15, 0x02, next_codec.wire_byte()]);
                }
                (0xE6, 0x01) => {
                    self.emit_table1(vec![0xE7, 0x01, if st.dsee_extreme { 0x01 } else { 0x00 }])
                }
                (0xE8, 0x01) if p.len() >= 3 => {
                    st.dsee_extreme = p[2] == 0x01;
                    self.emit_table1(vec![0xE9, 0x01, if st.dsee_extreme { 0x01 } else { 0x00 }]);
                }
                (0xF0, 0x03) => self.emit_table1(build_assignable_cap(0xF1, &st.assignable_settings)),
                (0xF0, 0x04) => self.emit_table1(vec![0xF1, 0x04, 0x00, 0x04, 0x30, 0x31, 0x32, 0xFF]),
                (0xF0, 0x0D) => self.emit_table1(vec![
                    0xF1, 0x0D, 0x00, 0x00, 0x02, 0x01, 0x00, 0x02, 0x00, 0x01, 0x02, 0x00, 0x02,
                    0x00, 0x01,
                ]),
                (0xF6, 0x01) => self.emit_table1(vec![
                    0xF7,
                    0x01,
                    if st.wearing_detection { 0x00 } else { 0x01 },
                ]),
                (0xF8, 0x01) if p.len() >= 3 => {
                    st.wearing_detection = p[2] == 0x00;
                    self.emit_table1(vec![0xF9, 0x01, p[2]]);
                }
                (0xF6, 0x03) => self.emit_table1(build_assignable_param(0xF7, &st.assignable_settings)),
                (0xF8, 0x03) if p.len() >= 3 => {
                    let count = p[2] as usize;
                    for (idx, setting) in st.assignable_settings.iter_mut().enumerate() {
                        if idx < count && 3 + idx < p.len() {
                            setting.preset = AssignablePreset::from_byte(p[3 + idx]);
                        }
                    }
                    self.emit_table1(build_assignable_param(0xF9, &st.assignable_settings));
                }
                (0xF6, 0x04) => self.emit_table1(vec![0xF7, 0x04, st.voice_assistant.wire_byte()]),
                (0xF8, 0x04) if p.len() >= 3 => {
                    st.voice_assistant = VoiceAssistant::from_byte(p[2]);
                    self.emit_table1(vec![0xF9, 0x04, st.voice_assistant.wire_byte()]);
                }
                (0xF6, 0x0C) => {
                    self.emit_table1(vec![0xF7, 0x0C, if st.speak_to_chat { 0x00 } else { 0x01 }])
                }
                (0xF8, 0x0C) if p.len() >= 3 => {
                    st.speak_to_chat = p[2] == 0x00;
                    self.emit_table1(vec![0xF9, 0x0C, p[2]]);
                }
                (0xFA, 0x0C) => self.emit_table1(vec![
                    0xFB,
                    0x0C,
                    st.speak_to_chat_sensitivity.wire_byte(),
                    st.speak_to_chat_timeout.wire_byte(),
                ]),
                (0xFC, 0x0C) if p.len() >= 4 => {
                    st.speak_to_chat_sensitivity = SpeakToChatSensitivity::from_byte(p[2]);
                    st.speak_to_chat_timeout = SpeakToChatTimeout::from_byte(p[3]);
                    self.emit_table1(vec![
                        0xFD,
                        0x0C,
                        st.speak_to_chat_sensitivity.wire_byte(),
                        st.speak_to_chat_timeout.wire_byte(),
                    ]);
                }
                (0xF6, 0x0D) => self.emit_table1(vec![
                    0xF7,
                    0x0D,
                    0x02,
                    st.quick_access_double_tap.0,
                    st.quick_access_triple_tap.0,
                ]),
                (0xF8, 0x0D) if p.len() >= 5 => {
                    st.quick_access_double_tap = QuickAccessService(p[3]);
                    st.quick_access_triple_tap = QuickAccessService(p[4]);
                    self.emit_table1(vec![0xF9, 0x0D, 0x02, p[3], p[4]]);
                }
                _ => {}
            }
        } else if table == MdrDataType::DataMdrNo2 {
            match (op, sub) {
                (0x06, _) => {
                    let mut ids: Vec<i32> = st.t2_functions.iter().copied().collect();
                    ids.sort();
                    let mut out = vec![0x07, 0x00, ids.len() as u8];
                    for id in ids {
                        out.push(id as u8);
                        out.push(0x00);
                    }
                    self.emit_table2(out);
                }
                (0x46, 0x01) => self.emit_table2(vec![
                    0x47,
                    0x01,
                    if st.voice_guidance_enabled { 0x00 } else { 0x01 },
                    st.voice_guidance_language_code as u8,
                ]),
                (0x48, 0x01) if p.len() >= 3 => {
                    st.voice_guidance_enabled = p[2] == 0x00;
                    self.emit_table2(vec![
                        0x49,
                        0x01,
                        p[2],
                        st.voice_guidance_language_code as u8,
                    ]);
                }
                (0x32, 0x02) => {
                    self.emit_table2(vec![0x33, 0x02, if st.pairing_mode { 0x01 } else { 0x00 }])
                }
                (0x34, 0x02) if p.len() >= 3 => {
                    st.pairing_mode = p[2] == 0x01;
                    self.emit_table2(vec![0x35, 0x02, p[2]]);
                }
                (0x36, 0x01) => self.emit_table2(vec![
                    0x37,
                    0x01,
                    if st.source_switch_enabled { 0x01 } else { 0x00 },
                ]),
                (0x38, 0x01) if p.len() >= 3 => {
                    st.source_switch_enabled = p[2] != 0x00;
                    self.emit_table2(vec![0x39, 0x01, p[2]]);
                }
                (0x36, 0x02) => self.emit_table2(build_paired_devices_reply(0x37, &st.paired_devices)),
                (0x3C, 0x01) if p.len() >= 19 => {
                    let target = String::from_utf8_lossy(&p[2..19]).to_string();
                    for d in &mut st.paired_devices {
                        d.is_playing = d.address.eq_ignore_ascii_case(&target) && d.is_connected;
                    }
                    let mut reply = vec![0x3D, 0x01, 0x00];
                    reply.extend_from_slice(&p[2..19]);
                    self.emit_table2(reply);
                }
                (0x3C, 0x02) if p.len() >= 20 => {
                    let action = p[2];
                    let target = String::from_utf8_lossy(&p[3..20]).to_string();
                    match action {
                        0x00 => {
                            for d in &mut st.paired_devices {
                                if d.address.eq_ignore_ascii_case(&target) {
                                    d.is_connected = false;
                                    d.is_playing = false;
                                }
                            }
                        }
                        0x01 => {
                            for d in &mut st.paired_devices {
                                if d.address.eq_ignore_ascii_case(&target) {
                                    d.is_connected = true;
                                }
                            }
                        }
                        0x02 => {
                            st.paired_devices
                                .retain(|d| !d.address.eq_ignore_ascii_case(&target));
                        }
                        _ => {}
                    }
                    self.emit_table2(build_paired_devices_reply(0x39, &st.paired_devices));
                }
                _ => {}
            }
        }
    }
}

impl BluetoothTransport for SimulatedSonyTransport {
    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    fn connect(&self, _address: &str, _service_uuid: &str) -> Result<(), ProtocolError> {
        self.decoder.clear();
        *self.pending_buf.lock().unwrap() = (Vec::new(), 0);
        *self.sim_tx_seq.lock().unwrap() = 0;
        {
            let (lock, _) = &self.rx_queue;
            let mut q = lock.lock().unwrap();
            q.chunks.clear();
            q.closed = false;
        }
        *self.state.lock().unwrap() = initial_state_for(self.profile);
        self.connected.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize, ProtocolError> {
        if !self.is_connected() {
            return Err(ProtocolError::NotConnected);
        }
        let mut pending = self.pending_buf.lock().unwrap();
        while pending.1 >= pending.0.len() {
            let (lock, cv) = &self.rx_queue;
            let mut q = lock.lock().unwrap();
            while q.chunks.is_empty() && !q.closed {
                q = cv.wait(q).unwrap();
            }
            if q.chunks.is_empty() && q.closed {
                return Err(ProtocolError::NotConnected);
            }
            pending.0 = q.chunks.pop_front().unwrap();
            pending.1 = 0;
        }
        let avail = pending.0.len() - pending.1;
        let to_copy = avail.min(buf.len());
        buf[..to_copy].copy_from_slice(&pending.0[pending.1..pending.1 + to_copy]);
        pending.1 += to_copy;
        Ok(to_copy)
    }

    fn write(&self, bytes: &[u8]) -> Result<(), ProtocolError> {
        if !self.is_connected() {
            return Err(ProtocolError::NotConnected);
        }
        let proto = self.state.lock().unwrap().protocol_version;
        for frame in self.decoder.feed(bytes) {
            if frame.frame_type == MdrDataType::Ack {
                *self.sim_tx_seq.lock().unwrap() = frame.sequence & 0x01;
                continue;
            }
            let ack = MdrFrame {
                frame_type: MdrDataType::Ack,
                sequence: 1 - (frame.sequence & 0x01),
                payload: Vec::new(),
            };
            if let Ok(encoded) = encode_frame(&ack) {
                self.enqueue_rx(encoded);
            }
            if proto == MdrProtocolVersion::V1 {
                self.handle_v1_command(frame.frame_type, &frame.payload);
            } else {
                self.handle_v2_command(frame.frame_type, &frame.payload);
            }
        }
        Ok(())
    }

    fn close(&self) {
        self.connected.store(false, Ordering::SeqCst);
        let (lock, cv) = &self.rx_queue;
        let mut q = lock.lock().unwrap();
        q.closed = true;
        cv.notify_all();
    }
}

// MARK: - Simulated Bose / Soundcore / Galaxy Buds Transports

pub struct SimulatedBoseTransport {
    connected: AtomicBool,
    rx_queue: (Mutex<RxQueue>, Condvar),
    pending_buf: Mutex<(Vec<u8>, usize)>,
    decoder: BoseBmapStreamDecoder,
    state: Mutex<HeadphoneState>,
}

impl SimulatedBoseTransport {
    pub fn new() -> Self {
        Self {
            connected: AtomicBool::new(false),
            rx_queue: (
                Mutex::new(RxQueue {
                    chunks: VecDeque::new(),
                    closed: false,
                }),
                Condvar::new(),
            ),
            pending_buf: Mutex::new((Vec::new(), 0)),
            decoder: BoseBmapStreamDecoder::new(),
            state: Mutex::new(HeadphoneState {
                model_name: "Bose QC Ultra".to_string(),
                firmware_version: "1.6.7".to_string(),
                bluetooth_address: "AA:BB:CC:22:00:01".to_string(),
                protocol_version: MdrProtocolVersion::BoseBmap,
                battery: BatteryState {
                    level: Some(81),
                    charging_status: ChargingStatus::NotCharging,
                    ..Default::default()
                },
                noise_control: NoiseControlState::default(),
                auto_power_off: AutoPowerOff::After30Min,
                voice_guidance_enabled: true,
                custom_capabilities: Some(HeadphoneCapabilities::bose_bmap()),
                ..Default::default()
            }),
        }
    }

    fn emit_status(&self, block: u8, func: u8, payload: Vec<u8>) {
        if let Ok(bytes) = bose_bmap::encode_frame(&BoseBmapFrame {
            block,
            func,
            op: bose_bmap::OP_STATUS,
            payload,
        }) {
            let (lock, cv) = &self.rx_queue;
            let mut q = lock.lock().unwrap();
            q.chunks.push_back(bytes);
            cv.notify_all();
        }
    }
}

impl BluetoothTransport for SimulatedBoseTransport {
    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    fn connect(&self, address: &str, _service_uuid: &str) -> Result<(), ProtocolError> {
        self.decoder.clear();
        *self.pending_buf.lock().unwrap() = (Vec::new(), 0);
        {
            let (lock, _) = &self.rx_queue;
            let mut q = lock.lock().unwrap();
            q.chunks.clear();
            q.closed = false;
        }
        self.state.lock().unwrap().bluetooth_address = address.to_string();
        self.connected.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize, ProtocolError> {
        read_from_queue(&self.connected, &self.rx_queue, &self.pending_buf, buf)
    }

    fn write(&self, bytes: &[u8]) -> Result<(), ProtocolError> {
        if !self.is_connected() {
            return Err(ProtocolError::NotConnected);
        }
        for frame in self.decoder.feed(bytes) {
            let mut st = self.state.lock().unwrap();
            match frame.op {
                bose_bmap::OP_GET => match (frame.block, frame.func) {
                    (bose_bmap::BLOCK_PRODUCT_INFO, bose_bmap::FUNC_FIRMWARE_VERSION) => self
                        .emit_status(
                            frame.block,
                            frame.func,
                            st.firmware_version.as_bytes().to_vec(),
                        ),
                    (bose_bmap::BLOCK_SETTINGS, bose_bmap::FUNC_DEVICE_NAME) => {
                        self.emit_status(frame.block, frame.func, st.model_name.as_bytes().to_vec())
                    }
                    (bose_bmap::BLOCK_SETTINGS, bose_bmap::FUNC_VOICE_PROMPTS) => self.emit_status(
                        frame.block,
                        frame.func,
                        vec![if st.voice_guidance_enabled { 0x01 } else { 0x00 }],
                    ),
                    (bose_bmap::BLOCK_SETTINGS, bose_bmap::FUNC_AUTO_OFF) => {
                        self.emit_status(frame.block, frame.func, vec![30])
                    }
                    (bose_bmap::BLOCK_SETTINGS, bose_bmap::FUNC_NOISE_CONTROL) => {
                        let b = match st.noise_control.mode {
                            NoiseControlMode::Off => 0x00,
                            NoiseControlMode::NoiseCancelling => 0x01,
                            NoiseControlMode::AmbientSound => 0x03,
                        };
                        self.emit_status(frame.block, frame.func, vec![b]);
                    }
                    (bose_bmap::BLOCK_STATUS, bose_bmap::FUNC_BATTERY) => self.emit_status(
                        frame.block,
                        frame.func,
                        vec![st.battery.level.unwrap_or(81) as u8],
                    ),
                    (bose_bmap::BLOCK_DEVICE_MANAGEMENT, bose_bmap::FUNC_PAIRED_DEVICES) => self
                        .emit_status(
                            frame.block,
                            frame.func,
                            vec![0x01, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66],
                        ),
                    _ => {}
                },
                bose_bmap::OP_SET | bose_bmap::OP_SETGET => match (frame.block, frame.func) {
                    (bose_bmap::BLOCK_SETTINGS, bose_bmap::FUNC_NOISE_CONTROL) => {
                        if let Some(&b) = frame.payload.first() {
                            st.noise_control.mode = match b {
                                0x00 => NoiseControlMode::Off,
                                0x01 => NoiseControlMode::NoiseCancelling,
                                0x03 => NoiseControlMode::AmbientSound,
                                _ => st.noise_control.mode,
                            };
                        }
                        if frame.op == bose_bmap::OP_SETGET {
                            let b = match st.noise_control.mode {
                                NoiseControlMode::Off => 0x00,
                                NoiseControlMode::NoiseCancelling => 0x01,
                                NoiseControlMode::AmbientSound => 0x03,
                            };
                            self.emit_status(frame.block, frame.func, vec![b]);
                        }
                    }
                    (bose_bmap::BLOCK_SETTINGS, bose_bmap::FUNC_VOICE_PROMPTS) => {
                        if let Some(&b) = frame.payload.first() {
                            st.voice_guidance_enabled = b != 0;
                        }
                        if frame.op == bose_bmap::OP_SETGET {
                            self.emit_status(
                                frame.block,
                                frame.func,
                                vec![if st.voice_guidance_enabled { 0x01 } else { 0x00 }],
                            );
                        }
                    }
                    (bose_bmap::BLOCK_SETTINGS, bose_bmap::FUNC_AUTO_OFF) => {
                        if let Some(&m) = frame.payload.first() {
                            st.auto_power_off = match m {
                                0 => AutoPowerOff::Never,
                                5 => AutoPowerOff::After5Min,
                                15 => AutoPowerOff::After15Min,
                                30 => AutoPowerOff::After30Min,
                                60 => AutoPowerOff::After1Hour,
                                180 => AutoPowerOff::After3Hours,
                                _ => AutoPowerOff::After30Min,
                            };
                            if frame.op == bose_bmap::OP_SETGET {
                                self.emit_status(frame.block, frame.func, vec![m]);
                            }
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        Ok(())
    }

    fn close(&self) {
        self.connected.store(false, Ordering::SeqCst);
        let (lock, cv) = &self.rx_queue;
        let mut q = lock.lock().unwrap();
        q.closed = true;
        cv.notify_all();
    }
}

pub struct SimulatedSoundcoreTransport {
    connected: AtomicBool,
    rx_queue: (Mutex<RxQueue>, Condvar),
    pending_buf: Mutex<(Vec<u8>, usize)>,
    decoder: SoundcoreStreamDecoder,
    battery_level: Mutex<u8>,
    nc_mode: Mutex<NoiseControlMode>,
    eq_bands: Mutex<Vec<i32>>,
}

impl SimulatedSoundcoreTransport {
    pub fn new() -> Self {
        Self {
            connected: AtomicBool::new(false),
            rx_queue: (
                Mutex::new(RxQueue {
                    chunks: VecDeque::new(),
                    closed: false,
                }),
                Condvar::new(),
            ),
            pending_buf: Mutex::new((Vec::new(), 0)),
            decoder: SoundcoreStreamDecoder::new(),
            battery_level: Mutex::new(84),
            nc_mode: Mutex::new(NoiseControlMode::NoiseCancelling),
            eq_bands: Mutex::new(vec![0; 8]),
        }
    }

    fn send_inbound(&self, group: u8, command: u8, payload: Vec<u8>) {
        let encoded = soundcore::encode_packet(&SoundcorePacket {
            group,
            command,
            payload,
            is_inbound: true,
        });
        let (lock, cv) = &self.rx_queue;
        let mut q = lock.lock().unwrap();
        q.chunks.push_back(encoded);
        cv.notify_all();
    }
}

impl BluetoothTransport for SimulatedSoundcoreTransport {
    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    fn connect(&self, _address: &str, _service_uuid: &str) -> Result<(), ProtocolError> {
        self.decoder.clear();
        *self.pending_buf.lock().unwrap() = (Vec::new(), 0);
        {
            let (lock, _) = &self.rx_queue;
            let mut q = lock.lock().unwrap();
            q.chunks.clear();
            q.closed = false;
        }
        self.connected.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize, ProtocolError> {
        read_from_queue(&self.connected, &self.rx_queue, &self.pending_buf, buf)
    }

    fn write(&self, bytes: &[u8]) -> Result<(), ProtocolError> {
        if !self.is_connected() {
            return Err(ProtocolError::NotConnected);
        }
        for pkt in self.decoder.feed(bytes) {
            if pkt.is_inbound {
                continue;
            }
            match (pkt.group, pkt.command) {
                (soundcore::GROUP_INFO, soundcore::CMD_GET_STATE) => {
                    let mut p = vec![0u8; soundcore::STATE_PAYLOAD_MIN_OVER_EAR_SIZE];
                    p[soundcore::STATE_OFFSET_BATTERY_LEVEL] = *self.battery_level.lock().unwrap();
                    p[14] = 0xFE;
                    p[15] = 0xFE;
                    let bands = self.eq_bands.lock().unwrap();
                    for i in 0..8 {
                        p[soundcore::STATE_OFFSET_EQ_BANDS + i] =
                            ((bands.get(i).copied().unwrap_or(0).clamp(-6, 6) * 10) + 120) as u8;
                    }
                    p[soundcore::STATE_OFFSET_SOUND_MODE] =
                        soundcore::encode_sound_mode_byte(*self.nc_mode.lock().unwrap());
                    self.send_inbound(soundcore::GROUP_INFO, soundcore::CMD_GET_STATE, p);
                }
                (soundcore::GROUP_INFO, soundcore::CMD_GET_BATTERY) => {
                    self.send_inbound(
                        soundcore::GROUP_INFO,
                        soundcore::CMD_GET_BATTERY,
                        vec![*self.battery_level.lock().unwrap(), 0x00],
                    );
                }
                (soundcore::GROUP_ANC, soundcore::CMD_SET_SOUND_MODE) => {
                    if let Some(&b) = pkt.payload.first() {
                        if let Some(m) = soundcore::decode_sound_mode_byte(b) {
                            *self.nc_mode.lock().unwrap() = m;
                        }
                    }
                    let m = *self.nc_mode.lock().unwrap();
                    self.send_inbound(
                        soundcore::GROUP_ANC,
                        soundcore::CMD_SET_SOUND_MODE,
                        vec![
                            soundcore::encode_sound_mode_byte(m),
                            0x00,
                            if m == NoiseControlMode::AmbientSound {
                                0x01
                            } else {
                                0x00
                            },
                            0x00,
                        ],
                    );
                }
                (soundcore::GROUP_EQ, soundcore::CMD_SET_EQ) => {
                    let off = if pkt.payload.len() >= 10 {
                        2
                    } else if pkt.payload.len() >= 8 {
                        0
                    } else {
                        usize::MAX
                    };
                    if off != usize::MAX {
                        let mut bands = self.eq_bands.lock().unwrap();
                        *bands = (0..8)
                            .map(|i| ((pkt.payload[off + i] as i32 - 120) / 10).clamp(-6, 6))
                            .collect();
                    }
                    let bands = self.eq_bands.lock().unwrap();
                    let mut reply = vec![0xFE, 0xFE];
                    for &b in bands.iter() {
                        reply.push(((b.clamp(-6, 6) * 10) + 120) as u8);
                    }
                    self.send_inbound(soundcore::GROUP_EQ, soundcore::CMD_SET_EQ, reply);
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn close(&self) {
        self.connected.store(false, Ordering::SeqCst);
        let (lock, cv) = &self.rx_queue;
        let mut q = lock.lock().unwrap();
        q.closed = true;
        cv.notify_all();
    }
}

pub struct SimulatedGalaxyBudsTransport {
    connected: AtomicBool,
    rx_queue: (Mutex<RxQueue>, Condvar),
    pending_buf: Mutex<(Vec<u8>, usize)>,
    decoder: GalaxyBudsStreamDecoder,
    nc_mode: Mutex<NoiseControlMode>,
    eq_idx: Mutex<u8>,
    touch_locked: Mutex<bool>,
    seamless: Mutex<bool>,
}

impl SimulatedGalaxyBudsTransport {
    pub fn new() -> Self {
        Self {
            connected: AtomicBool::new(false),
            rx_queue: (
                Mutex::new(RxQueue {
                    chunks: VecDeque::new(),
                    closed: false,
                }),
                Condvar::new(),
            ),
            pending_buf: Mutex::new((Vec::new(), 0)),
            decoder: GalaxyBudsStreamDecoder::new(),
            nc_mode: Mutex::new(NoiseControlMode::NoiseCancelling),
            eq_idx: Mutex::new(0),
            touch_locked: Mutex::new(false),
            seamless: Mutex::new(true),
        }
    }

    fn emit_extended_status(&self) {
        let nc_byte = match *self.nc_mode.lock().unwrap() {
            NoiseControlMode::Off => 0x00,
            NoiseControlMode::NoiseCancelling => 0x01,
            NoiseControlMode::AmbientSound => 0x02,
        };
        let payload = vec![
            0x0C,
            0x00,
            88,
            84,
            0x01,
            0x00,
            0x11,
            72,
            *self.eq_idx.lock().unwrap(),
            10,
            nc_byte,
            if *self.touch_locked.lock().unwrap() {
                0x01
            } else {
                0x00
            },
            if *self.seamless.lock().unwrap() {
                0x01
            } else {
                0x00
            },
        ];
        if let Ok(encoded) = galaxy_buds::encode_frame(&GalaxyBudsFrame {
            msg_id: galaxy_buds::MSG_EXTENDED_STATUS_UPDATED,
            payload,
            is_response: false,
        }) {
            let (lock, cv) = &self.rx_queue;
            let mut q = lock.lock().unwrap();
            q.chunks.push_back(encoded);
            cv.notify_all();
        }
    }
}

impl BluetoothTransport for SimulatedGalaxyBudsTransport {
    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    fn connect(&self, _address: &str, _service_uuid: &str) -> Result<(), ProtocolError> {
        self.decoder.clear();
        *self.pending_buf.lock().unwrap() = (Vec::new(), 0);
        {
            let (lock, _) = &self.rx_queue;
            let mut q = lock.lock().unwrap();
            q.chunks.clear();
            q.closed = false;
        }
        self.connected.store(true, Ordering::SeqCst);
        self.emit_extended_status();
        Ok(())
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize, ProtocolError> {
        read_from_queue(&self.connected, &self.rx_queue, &self.pending_buf, buf)
    }

    fn write(&self, bytes: &[u8]) -> Result<(), ProtocolError> {
        if !self.is_connected() {
            return Err(ProtocolError::NotConnected);
        }
        for frame in self.decoder.feed(bytes) {
            match frame.msg_id {
                galaxy_buds::MSG_SET_NOISE_CONTROLS => {
                    if let Some(&b) = frame.payload.first() {
                        *self.nc_mode.lock().unwrap() = match b {
                            0x00 => NoiseControlMode::Off,
                            0x01 => NoiseControlMode::NoiseCancelling,
                            0x02 => NoiseControlMode::AmbientSound,
                            _ => *self.nc_mode.lock().unwrap(),
                        };
                    }
                    self.emit_extended_status();
                }
                galaxy_buds::MSG_EQUALIZER => {
                    if let Some(&b) = frame.payload.first() {
                        *self.eq_idx.lock().unwrap() = b.min(5);
                    }
                    self.emit_extended_status();
                }
                galaxy_buds::MSG_LOCK_TOUCHPAD => {
                    if let Some(&b) = frame.payload.first() {
                        *self.touch_locked.lock().unwrap() = b != 0;
                    }
                    self.emit_extended_status();
                }
                galaxy_buds::MSG_SET_SEAMLESS_CONNECTION => {
                    if let Some(&b) = frame.payload.first() {
                        *self.seamless.lock().unwrap() = b != 0;
                    }
                    self.emit_extended_status();
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn close(&self) {
        self.connected.store(false, Ordering::SeqCst);
        let (lock, cv) = &self.rx_queue;
        let mut q = lock.lock().unwrap();
        q.closed = true;
        cv.notify_all();
    }
}

fn read_from_queue(
    connected: &AtomicBool,
    rx_queue: &(Mutex<RxQueue>, Condvar),
    pending_buf: &Mutex<(Vec<u8>, usize)>,
    buf: &mut [u8],
) -> Result<usize, ProtocolError> {
    if !connected.load(Ordering::SeqCst) {
        return Err(ProtocolError::NotConnected);
    }
    let mut pending = pending_buf.lock().unwrap();
    while pending.1 >= pending.0.len() {
        let (lock, cv) = rx_queue;
        let mut q = lock.lock().unwrap();
        while q.chunks.is_empty() && !q.closed {
            q = cv.wait(q).unwrap();
        }
        if q.chunks.is_empty() && q.closed {
            return Err(ProtocolError::NotConnected);
        }
        pending.0 = q.chunks.pop_front().unwrap();
        pending.1 = 0;
    }
    let avail = pending.0.len() - pending.1;
    let to_copy = avail.min(buf.len());
    buf[..to_copy].copy_from_slice(&pending.0[pending.1..pending.1 + to_copy]);
    pending.1 += to_copy;
    Ok(to_copy)
}

// MARK: - Helpers

fn lp_string_reply(op: u8, sub: u8, text: &str) -> Vec<u8> {
    let mut out = vec![op, sub];
    write_lp_string(&mut out, text);
    out
}

fn write_lp_string(out: &mut Vec<u8>, text: &str) {
    let bytes = text.as_bytes();
    let len = bytes.len().min(255);
    out.push(len as u8);
    out.extend_from_slice(&bytes[..len]);
}

fn build_eq_capability(eq: &EqualizerState, sub: u8) -> Vec<u8> {
    let raw_bands = if eq.has_clear_bass {
        eq.band_count + 1
    } else {
        eq.band_count
    };
    let mut out = vec![
        0x51,
        sub,
        raw_bands as u8,
        eq.step_count as u8,
        eq.available_preset_codes.len() as u8,
    ];
    for &code in &eq.available_preset_codes {
        out.push(code as u8);
        out.push(0);
    }
    out
}

fn build_eq_reply(op: u8, sub: u8, eq: &EqualizerState) -> Vec<u8> {
    let zero_offset = eq.step_count / 2;
    let max_val = eq.step_count - 1;
    let total = (if eq.has_clear_bass { 1 } else { 0 }) + eq.bands.len();
    let mut out = vec![op, sub, eq.preset_code as u8, total as u8];
    if eq.has_clear_bass {
        out.push((eq.clear_bass + zero_offset).clamp(0, max_val) as u8);
    }
    for &b in &eq.bands {
        out.push((b + zero_offset).clamp(0, max_val) as u8);
    }
    out
}

fn apply_eq_set(eq: &mut EqualizerState, p: &[u8]) {
    let preset = p[2] as i32;
    let count = p[3] as usize;
    let zero_offset = eq.step_count / 2;
    if count >= 1 && p.len() >= 4 + count {
        if eq.has_clear_bass {
            eq.preset_code = preset;
            eq.clear_bass = (p[4] as i32) - zero_offset;
            eq.bands = (0..count - 1)
                .map(|i| (p[5 + i] as i32) - zero_offset)
                .collect();
        } else {
            eq.preset_code = preset;
            eq.clear_bass = 0;
            eq.bands = (0..count).map(|i| (p[4 + i] as i32) - zero_offset).collect();
        }
    } else {
        eq.preset_code = preset;
    }
}

fn build_v1_nc_reply(op: u8, nc: &NoiseControlState) -> Vec<u8> {
    let effect = if nc.mode == NoiseControlMode::Off {
        0x00
    } else {
        0x01
    };
    let nc_val = if nc.mode == NoiseControlMode::NoiseCancelling {
        0x02
    } else {
        0x00
    };
    let asm_amt = if nc.mode == NoiseControlMode::AmbientSound {
        nc.ambient_level.clamp(1, 20) as u8
    } else {
        0x00
    };
    vec![
        op,
        0x02,
        effect,
        0x02,
        nc_val,
        0x01,
        if nc.focus_on_voice { 0x01 } else { 0x00 },
        asm_amt,
    ]
}

fn build_v2_nc_reply(op: u8, nc: &NoiseControlState, subtype: i32) -> Vec<u8> {
    if subtype == 0x11 {
        return build_v1_nc_reply(op, nc);
    }
    vec![
        op,
        0x17,
        0x01,
        if nc.mode == NoiseControlMode::Off {
            0x00
        } else {
            0x01
        },
        if nc.mode == NoiseControlMode::AmbientSound {
            0x01
        } else {
            0x00
        },
        if nc.focus_on_voice { 0x01 } else { 0x00 },
        nc.ambient_level.clamp(1, 20) as u8,
    ]
}

fn build_assignable_cap(op: u8, settings: &[AssignableSetting]) -> Vec<u8> {
    let mut out = vec![op, 0x03, settings.len() as u8];
    for s in settings {
        let opts = if s.available_presets.is_empty() {
            vec![
                AssignablePreset::AmbientSoundControl,
                AssignablePreset::PlaybackControl,
                AssignablePreset::VolumeControl,
                AssignablePreset::NoFunction,
            ]
        } else {
            s.available_presets.clone()
        };
        out.push(s.key_side as u8);
        out.push(s.preset.wire_byte());
        out.push(opts.len() as u8);
        for o in opts {
            out.push(o.wire_byte());
        }
    }
    out
}

fn build_assignable_param(op: u8, settings: &[AssignableSetting]) -> Vec<u8> {
    let mut out = vec![op, 0x03, settings.len() as u8];
    for s in settings {
        out.push(s.preset.wire_byte());
    }
    out
}

fn build_paired_devices_reply(op: u8, devices: &[PairedDevice]) -> Vec<u8> {
    let mut out = vec![op, 0x02, devices.len() as u8];
    let mut playing_idx = 0u8;
    for (idx, dev) in devices.iter().enumerate() {
        let status = if dev.is_connected { (idx + 1) as u8 } else { 0 };
        if dev.is_playing {
            playing_idx = status;
        }
        let mut mac = [b'0'; 17];
        for (i, b) in dev.address.to_uppercase().bytes().take(17).enumerate() {
            mac[i] = b;
        }
        out.extend_from_slice(&mac);
        out.push(status);
        out.push(((dev.class_of_device >> 16) & 0xFF) as u8);
        out.push(((dev.class_of_device >> 8) & 0xFF) as u8);
        out.push((dev.class_of_device & 0xFF) as u8);
        write_lp_string(&mut out, &dev.name);
    }
    out.push(playing_idx);
    out
}

pub fn initial_state_for(profile: SimulatedProfile) -> HeadphoneState {
    let default_presets = vec![
        0x00, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0xA0, 0xA1, 0xA2,
    ];
    let default_paired = vec![
        PairedDevice {
            address: "11:22:33:44:55:66".into(),
            name: "Pixel 9 Pro".into(),
            is_connected: true,
            is_playing: false,
            class_of_device: 0x0204,
        },
        PairedDevice {
            address: "AA:11:BB:22:CC:33".into(),
            name: "Windows Workstation".into(),
            is_connected: true,
            is_playing: true,
            class_of_device: 0x010C,
        },
        PairedDevice {
            address: "99:88:77:66:55:44".into(),
            name: "MacBook Pro".into(),
            is_connected: false,
            is_playing: false,
            class_of_device: 0x010C,
        },
    ];
    let set = |vals: &[i32]| vals.iter().copied().collect();

    match profile {
        SimulatedProfile::Wh1000Xm5 => HeadphoneState {
            model_name: "WH-1000XM5".into(),
            firmware_version: "2.4.1".into(),
            hardware_color_code: Some(0x01),
            detected_finish: HeadphoneFinish::ObsidianBlack,
            bluetooth_address: "AA:BB:CC:11:22:33".into(),
            protocol_version: MdrProtocolVersion::V2,
            has_table_2: true,
            t1_functions: set(&[
                0x12, 0x20, 0x23, 0x25, 0x50, 0x6B, 0x69, 0xD1, 0xD2, 0xE1, 0xE2, 0xF1, 0xF4, 0xFC,
                0xFD,
            ]),
            t2_functions: set(&[0x31, 0x32, 0x41]),
            battery: BatteryState {
                level: Some(78),
                charging_status: ChargingStatus::NotCharging,
                ..Default::default()
            },
            codec: AudioCodec::Ldac,
            equalizer: EqualizerState {
                available_preset_codes: default_presets,
                ..Default::default()
            },
            general_settings: vec![
                GeneralSetting {
                    id: 0xD1,
                    key: "TOUCH_PANEL_SETTING".into(),
                    summary_key: String::new(),
                    is_on: true,
                },
                GeneralSetting {
                    id: 0xD2,
                    key: "MULTIPOINT_SETTING".into(),
                    summary_key: "MULTIPOINT_SETTING_SUMMARY_LDAC_AVAILABLE".into(),
                    is_on: true,
                },
            ],
            paired_devices: default_paired,
            ..Default::default()
        },
        SimulatedProfile::Wh1000Xm4 => HeadphoneState {
            model_name: "WH-1000XM4".into(),
            firmware_version: "2.5.0".into(),
            hardware_color_code: Some(0x03),
            detected_finish: HeadphoneFinish::PlatinumSilver,
            bluetooth_address: "AA:BB:CC:11:22:44".into(),
            protocol_version: MdrProtocolVersion::V2,
            has_table_2: true,
            t1_functions: set(&[
                0x12, 0x20, 0x23, 0x25, 0x50, 0x61, 0x69, 0xD1, 0xD2, 0xE1, 0xE2, 0xF1, 0xF4, 0xFC,
            ]),
            t2_functions: set(&[0x31, 0x32, 0x41]),
            battery: BatteryState {
                level: Some(64),
                charging_status: ChargingStatus::NotCharging,
                ..Default::default()
            },
            codec: AudioCodec::Ldac,
            noise_control: NoiseControlState {
                mode: NoiseControlMode::NoiseCancelling,
                ambient_level: 12,
                focus_on_voice: false,
            },
            equalizer: EqualizerState {
                available_preset_codes: default_presets,
                ..Default::default()
            },
            general_settings: vec![
                GeneralSetting {
                    id: 0xD1,
                    key: "TOUCH_PANEL_SETTING".into(),
                    summary_key: String::new(),
                    is_on: true,
                },
                GeneralSetting {
                    id: 0xD2,
                    key: "MULTIPOINT_SETTING".into(),
                    summary_key: String::new(),
                    is_on: true,
                },
            ],
            paired_devices: default_paired,
            ..Default::default()
        },
        SimulatedProfile::WhCh720n => HeadphoneState {
            model_name: "WH-CH720N".into(),
            firmware_version: "1.0.9".into(),
            hardware_color_code: Some(0x05),
            detected_finish: HeadphoneFinish::MidnightBlue,
            bluetooth_address: "AA:BB:CC:11:22:72".into(),
            protocol_version: MdrProtocolVersion::V2,
            has_table_2: true,
            t1_functions: set(&[
                0x12, 0x20, 0x23, 0x25, 0x50, 0x68, 0x69, 0xD2, 0xE1, 0xE2, 0xF4,
            ]),
            t2_functions: set(&[0x31, 0x32, 0x41]),
            battery: BatteryState {
                level: Some(85),
                charging_status: ChargingStatus::NotCharging,
                ..Default::default()
            },
            codec: AudioCodec::Aac,
            connection_priority: ConnectionPriority::StableConnection,
            equalizer: EqualizerState {
                available_preset_codes: default_presets,
                ..Default::default()
            },
            general_settings: vec![GeneralSetting {
                id: 0xD2,
                key: "MULTIPOINT_SETTING".into(),
                summary_key: String::new(),
                is_on: true,
            }],
            paired_devices: default_paired,
            ..Default::default()
        },
        SimulatedProfile::Wf1000Xm5 => HeadphoneState {
            model_name: "WF-1000XM5".into(),
            firmware_version: "3.3.1".into(),
            hardware_color_code: Some(0x0A),
            detected_finish: HeadphoneFinish::ChampagneGold,
            bluetooth_address: "AA:BB:CC:11:22:55".into(),
            protocol_version: MdrProtocolVersion::V2,
            has_table_2: true,
            t1_functions: set(&[
                0x12, 0x21, 0x22, 0x23, 0x25, 0x50, 0x6B, 0x69, 0xD2, 0xE1, 0xE2, 0xF1, 0xF3, 0xFC,
                0xFD,
            ]),
            t2_functions: set(&[0x31, 0x32, 0x41]),
            battery: BatteryState {
                level: Some(76),
                charging_status: ChargingStatus::NotCharging,
                left: Some(BatteryUnit::new(82, ChargingStatus::NotCharging)),
                right: Some(BatteryUnit::new(76, ChargingStatus::NotCharging)),
                case_battery: Some(BatteryUnit::new(65, ChargingStatus::NotCharging)),
            },
            codec: AudioCodec::Ldac,
            equalizer: EqualizerState {
                available_preset_codes: default_presets,
                ..Default::default()
            },
            assignable_settings: vec![
                AssignableSetting {
                    key_side: 0,
                    preset: AssignablePreset::AmbientSoundControl,
                    available_presets: vec![
                        AssignablePreset::AmbientSoundControl,
                        AssignablePreset::PlaybackControl,
                        AssignablePreset::VolumeControl,
                        AssignablePreset::NoFunction,
                    ],
                },
                AssignableSetting {
                    key_side: 1,
                    preset: AssignablePreset::PlaybackControl,
                    available_presets: vec![
                        AssignablePreset::PlaybackControl,
                        AssignablePreset::AmbientSoundControl,
                        AssignablePreset::VolumeControl,
                        AssignablePreset::NoFunction,
                    ],
                },
            ],
            quick_access_double_tap: QuickAccessService(0x01),
            quick_access_triple_tap: QuickAccessService(0x00),
            quick_access_double_options: vec![QuickAccessService(0x00), QuickAccessService(0x01)],
            quick_access_triple_options: vec![QuickAccessService(0x00), QuickAccessService(0x01)],
            general_settings: vec![GeneralSetting {
                id: 0xD2,
                key: "MULTIPOINT_SETTING".into(),
                summary_key: String::new(),
                is_on: true,
            }],
            paired_devices: default_paired,
            ..Default::default()
        },
        SimulatedProfile::Wh1000Xm3 => HeadphoneState {
            model_name: "WH-1000XM3".into(),
            firmware_version: "4.5.2".into(),
            hardware_color_code: Some(0x01),
            detected_finish: HeadphoneFinish::ObsidianBlack,
            bluetooth_address: "AA:BB:CC:11:22:30".into(),
            protocol_version: MdrProtocolVersion::V1,
            has_table_2: false,
            t1_functions: set(&[0x13, 0x21, 0x24, 0x25, 0x32, 0x51, 0x72, 0x82, 0xC1, 0xD2]),
            battery: BatteryState {
                level: Some(72),
                charging_status: ChargingStatus::NotCharging,
                ..Default::default()
            },
            codec: AudioCodec::Ldac,
            equalizer: EqualizerState {
                available_preset_codes: default_presets,
                ..Default::default()
            },
            ..Default::default()
        },
        SimulatedProfile::Wf1000Xm3 => HeadphoneState {
            model_name: "WF-1000XM3".into(),
            firmware_version: "3.1.2".into(),
            hardware_color_code: Some(0x02),
            detected_finish: HeadphoneFinish::PlatinumSilver,
            bluetooth_address: "AA:BB:CC:11:22:31".into(),
            protocol_version: MdrProtocolVersion::V1,
            has_table_2: false,
            t1_functions: set(&[0x13, 0x22, 0x23, 0x24, 0x25, 0x32, 0x51, 0xB1, 0xC1, 0xD2]),
            battery: BatteryState {
                level: Some(74),
                charging_status: ChargingStatus::NotCharging,
                left: Some(BatteryUnit::new(80, ChargingStatus::NotCharging)),
                right: Some(BatteryUnit::new(74, ChargingStatus::NotCharging)),
                case_battery: Some(BatteryUnit::new(90, ChargingStatus::NotCharging)),
            },
            codec: AudioCodec::Aac,
            equalizer: EqualizerState {
                available_preset_codes: default_presets,
                ..Default::default()
            },
            ..Default::default()
        },
        _ => HeadphoneState::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::mdr_arq::MdrArqSession;
    use crate::protocol::sony_v2;
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test]
    async fn arq_session_handshake_and_controls_over_simulator() {
        let transport = Arc::new(SimulatedSonyTransport::new(SimulatedProfile::Wh1000Xm5));
        let session = MdrArqSession::new(transport);
        session
            .connect("AA:BB:CC:11:22:33", sony_v2::SERVICE_UUID_V2)
            .await
            .unwrap();

        let proto = session
            .query(
                sony_v2::get_protocol_info(),
                0x01,
                None,
                MdrDataType::DataMdr,
                Duration::from_secs(2),
            )
            .await
            .unwrap();
        let mut st = sony_v2::apply_table1_message(&HeadphoneState::default(), &proto);

        let name_reply = session
            .query(
                sony_v2::get_model_name(),
                0x05,
                Some(0x01),
                MdrDataType::DataMdr,
                Duration::from_secs(2),
            )
            .await
            .unwrap();
        st = sony_v2::apply_table1_message(&st, &name_reply);
        assert_eq!(st.model_name, "WH-1000XM5");

        let nc_target = NoiseControlState {
            mode: NoiseControlMode::AmbientSound,
            ambient_level: 15,
            focus_on_voice: true,
        };
        session
            .send(
                sony_v2::set_noise_control(&nc_target, false, 0x17),
                MdrDataType::DataMdr,
            )
            .await
            .unwrap();

        let nc_reply = session
            .query(
                sony_v2::get_noise_control(0x17),
                0x67,
                Some(0x17),
                MdrDataType::DataMdr,
                Duration::from_secs(2),
            )
            .await
            .unwrap();
        st = sony_v2::apply_table1_message(&st, &nc_reply);
        assert_eq!(st.noise_control, nc_target);
        session.close();
    }
}
