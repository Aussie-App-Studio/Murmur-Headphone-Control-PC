use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use tokio::sync::broadcast;

use crate::catalog::resolve_device_profile;
use crate::models::{
    AudioCodec, BatteryState, BatteryUnit, ChargingStatus, HeadphoneCapabilities, HeadphoneFinish,
    HeadphoneState, MdrProtocolVersion,
};
use crate::protocol::transport::{HeadphoneSession, ProtocolError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AirPodsBleReading {
    pub model_name: &'static str,
    pub is_over_ear: bool,
    pub battery: BatteryState,
}

fn decode_battery_nibble(nibble: u8) -> Option<i32> {
    if nibble <= 10 {
        Some((nibble as i32 * 10).clamp(0, 100))
    } else {
        None
    }
}

fn decode_apple_audio_model(model_id: u16) -> (&'static str, bool) {
    match model_id {
        0x0220 => ("AirPods (1st Gen)", false),
        0x0F20 => ("AirPods (2nd Gen)", false),
        0x1320 => ("AirPods (3rd Gen)", false),
        0x1920 => ("AirPods 4", false),
        0x1B20 => ("AirPods 4 (ANC)", false),
        0x0E20 => ("AirPods Pro", false),
        0x1420 => ("AirPods Pro 2", false),
        0x2420 => ("AirPods Pro 2 (USB-C)", false),
        0x2720 => ("AirPods Pro 3", false),
        0x0A20 | 0x1F20 => ("AirPods Max", true),
        0x0520 => ("BeatsX", false),
        0x0620 => ("Beats Solo3", true),
        0x0B20 => ("Powerbeats Pro", false),
        0x0C20 => ("Beats Solo Pro", true),
        0x1020 => ("Beats Flex", false),
        0x1120 => ("Beats Studio Buds", false),
        0x1220 => ("Beats Fit Pro", false),
        0x1620 => ("Beats Studio Buds+", false),
        0x1720 => ("Beats Studio Pro", true),
        0x2620 => ("Beats Solo 4", true),
        _ => ("AirPods", false),
    }
}

/// Parses an Apple Continuity Proximity Pairing BLE manufacturer payload (Company ID `0x004C`).
pub fn parse_apple_continuity_proximity(data: &[u8]) -> Option<AirPodsBleReading> {
    if data.len() < 27 || data[0] != 0x07 || data[1] != 0x19 {
        return None;
    }

    let model_id = u16::from_be_bytes([data[3], data[4]]);
    let (model_name, is_over_ear) = decode_apple_audio_model(model_id);

    let is_flipped = (data[5] & 0x20) == 0;
    let nibble_a = (data[6] >> 4) & 0x0F;
    let nibble_b = data[6] & 0x0F;
    let charge_flags = (data[7] >> 4) & 0x0F;
    let case_nibble = data[7] & 0x0F;

    let (left_raw, right_raw, left_chg, right_chg) = if is_flipped {
        (
            nibble_b,
            nibble_a,
            (charge_flags & 0x02) != 0,
            (charge_flags & 0x01) != 0,
        )
    } else {
        (
            nibble_a,
            nibble_b,
            (charge_flags & 0x01) != 0,
            (charge_flags & 0x02) != 0,
        )
    };
    let case_chg = (charge_flags & 0x04) != 0;

    let left_lvl = decode_battery_nibble(left_raw);
    let right_lvl = decode_battery_nibble(right_raw);
    let case_lvl = decode_battery_nibble(case_nibble);

    if left_lvl.is_none() && right_lvl.is_none() && case_lvl.is_none() {
        return None;
    }

    let battery = if is_over_ear {
        let lvl = left_lvl.or(right_lvl)?;
        let charging = if left_chg || right_chg {
            ChargingStatus::Charging
        } else {
            ChargingStatus::NotCharging
        };
        BatteryState {
            level: Some(lvl),
            charging_status: charging,
            left: None,
            right: None,
            case_battery: None,
        }
    } else {
        let to_unit = |lvl: Option<i32>, chg: bool| {
            lvl.map(|l| {
                BatteryUnit::new(
                    l,
                    if chg {
                        ChargingStatus::Charging
                    } else {
                        ChargingStatus::NotCharging
                    },
                )
            })
        };
        let min_bud = [left_lvl, right_lvl].into_iter().flatten().min();
        let any_bud_chg = left_chg || right_chg;
        BatteryState {
            level: min_bud,
            charging_status: if any_bud_chg {
                ChargingStatus::Charging
            } else {
                ChargingStatus::NotCharging
            },
            left: to_unit(left_lvl, left_chg),
            right: to_unit(right_lvl, right_chg),
            case_battery: to_unit(case_lvl, case_chg),
        }
    };

    Some(AirPodsBleReading {
        model_name,
        is_over_ear,
        battery,
    })
}

pub struct AirPodsBleSession {
    connected: AtomicBool,
    state: Mutex<HeadphoneState>,
    state_tx: broadcast::Sender<HeadphoneState>,
    disconnect_tx: broadcast::Sender<()>,
    stop_signal: Arc<(Mutex<bool>, Condvar)>,
    watcher_thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl AirPodsBleSession {
    pub fn new() -> Arc<Self> {
        let (state_tx, _) = broadcast::channel(16);
        let (disconnect_tx, _) = broadcast::channel(4);
        Arc::new(Self {
            connected: AtomicBool::new(false),
            state: Mutex::new(HeadphoneState::default()),
            state_tx,
            disconnect_tx,
            stop_signal: Arc::new((Mutex::new(false), Condvar::new())),
            watcher_thread: Mutex::new(None),
        })
    }

    fn apply_ble_reading(self: &Arc<Self>, reading: AirPodsBleReading) {
        let mut st = self.state.lock().unwrap();
        if st.battery == reading.battery {
            return;
        }
        st.battery = reading.battery;
        if st.model_name == "AirPods" || st.model_name.trim().is_empty() {
            st.model_name = reading.model_name.to_string();
        }
        st.custom_capabilities = Some(HeadphoneCapabilities::airpods_ble_readonly(
            reading.is_over_ear,
        ));
        let updated = st.clone();
        drop(st);
        let _ = self.state_tx.send(updated);
    }

    #[cfg(windows)]
    fn start_ble_watcher(self: &Arc<Self>, expected_over_ear: bool) {
        {
            let mut flag = self.stop_signal.0.lock().unwrap();
            *flag = false;
        }
        let session = Arc::clone(self);
        let stop_signal = Arc::clone(&self.stop_signal);

        let handle = std::thread::Builder::new()
            .name("murmur-airpods-ble".into())
            .spawn(move || {
                run_windows_ble_watcher(session, stop_signal, expected_over_ear);
            });
        if let Ok(h) = handle {
            *self.watcher_thread.lock().unwrap() = Some(h);
        }
    }

    #[cfg(not(windows))]
    fn start_ble_watcher(self: &Arc<Self>, _expected_over_ear: bool) {}
}

#[cfg(windows)]
fn run_windows_ble_watcher(
    session: Arc<AirPodsBleSession>,
    stop_signal: Arc<(Mutex<bool>, Condvar)>,
    expected_over_ear: bool,
) {
    use windows::Devices::Bluetooth::Advertisement::{
        BluetoothLEAdvertisementReceivedEventArgs, BluetoothLEAdvertisementWatcher,
        BluetoothLEScanningMode,
    };
    use windows::Foundation::TypedEventHandler;
    use windows::Storage::Streams::DataReader;

    let Ok(watcher) = BluetoothLEAdvertisementWatcher::new() else {
        log::warn!("Unable to create Windows BLE Advertisement Watcher for AirPods");
        return;
    };
    let _ = watcher.SetScanningMode(BluetoothLEScanningMode::Passive);

    let handler = TypedEventHandler::<
        BluetoothLEAdvertisementWatcher,
        BluetoothLEAdvertisementReceivedEventArgs,
    >::new(move |_sender, args_ref| {
        let Some(args) = args_ref.as_ref() else {
            return Ok(());
        };
        if args.RawSignalStrengthInDBm()? < -75 {
            return Ok(());
        }
        let adv = args.Advertisement()?;
        let mfd_list = adv.ManufacturerData()?;
        for mfd in mfd_list {
            if mfd.CompanyId()? == 0x004C {
                let buf = mfd.Data()?;
                let len = buf.Length()? as usize;
                if len >= 27 {
                    let reader = DataReader::FromBuffer(&buf)?;
                    let mut bytes = vec![0u8; len];
                    reader.ReadBytes(&mut bytes)?;
                    if let Some(reading) = parse_apple_continuity_proximity(&bytes) {
                        if reading.is_over_ear == expected_over_ear {
                            session.apply_ble_reading(reading);
                        }
                    }
                }
            }
        }
        Ok(())
    });

    let Ok(token) = watcher.Received(&handler) else {
        return;
    };
    if let Err(e) = watcher.Start() {
        log::warn!("Windows BLE watcher Start() failed: {}", e);
        let _ = watcher.RemoveReceived(token);
        return;
    }

    let (lock, cvar) = &*stop_signal;
    let mut stopped = lock.lock().unwrap();
    while !*stopped {
        stopped = cvar.wait(stopped).unwrap();
    }

    let _ = watcher.Stop();
    let _ = watcher.RemoveReceived(token);
}

#[async_trait::async_trait]
impl HeadphoneSession for Arc<AirPodsBleSession> {
    fn protocol_version(&self) -> MdrProtocolVersion {
        MdrProtocolVersion::AirPodsAap
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
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
        let name = if device_name.trim().is_empty() {
            "AirPods".to_string()
        } else {
            device_name.trim().to_string()
        };
        let is_over_ear = resolve_device_profile(&name, false).form_factor
            == crate::catalog::HeadphoneFormFactor::OverEar;

        self.connected.store(true, Ordering::SeqCst);
        let synced = {
            let mut st = self.state.lock().unwrap();
            *st = HeadphoneState {
                model_name: name,
                firmware_version: "Apple BLE Continuity".to_string(),
                bluetooth_address: address.to_string(),
                protocol_version: MdrProtocolVersion::AirPodsAap,
                codec: AudioCodec::Aac,
                detected_finish: HeadphoneFinish::WarmWhite,
                custom_capabilities: Some(HeadphoneCapabilities::airpods_ble_readonly(is_over_ear)),
                ..Default::default()
            };
            st.clone()
        };

        self.start_ble_watcher(is_over_ear);
        Ok(synced)
    }

    async fn poll_battery(&self) -> Result<(), ProtocolError> {
        Ok(())
    }

    fn close(&self) {
        self.connected.store(false, Ordering::SeqCst);
        {
            let (lock, cvar) = &*self.stop_signal;
            let mut flag = lock.lock().unwrap();
            *flag = true;
            cvar.notify_all();
        }
        if let Some(h) = self.watcher_thread.lock().unwrap().take() {
            let _ = h.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_proximity_packet(
        model_hi: u8,
        model_lo: u8,
        status5: u8,
        pods_byte: u8,
        flags_case_byte: u8,
    ) -> Vec<u8> {
        let mut p = vec![0u8; 27];
        p[0] = 0x07;
        p[1] = 0x19;
        p[2] = 0x01;
        p[3] = model_hi;
        p[4] = model_lo;
        p[5] = status5;
        p[6] = pods_byte;
        p[7] = flags_case_byte;
        p
    }

    #[test]
    fn parses_normal_airpods_pro_2_battery_and_charging() {
        // Not flipped: bit 5 = 1 (0x20). Nibble A (Left)=9 (90%), Nibble B (Right)=8 (80%).
        // Charge flags high nibble = 0x05 (bit0 Left charging + bit2 Case charging), Case = 6 (60%).
        let pkt = make_proximity_packet(0x24, 0x20, 0x20, 0x98, 0x56);
        let reading = parse_apple_continuity_proximity(&pkt).unwrap();
        assert_eq!(reading.model_name, "AirPods Pro 2 (USB-C)");
        assert!(!reading.is_over_ear);
        assert_eq!(
            reading.battery.left,
            Some(BatteryUnit::new(90, ChargingStatus::Charging))
        );
        assert_eq!(
            reading.battery.right,
            Some(BatteryUnit::new(80, ChargingStatus::NotCharging))
        );
        assert_eq!(
            reading.battery.case_battery,
            Some(BatteryUnit::new(60, ChargingStatus::Charging))
        );
        assert_eq!(reading.battery.level, Some(80));
        assert!(reading.battery.is_charging());
    }

    #[test]
    fn parses_flipped_airpods_with_disconnected_slot() {
        // Flipped: bit 5 = 0 (0x00). Right=NibbleA=0x0F (absent), Left=NibbleB=7 (70%).
        // Charge flags = 0x02 (Right chg flag in flipped => Left charging), Case = 0x0A (100%).
        let pkt = make_proximity_packet(0x13, 0x20, 0x00, 0xF7, 0x2A);
        let reading = parse_apple_continuity_proximity(&pkt).unwrap();
        assert_eq!(reading.model_name, "AirPods (3rd Gen)");
        assert!(!reading.is_over_ear);
        assert_eq!(
            reading.battery.left,
            Some(BatteryUnit::new(70, ChargingStatus::Charging))
        );
        assert_eq!(reading.battery.right, None);
        assert_eq!(
            reading.battery.case_battery,
            Some(BatteryUnit::new(100, ChargingStatus::NotCharging))
        );
        assert_eq!(reading.battery.level, Some(70));
    }

    #[test]
    fn parses_airpods_max_single_battery() {
        let pkt = make_proximity_packet(0x0A, 0x20, 0x20, 0x8F, 0x0F);
        let reading = parse_apple_continuity_proximity(&pkt).unwrap();
        assert_eq!(reading.model_name, "AirPods Max");
        assert!(reading.is_over_ear);
        assert_eq!(reading.battery.level, Some(80));
        assert_eq!(reading.battery.left, None);
        assert_eq!(reading.battery.right, None);
        assert_eq!(reading.battery.case_battery, None);
    }

    #[test]
    fn rejects_short_or_wrong_continuity_payload() {
        assert_eq!(parse_apple_continuity_proximity(&[0x07, 0x19, 0x00]), None);
        let mut wrong = vec![0u8; 27];
        wrong[0] = 0x10;
        wrong[1] = 0x05;
        assert_eq!(parse_apple_continuity_proximity(&wrong), None);
    }

    #[test]
    fn beats_routing_and_form_factor_classification() {
        let studio = crate::protocol::transport::classify_device(
            "Beats Studio Pro",
            "11:22:33:44:55:66",
            true,
            0x240404,
        )
        .unwrap();
        assert_eq!(studio.detected_protocol, MdrProtocolVersion::AirPodsAap);
        assert_eq!(
            resolve_device_profile("Beats Studio Pro", false).form_factor,
            crate::catalog::HeadphoneFormFactor::OverEar
        );
        assert_eq!(
            resolve_device_profile("Beats Fit Pro", false).form_factor,
            crate::catalog::HeadphoneFormFactor::Earbuds
        );
    }
}
