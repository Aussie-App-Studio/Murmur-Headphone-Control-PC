use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::Utc;
use tokio::sync::mpsc;

use crate::battery::project_battery;
use crate::models::{
    AmbientScene, AppThemeMode, AssignablePreset, AutoPowerOff, BatteryProjection, BatterySample,
    CachedHeadphoneSnapshot, ConnectionPriority, ConnectionStatus, CustomEqProfile,
    EqualizerPreset, FallbackNotice, HeadphoneAge, HeadphoneFinish, HeadphoneFinishOverride,
    HeadphoneState, MdrProtocolVersion, NcAmbButtonCycle, NoiseControlMode, NoiseControlState,
    PendingOfflineChanges, QuickAccessService, SettingsVaultSnapshot, SpeakToChatSensitivity,
    SpeakToChatTimeout, VoiceAssistant,
};
use crate::platform::{enumerate_paired_devices, WindowsRfcommTransport};
use crate::protocol::airpods_ble::AirPodsBleSession;
use crate::protocol::bose_bmap::BoseBmapSession;
use crate::protocol::galaxy_buds::GalaxyBudsSession;
use crate::protocol::generic::GenericBluetoothSession;
use crate::protocol::mdr_arq::MdrArqSession;
use crate::protocol::mdr_frame::MdrDataType;
use crate::protocol::sony_v1;
use crate::protocol::sony_v2;
use crate::protocol::soundcore::SoundcoreSession;
use crate::protocol::transport::{service_uuid_for_protocol, BondedDevice, HeadphoneSession};
use crate::storage::{AppConfig, DeviceCacheStore, MurmurStorage};

#[derive(Clone, Debug)]
pub struct UiSnapshot {
    pub connection_status: ConnectionStatus,
    pub last_seen_ms: Option<i64>,
    pub status_message: Option<String>,
    pub connection_hint: Option<String>,
    pub fallback_notice: Option<FallbackNotice>,
    pub headphone: HeadphoneState,
    pub paired_devices: Vec<BondedDevice>,
    pub theme_mode: AppThemeMode,
    pub minimize_to_tray: bool,
    pub headphone_age: HeadphoneAge,
    pub finish_override: HeadphoneFinishOverride,
    pub auto_enforce_vault: bool,
    pub custom_eq_profiles: Vec<CustomEqProfile>,
    pub ambient_scenes: Vec<AmbientScene>,
    pub vault_snapshot: Option<SettingsVaultSnapshot>,
    pub vault_diffs: Vec<String>,
    pub pending_changes: Option<PendingOfflineChanges>,
    pub battery_projection: BatteryProjection,
    pub battery_history: Vec<BatterySample>,
    pub locked_playback_address: Option<String>,
    pub data_dir: String,
}

impl UiSnapshot {
    pub fn active_finish(&self) -> HeadphoneFinish {
        self.finish_override.resolve(self.headphone.detected_finish)
    }

    pub fn is_live(&self) -> bool {
        self.connection_status == ConnectionStatus::Connected
    }
}

#[derive(Clone, Debug)]
pub enum AppCommand {
    RefreshDevices,
    ConnectDevice(String),
    Disconnect,

    SetNoiseControl {
        mode: NoiseControlMode,
        ambient_level: Option<i32>,
        focus_on_voice: Option<bool>,
        dragging: bool,
    },
    CycleNoiseControl,
    SetNcAmbButtonCycle(NcAmbButtonCycle),

    SetEqPreset(EqualizerPreset),
    SetEqCustom {
        clear_bass: i32,
        bands: Vec<i32>,
        dragging: bool,
    },
    SaveCustomEqProfile {
        name: String,
        clear_bass: i32,
        bands: Vec<i32>,
    },
    DeleteCustomEqProfile(String),

    SetDsee(bool),
    SetConnectionPriority(ConnectionPriority),
    SetSpeakToChat(bool),
    SetSpeakToChatConfig(SpeakToChatSensitivity, SpeakToChatTimeout),
    SetWearingDetection(bool),
    SetTouchPanel(bool),
    SetMultipoint(bool),
    SetAutoPowerOff(AutoPowerOff),
    SetVoiceAssistant(VoiceAssistant),
    SetVoiceGuidance(bool),
    SetAssignablePreset(usize, AssignablePreset),
    SetQuickAccess(QuickAccessService, QuickAccessService),
    SetPairingMode(bool),
    SetSourceSwitch(bool),
    SwitchAudioSource(String),
    ToggleLockPlaybackSource(String),
    PerformDeviceAction(sony_v2::DeviceAction, String),
    PowerOffHeadphones,

    SaveAmbientScene {
        name: String,
        level: i32,
        focus_on_voice: bool,
    },
    DeleteAmbientScene(String),

    SaveVault,
    RestoreVault,
    ClearVault,
    SetAutoEnforceVault(bool),
    ExportVault(PathBuf),
    ImportVault(PathBuf),

    DiscardPendingChanges,
    SetThemeMode(AppThemeMode),
    SetMinimizeToTray(bool),
    SetHeadphoneAge(HeadphoneAge),
    SetFinishOverride(HeadphoneFinishOverride),
    RespondToAlert {
        message_type: i32,
        accept: bool,
    },
    DismissAlert,
    ClearStatusMessage,
}

#[derive(Clone)]
pub struct AppController {
    snapshot: Arc<Mutex<UiSnapshot>>,
    snapshot_version: Arc<AtomicU64>,
    cmd_tx: mpsc::UnboundedSender<AppCommand>,
    repaint_ctx: Arc<Mutex<Option<egui::Context>>>,
}

impl AppController {
    pub fn start() -> Self {
        let storage = Arc::new(MurmurStorage::new());
        let config = storage.load_config();
        let eq_profiles = storage.load_eq_profiles();
        let ambient_scenes = storage.load_ambient_scenes();
        let cache_store = storage.load_cache_store();
        let battery_history = storage.load_battery_history();

        let last_addr = config.last_device_address.clone().unwrap_or_default();
        let cached_snap = if !last_addr.is_empty() {
            cache_store.snapshots.get(&last_addr.to_uppercase()).cloned()
        } else {
            cache_store.snapshots.values().next().cloned()
        };
        let active_addr = cached_snap
            .as_ref()
            .map(|c| c.bluetooth_address.to_uppercase())
            .unwrap_or_else(|| last_addr.to_uppercase());
        let base_hp = cached_snap
            .as_ref()
            .map(|c| c.to_headphone_state())
            .unwrap_or_default();
        let pending = cache_store.pending.get(&active_addr).cloned();
        let initial_hp = pending
            .as_ref()
            .map(|p| p.apply_overlay(&base_hp))
            .unwrap_or_else(|| base_hp.clone());
        let vault = cache_store.vaults.get(&active_addr).cloned();
        let age = config
            .per_device_age
            .get(&active_addr)
            .copied()
            .unwrap_or_default();
        let finish = config
            .per_device_finish
            .get(&active_addr)
            .copied()
            .unwrap_or_default();

        let initial_devices = enumerate_paired_devices(config.last_device_address.as_deref());
        let now_ms = Utc::now().timestamp_millis();
        let filtered_hist: Vec<BatterySample> = if active_addr.is_empty() {
            battery_history.clone()
        } else {
            let dev_hist: Vec<BatterySample> = battery_history
                .iter()
                .filter(|s| s.device_address.eq_ignore_ascii_case(&active_addr))
                .cloned()
                .collect();
            if dev_hist.is_empty() {
                battery_history.clone()
            } else {
                dev_hist
            }
        };
        let proj = project_battery(
            initial_hp.battery.effective_level(),
            initial_hp.battery.is_charging(),
            initial_hp.noise_control.mode != NoiseControlMode::Off,
            initial_hp.codec.is_high_draw() || initial_hp.dsee_extreme,
            age,
            &filtered_hist,
            now_ms,
            Some(initial_hp.device_profile().battery_baseline),
        );

        let initial_snapshot = UiSnapshot {
            connection_status: ConnectionStatus::Offline,
            last_seen_ms: cached_snap.as_ref().map(|c| c.last_seen_ms),
            status_message: None,
            connection_hint: None,
            fallback_notice: None,
            headphone: initial_hp,
            paired_devices: initial_devices,
            theme_mode: config.theme_mode,
            minimize_to_tray: config.minimize_to_tray,
            headphone_age: age,
            finish_override: finish,
            auto_enforce_vault: config.auto_enforce_vault,
            custom_eq_profiles: eq_profiles,
            ambient_scenes,
            vault_snapshot: vault,
            vault_diffs: Vec::new(),
            pending_changes: pending,
            battery_projection: proj,
            battery_history: filtered_hist,
            locked_playback_address: None,
            data_dir: storage.data_dir().to_string_lossy().to_string(),
        };

        let snapshot = Arc::new(Mutex::new(initial_snapshot));
        let snapshot_version = Arc::new(AtomicU64::new(1));
        let repaint_ctx = Arc::new(Mutex::new(None));
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();

        let snap_clone = Arc::clone(&snapshot);
        let ver_clone = Arc::clone(&snapshot_version);
        let ctx_clone = Arc::clone(&repaint_ctx);
        let cmd_tx_clone = cmd_tx.clone();

        std::thread::Builder::new()
            .name("murmur-runtime".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to build Tokio runtime");
                rt.block_on(async move {
                    let mut worker = BackendWorker::new(
                        storage,
                        config,
                        cache_store,
                        battery_history,
                        base_hp,
                        snap_clone,
                        ver_clone,
                        ctx_clone,
                        cmd_tx_clone,
                    );
                    worker.run(cmd_rx).await;
                });
            })
            .expect("Failed to spawn runtime thread");

        Self {
            snapshot,
            snapshot_version,
            cmd_tx,
            repaint_ctx,
        }
    }

    pub fn set_egui_ctx(&self, ctx: egui::Context) {
        *self.repaint_ctx.lock().unwrap() = Some(ctx);
    }

    pub fn snapshot(&self) -> UiSnapshot {
        self.snapshot.lock().unwrap().clone()
    }

    pub fn snapshot_version(&self) -> u64 {
        self.snapshot_version.load(Ordering::Acquire)
    }

    pub fn send(&self, cmd: AppCommand) {
        let _ = self.cmd_tx.send(cmd);
    }
}

enum ActiveSession {
    SonyArq {
        session: Arc<MdrArqSession>,
    },
    External(Box<dyn HeadphoneSession>),
}

enum InternalEvent {
    SonyMessage(crate::protocol::mdr_frame::MdrDataType, Vec<u8>),
    StateUpdate(HeadphoneState),
    Disconnected,
}

struct BackendWorker {
    storage: Arc<MurmurStorage>,
    config: AppConfig,
    cache_store: DeviceCacheStore,
    all_battery_samples: Vec<BatterySample>,
    last_cached_base: HeadphoneState,
    snapshot: Arc<Mutex<UiSnapshot>>,
    snapshot_version: Arc<AtomicU64>,
    repaint_ctx: Arc<Mutex<Option<egui::Context>>>,
    cmd_tx: mpsc::UnboundedSender<AppCommand>,
    internal_tx: mpsc::UnboundedSender<InternalEvent>,
    internal_rx: mpsc::UnboundedReceiver<InternalEvent>,
    active: Option<ActiveSession>,
    session_tasks: Vec<tokio::task::JoinHandle<()>>,
    last_logged_battery: Option<i32>,
}

impl BackendWorker {
    #[allow(clippy::too_many_arguments)]
    fn new(
        storage: Arc<MurmurStorage>,
        config: AppConfig,
        cache_store: DeviceCacheStore,
        all_battery_samples: Vec<BatterySample>,
        last_cached_base: HeadphoneState,
        snapshot: Arc<Mutex<UiSnapshot>>,
        snapshot_version: Arc<AtomicU64>,
        repaint_ctx: Arc<Mutex<Option<egui::Context>>>,
        cmd_tx: mpsc::UnboundedSender<AppCommand>,
    ) -> Self {
        let (internal_tx, internal_rx) = mpsc::unbounded_channel();
        Self {
            storage,
            config,
            cache_store,
            all_battery_samples,
            last_cached_base,
            snapshot,
            snapshot_version,
            repaint_ctx,
            cmd_tx,
            internal_tx,
            internal_rx,
            active: None,
            session_tasks: Vec::new(),
            last_logged_battery: None,
        }
    }

    fn request_repaint(&self) {
        self.snapshot_version.fetch_add(1, Ordering::Release);
        if let Some(ctx) = self.repaint_ctx.lock().unwrap().as_ref() {
            ctx.request_repaint();
        }
    }

    async fn run(&mut self, mut cmd_rx: mpsc::UnboundedReceiver<AppCommand>) {
        // Startup auto-connect
        self.connect_physical_if_available().await;

        let mut reconnect_tick = tokio::time::interval(Duration::from_secs(10));
        reconnect_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut battery_tick = tokio::time::interval(Duration::from_secs(60));
        battery_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                Some(cmd) = cmd_rx.recv() => {
                    self.handle_command(cmd).await;
                    self.request_repaint();
                }
                Some(ev) = self.internal_rx.recv() => {
                    self.handle_internal_event(ev).await;
                    self.request_repaint();
                }
                _ = reconnect_tick.tick() => {
                    let status = self.snapshot.lock().unwrap().connection_status;
                    if status == ConnectionStatus::Offline {
                        self.connect_physical_if_available().await;
                        self.request_repaint();
                    }
                }
                _ = battery_tick.tick() => {
                    self.poll_battery_tick().await;
                }
            }
        }
    }

    fn teardown_session(&mut self) {
        for h in self.session_tasks.drain(..) {
            h.abort();
        }
        if let Some(active) = self.active.take() {
            match active {
                ActiveSession::SonyArq { session, .. } => session.close(),
                ActiveSession::External(ext) => ext.close(),
            }
        }
    }

    async fn connect_physical_if_available(&mut self) {
        let pref = self.config.last_device_address.clone();
        let devices = tokio::task::spawn_blocking(move || enumerate_paired_devices(pref.as_deref()))
            .await
            .unwrap_or_default();
        {
            let mut s = self.snapshot.lock().unwrap();
            s.paired_devices = devices.clone();
        }

        // Auto-connect to actively connected headphones first
        let connected_candidates: Vec<BondedDevice> =
            devices.into_iter().filter(|d| d.is_connected).collect();
        if connected_candidates.is_empty() {
            return;
        }

        {
            let mut s = self.snapshot.lock().unwrap();
            s.connection_status = ConnectionStatus::Connecting;
            s.connection_hint = None;
        }
        self.request_repaint();

        for dev in connected_candidates {
            if self.try_connect_bonded(&dev).await {
                return;
            }
        }

        let mut s = self.snapshot.lock().unwrap();
        if s.connection_status == ConnectionStatus::Connecting {
            s.connection_status = ConnectionStatus::Offline;
        }
    }

    async fn try_connect_bonded(&mut self, dev: &BondedDevice) -> bool {
        let is_sony = matches!(
            dev.detected_protocol,
            MdrProtocolVersion::V1 | MdrProtocolVersion::V2
        );

        if !is_sony {
            if dev.detected_protocol == MdrProtocolVersion::AirPodsAap {
                let ext = AirPodsBleSession::new();
                if let Ok(synced) = ext.connect_and_sync(&dev.address, &dev.name).await {
                    let notice = FallbackNotice::for_fallback(
                        &dev.name,
                        MdrProtocolVersion::AirPodsAap,
                        None,
                    );
                    self.config.last_device_address = Some(dev.address.clone());
                    self.storage.save_config(&self.config);
                    self.attach_external_session(
                        Box::new(ext),
                        synced,
                        &dev.address,
                        Some(notice),
                    )
                    .await;
                    return true;
                }
                return false;
            }

            if dev.detected_protocol != MdrProtocolVersion::GenericBt {
                let transport = Arc::new(WindowsRfcommTransport::new());
                let uuid = dev
                    .preferred_service_uuid
                    .clone()
                    .unwrap_or_else(|| service_uuid_for_protocol(dev.detected_protocol).to_string());
                let ext: Box<dyn HeadphoneSession> = match dev.detected_protocol {
                    MdrProtocolVersion::BoseBmap => Box::new(BoseBmapSession::new(transport, Some(uuid))),
                    MdrProtocolVersion::Soundcore => Box::new(SoundcoreSession::new(transport, Some(uuid))),
                    MdrProtocolVersion::GalaxyBuds => Box::new(GalaxyBudsSession::new(transport, Some(uuid))),
                    _ => Box::new(GenericBluetoothSession::new()),
                };
                match ext.connect_and_sync(&dev.address, &dev.name).await {
                    Ok(synced) => {
                        self.config.last_device_address = Some(dev.address.clone());
                        self.storage.save_config(&self.config);
                        self.attach_external_session(ext, synced, &dev.address, None)
                            .await;
                        return true;
                    }
                    Err(e) => {
                        ext.close();
                        if dev.is_connected {
                            let fallback = GenericBluetoothSession::new();
                            if let Ok(synced) =
                                fallback.connect_and_sync(&dev.address, &dev.name).await
                            {
                                let notice = FallbackNotice::for_fallback(
                                    &dev.name,
                                    dev.detected_protocol,
                                    Some(&e.to_string()),
                                );
                                self.config.last_device_address = Some(dev.address.clone());
                                self.storage.save_config(&self.config);
                                self.attach_external_session(
                                    Box::new(fallback),
                                    synced,
                                    &dev.address,
                                    Some(notice),
                                )
                                .await;
                                return true;
                            }
                        }
                        return false;
                    }
                }
            } else {
                let ext = GenericBluetoothSession::new();
                if let Ok(synced) = ext.connect_and_sync(&dev.address, &dev.name).await {
                    let notice =
                        FallbackNotice::for_fallback(&dev.name, MdrProtocolVersion::GenericBt, None);
                    self.config.last_device_address = Some(dev.address.clone());
                    self.storage.save_config(&self.config);
                    self.attach_external_session(
                        Box::new(ext),
                        synced,
                        &dev.address,
                        Some(notice),
                    )
                    .await;
                    return true;
                }
                return false;
            }
        }

        // Sony V1 / V2 RFCOMM handshake
        for proto in dev.preferred_protocols() {
            let transport = Arc::new(WindowsRfcommTransport::new());
            let session = MdrArqSession::new(transport);
            let uuid = service_uuid_for_protocol(proto);
            if session.connect(&dev.address, uuid).await.is_ok() {
                let info_cmd = if proto == MdrProtocolVersion::V1 {
                    sony_v1::get_protocol_info()
                } else {
                    sony_v2::get_protocol_info()
                };
                match session
                    .query(info_cmd, 0x01, None, MdrDataType::DataMdr, Duration::from_secs(2))
                    .await
                {
                    Ok(reply) => {
                        let detected = if proto == MdrProtocolVersion::V1 || reply.len() <= 4 {
                            MdrProtocolVersion::V1
                        } else {
                            MdrProtocolVersion::V2
                        };
                        self.config.last_device_address = Some(dev.address.clone());
                        self.storage.save_config(&self.config);
                        self.attach_sony_session(session, &dev.address, &reply, detected)
                            .await;
                        return true;
                    }
                    Err(_) => session.close(),
                }
            } else {
                session.close();
            }
        }

        if dev.is_connected {
            let mut s = self.snapshot.lock().unwrap();
            s.connection_hint = Some(format!(
                "{} is paired & connected, but the Sony control channel is busy or unavailable.",
                dev.name
            ));
        }
        false
    }

    async fn attach_external_session(
        &mut self,
        ext: Box<dyn HeadphoneSession>,
        synced_state: HeadphoneState,
        address: &str,
        fallback_notice: Option<FallbackNotice>,
    ) {
        self.teardown_session();
        self.last_logged_battery = None;

        let mut rx_state = ext.subscribe_state();
        let tx_state = self.internal_tx.clone();
        self.session_tasks.push(tokio::spawn(async move {
            while let Ok(st) = rx_state.recv().await {
                let _ = tx_state.send(InternalEvent::StateUpdate(st));
            }
        }));

        let mut rx_disc = ext.subscribe_disconnect();
        let tx_disc = self.internal_tx.clone();
        self.session_tasks.push(tokio::spawn(async move {
            if rx_disc.recv().await.is_ok() {
                let _ = tx_disc.send(InternalEvent::Disconnected);
            }
        }));

        let addr_key = address.to_uppercase();
        let age = self.config.per_device_age.get(&addr_key).copied().unwrap_or_default();
        let finish = self
            .config
            .per_device_finish
            .get(&addr_key)
            .copied()
            .unwrap_or_default();
        let vault = self.cache_store.vaults.get(&addr_key).cloned();
        let pending = self.cache_store.pending.get(&addr_key).cloned();

        self.active = Some(ActiveSession::External(ext));
        {
            let mut s = self.snapshot.lock().unwrap();
            s.connection_status = ConnectionStatus::Connected;
            s.connection_hint = None;
            s.fallback_notice = fallback_notice;
            s.headphone = synced_state.clone();
            s.headphone_age = age;
            s.finish_override = finish;
            s.vault_snapshot = vault;
            s.pending_changes = pending.clone();
            s.locked_playback_address = None;
        }
        self.apply_headphone_update(synced_state, true);

        if let Some(p) = pending.filter(|p| !p.is_empty()) {
            self.flush_pending_to_live(p).await;
        }
    }

    async fn attach_sony_session(
        &mut self,
        session: Arc<MdrArqSession>,
        address: &str,
        initial_proto: &[u8],
        protocol_version: MdrProtocolVersion,
    ) {
        self.teardown_session();
        self.last_logged_battery = None;

        let mut rx_msg = session.subscribe_messages();
        let tx_msg = self.internal_tx.clone();
        self.session_tasks.push(tokio::spawn(async move {
            while let Ok(msg) = rx_msg.recv().await {
                if msg.table != MdrDataType::Ack {
                    let _ = tx_msg.send(InternalEvent::SonyMessage(msg.table, msg.payload));
                }
            }
        }));

        let mut rx_disc = session.subscribe_disconnect();
        let tx_disc = self.internal_tx.clone();
        self.session_tasks.push(tokio::spawn(async move {
            if rx_disc.recv().await.is_ok() {
                let _ = tx_disc.send(InternalEvent::Disconnected);
            }
        }));

        let addr_key = address.to_uppercase();
        let age = self.config.per_device_age.get(&addr_key).copied().unwrap_or_default();
        let finish = self
            .config
            .per_device_finish
            .get(&addr_key)
            .copied()
            .unwrap_or_default();
        let vault = self.cache_store.vaults.get(&addr_key).cloned();
        let pending = self.cache_store.pending.get(&addr_key).cloned();

        let mut fresh = HeadphoneState {
            bluetooth_address: address.to_string(),
            protocol_version,
            ..Default::default()
        };
        fresh = if protocol_version == MdrProtocolVersion::V1 {
            sony_v1::apply_table1_message(&fresh, initial_proto)
        } else {
            sony_v2::apply_table1_message(&fresh, initial_proto)
        };

        {
            let mut s = self.snapshot.lock().unwrap();
            s.connection_status = ConnectionStatus::Connected;
            s.connection_hint = None;
            s.fallback_notice = None;
            s.headphone = fresh;
            s.headphone_age = age;
            s.finish_override = finish;
            s.vault_snapshot = vault;
            s.pending_changes = pending.clone();
            s.locked_playback_address = None;
        }

        // Sync state queries
        if protocol_version == MdrProtocolVersion::V1 {
            self.query_t1(&session, sony_v1::get_model_name(), 0x05, Some(0x01)).await;
            self.query_t1(&session, sony_v1::get_firmware_version(), 0x05, Some(0x02)).await;
            self.query_t1(&session, sony_v1::get_series_and_color_info(), 0x05, Some(0x03)).await;
            self.query_t1(&session, sony_v1::get_support_functions(), 0x07, Some(0x00)).await;

            let caps = self.snapshot.lock().unwrap().headphone.capabilities();
            if caps.supports_dual_battery {
                self.query_t1(&session, sony_v1::get_left_right_battery(), 0x11, Some(0x01)).await;
            } else if caps.supports_single_battery {
                self.query_t1(&session, sony_v1::get_battery(), 0x11, Some(0x00)).await;
            }
            if caps.supports_case_battery {
                self.query_t1(&session, sony_v1::get_cradle_battery(), 0x11, Some(0x02)).await;
            }
            if caps.supports_codec {
                self.query_t1(&session, sony_v1::get_codec(), 0x15, Some(0x00)).await;
            }
            if caps.supports_noise_control {
                self.query_t1(
                    &session,
                    sony_v1::get_noise_control(caps.nc_control_subtype),
                    0x67,
                    Some(caps.nc_control_subtype as u8),
                )
                .await;
            }
            if caps.supports_equalizer {
                self.query_t1(&session, sony_v1::get_eq_capability(), 0x51, Some(0x01)).await;
                self.query_t1(&session, sony_v1::get_eq(), 0x57, Some(0x01)).await;
            }
            if caps.supports_connection_priority {
                self.query_t1(&session, sony_v1::get_connection_priority(), 0x77, Some(0x01)).await;
            }
            if caps.supports_dsee {
                self.query_t1(&session, sony_v1::get_dsee(), 0x77, Some(0x02)).await;
            }
            if caps.supports_auto_power_off {
                self.query_t1(&session, sony_v1::get_auto_power_off(), 0xA7, Some(0x01)).await;
            }
            if caps.supports_wearing_detection {
                self.query_t1(&session, sony_v1::get_wearing_detection(), 0xA7, Some(0x02)).await;
            }
            if caps.supports_assignable_settings {
                self.query_t1(&session, sony_v1::get_assignable_settings_capability(), 0xA1, Some(0x03)).await;
                self.query_t1(&session, sony_v1::get_assignable_settings(), 0xA7, Some(0x03)).await;
            }
            if caps.supports_voice_guidance {
                self.query_t1(&session, sony_v1::get_voice_guidance(), 0xB7, Some(0x01)).await;
            }
        } else {
            let _ = session.send(sony_v2::enable_alerts(), MdrDataType::DataMdr).await;
            self.query_t1(&session, sony_v2::get_model_name(), 0x05, Some(0x01)).await;
            self.query_t1(&session, sony_v2::get_firmware_version(), 0x05, Some(0x02)).await;
            self.query_t1(&session, sony_v2::get_series_and_color_info(), 0x05, Some(0x03)).await;
            self.query_t1(&session, sony_v2::get_support_functions_t1(), 0x07, Some(0x00)).await;

            let has_t2 = self.snapshot.lock().unwrap().headphone.has_table_2;
            if has_t2 {
                self.query_t2(&session, sony_v2::get_support_functions_t2(), 0x07, Some(0x00)).await;
            }

            let t1_funcs = self.snapshot.lock().unwrap().headphone.t1_functions.clone();
            let gs_slots: Vec<i32> = if t1_funcs.is_empty() {
                vec![0xD1, 0xD2]
            } else {
                (0xD1..=0xD4).filter(|s| t1_funcs.contains(s)).collect()
            };
            for slot in gs_slots {
                self.query_t1(
                    &session,
                    sony_v2::get_general_setting_capability(slot),
                    0xD1,
                    Some(slot as u8),
                )
                .await;
                self.query_t1(
                    &session,
                    sony_v2::get_general_setting(slot),
                    0xD7,
                    Some(slot as u8),
                )
                .await;
            }

            let caps = self.snapshot.lock().unwrap().headphone.capabilities();
            if caps.supports_dual_battery {
                self.query_t1(&session, sony_v2::get_left_right_battery(), 0x23, Some(0x01)).await;
            } else if caps.supports_single_battery {
                self.query_t1(&session, sony_v2::get_battery(), 0x23, Some(0x00)).await;
            }
            if caps.supports_case_battery {
                self.query_t1(&session, sony_v2::get_cradle_battery(), 0x23, Some(0x02)).await;
            }
            if caps.supports_codec {
                self.query_t1(&session, sony_v2::get_codec(), 0x13, Some(0x02)).await;
            }
            if caps.supports_noise_control {
                self.query_t1(
                    &session,
                    sony_v2::get_noise_control(caps.nc_control_subtype),
                    0x67,
                    Some(caps.nc_control_subtype as u8),
                )
                .await;
            }
            if caps.supports_nc_amb_button_cycle {
                self.query_t1(&session, sony_v2::get_nc_amb_button_cycle(), 0x67, Some(0x30)).await;
            }
            if caps.supports_equalizer {
                self.query_t1(&session, sony_v2::get_eq_capability(), 0x51, Some(0x00)).await;
                self.query_t1(&session, sony_v2::get_eq(), 0x57, Some(0x00)).await;
            }
            if caps.supports_dsee {
                self.query_t1(&session, sony_v2::get_dsee(), 0xE7, Some(0x01)).await;
            }
            if caps.supports_connection_priority {
                self.query_t1(&session, sony_v2::get_connection_priority(), 0xE7, Some(0x00)).await;
            }
            if caps.supports_speak_to_chat {
                self.query_t1(&session, sony_v2::get_speak_to_chat(), 0xF7, Some(0x0C)).await;
                self.query_t1(&session, sony_v2::get_speak_to_chat_config(), 0xFB, Some(0x0C)).await;
            }
            if caps.supports_wearing_detection {
                self.query_t1(&session, sony_v2::get_wearing_detection(), 0xF7, Some(0x01)).await;
            }
            if caps.supports_auto_power_off {
                self.query_t1(
                    &session,
                    sony_v2::get_auto_power_off(caps.auto_power_off_subtype),
                    0x27,
                    Some(caps.auto_power_off_subtype as u8),
                )
                .await;
            }
            if caps.supports_assignable_settings {
                self.query_t1(&session, sony_v2::get_assignable_settings_capability(), 0xF1, Some(0x03)).await;
                self.query_t1(&session, sony_v2::get_assignable_settings(), 0xF7, Some(0x03)).await;
            }
            if caps.supports_voice_assistant {
                self.query_t1(&session, sony_v2::get_voice_assistant_capability(), 0xF1, Some(0x04)).await;
                self.query_t1(&session, sony_v2::get_voice_assistant(), 0xF7, Some(0x04)).await;
            }
            if caps.supports_quick_access {
                self.query_t1(&session, sony_v2::get_quick_access_capability(), 0xF1, Some(0x0D)).await;
                self.query_t1(&session, sony_v2::get_quick_access(), 0xF7, Some(0x0D)).await;
            }

            let (has_t2, caps) = {
                let s = self.snapshot.lock().unwrap();
                (s.headphone.has_table_2, s.headphone.capabilities())
            };
            if has_t2 {
                if caps.supports_voice_guidance {
                    self.query_t2(&session, sony_v2::get_voice_guidance(), 0x47, Some(0x01)).await;
                }
                if caps.supports_paired_devices {
                    self.query_t2(&session, sony_v2::get_pairing_status(), 0x33, Some(0x02)).await;
                    self.query_t2(&session, sony_v2::get_paired_devices(), 0x37, Some(0x02)).await;
                    self.query_t2(&session, sony_v2::get_source_switch(), 0x37, Some(0x01)).await;
                }
            }
        }

        self.active = Some(ActiveSession::SonyArq { session });

        let hp = self.snapshot.lock().unwrap().headphone.clone();
        self.apply_headphone_update(hp, true);

        if let Some(p) = pending.filter(|p| !p.is_empty()) {
            self.flush_pending_to_live(p).await;
        } else if self.config.auto_enforce_vault {
            let vault_opt = self.snapshot.lock().unwrap().vault_snapshot.clone();
            if let Some(v) = vault_opt {
                let hp = self.snapshot.lock().unwrap().headphone.clone();
                if !v.diffs_from(&hp).is_empty() {
                    self.restore_vault(&v).await;
                }
            }
        }
    }

    async fn query_t1(
        &mut self,
        session: &Arc<MdrArqSession>,
        payload: Vec<u8>,
        op: u8,
        sub: Option<u8>,
    ) {
        if let Ok(reply) = session
            .query(payload, op, sub, MdrDataType::DataMdr, Duration::from_millis(1500))
            .await
        {
            let hp = self.snapshot.lock().unwrap().headphone.clone();
            let next = if hp.protocol_version == MdrProtocolVersion::V1 {
                sony_v1::apply_table1_message(&hp, &reply)
            } else {
                sony_v2::apply_table1_message(&hp, &reply)
            };
            self.apply_headphone_update(next, false);
        }
    }

    async fn query_t2(
        &mut self,
        session: &Arc<MdrArqSession>,
        payload: Vec<u8>,
        op: u8,
        sub: Option<u8>,
    ) {
        if let Ok(reply) = session
            .query(
                payload,
                op,
                sub,
                MdrDataType::DataMdrNo2,
                Duration::from_millis(1500),
            )
            .await
        {
            let hp = self.snapshot.lock().unwrap().headphone.clone();
            let next = sony_v2::apply_table2_message(&hp, &reply);
            self.apply_headphone_update(next, false);
        }
    }

    async fn flush_pending_to_live(&mut self, pending: PendingOfflineChanges) {
        let count = pending.pending_count();
        let hp_now = self.snapshot.lock().unwrap().headphone.clone();
        let caps = hp_now.capabilities();

        if caps.supports_noise_control {
            if let Some(nc) = &pending.noise_control {
                let _ = self.apply_noise_control_live(nc, false).await;
            }
        }
        if caps.supports_equalizer {
            if pending.eq_preset_code == Some(EqualizerPreset::Manual.wire_byte() as i32) {
                let cb = pending.eq_clear_bass.unwrap_or(hp_now.equalizer.clear_bass);
                let bands = pending
                    .eq_bands
                    .clone()
                    .unwrap_or_else(|| hp_now.equalizer.bands.clone());
                let _ = self.apply_eq_custom_live(cb, &bands, false).await;
            } else if let Some(code) = pending.eq_preset_code {
                if let Some(preset) = EqualizerPreset::from_byte((code & 0xFF) as u8) {
                    let _ = self.apply_eq_preset_live(preset).await;
                }
            }
        }
        if caps.supports_dsee {
            if let Some(dsee) = pending.dsee_extreme {
                let _ = self.apply_dsee_live(dsee).await;
            }
        }
        if caps.supports_connection_priority {
            if let Some(w) = pending.connection_priority_wire {
                let _ = self
                    .apply_connection_priority_live(ConnectionPriority::from_byte(w as u8))
                    .await;
            }
        }
        if caps.supports_speak_to_chat {
            if let Some(stc) = pending.speak_to_chat {
                let _ = self.apply_speak_to_chat_live(stc).await;
            }
            if pending.speak_to_chat_sensitivity_wire.is_some()
                || pending.speak_to_chat_timeout_wire.is_some()
            {
                let sens = pending
                    .speak_to_chat_sensitivity_wire
                    .map(|w| SpeakToChatSensitivity::from_byte(w as u8))
                    .unwrap_or(hp_now.speak_to_chat_sensitivity);
                let timeout = pending
                    .speak_to_chat_timeout_wire
                    .map(|w| SpeakToChatTimeout::from_byte(w as u8))
                    .unwrap_or(hp_now.speak_to_chat_timeout);
                let _ = self.apply_speak_to_chat_config_live(sens, timeout).await;
            }
        }
        if caps.supports_wearing_detection {
            if let Some(wd) = pending.wearing_detection {
                let _ = self.apply_wearing_detection_live(wd).await;
            }
        }
        if caps.supports_touch_panel {
            if let Some(tp) = pending.touch_panel_enabled {
                let _ = self.apply_touch_panel_live(tp).await;
            }
        }
        if caps.supports_auto_power_off {
            if let Some(w) = pending.auto_power_off_wire {
                if let Some(mode) = AutoPowerOff::from_byte(w as u8) {
                    let _ = self.apply_auto_power_off_live(mode).await;
                }
            }
        }
        if caps.supports_nc_amb_button_cycle {
            if let Some(w) = pending.nc_amb_button_cycle_wire {
                if let Some(cycle) = NcAmbButtonCycle::from_byte(w as u8) {
                    let _ = self.apply_nc_amb_cycle_live(cycle).await;
                }
            }
        }
        if caps.supports_voice_assistant {
            if let Some(w) = pending.voice_assistant_wire {
                let _ = self
                    .apply_voice_assistant_live(VoiceAssistant::from_byte(w as u8))
                    .await;
            }
        }
        if caps.supports_quick_access
            && (pending.quick_access_double_wire.is_some()
                || pending.quick_access_triple_wire.is_some())
        {
            let d = pending
                .quick_access_double_wire
                .map(|w| QuickAccessService(w as u8))
                .unwrap_or(hp_now.quick_access_double_tap);
            let t = pending
                .quick_access_triple_wire
                .map(|w| QuickAccessService(w as u8))
                .unwrap_or(hp_now.quick_access_triple_tap);
            let _ = self.apply_quick_access_live(d, t).await;
        }
        if caps.supports_multipoint {
            if let Some(mp) = pending.multipoint_enabled {
                let _ = self.apply_multipoint_live(mp).await;
            }
        }
        if caps.supports_voice_guidance {
            if let Some(vg) = pending.voice_guidance_enabled {
                let _ = self.apply_voice_guidance_live(vg).await;
            }
        }

        let updated_hp = pending.apply_overlay(&self.snapshot.lock().unwrap().headphone);
        let addr_key = updated_hp.bluetooth_address.to_uppercase();
        self.last_cached_base = updated_hp.clone();
        self.cache_store.pending.remove(&addr_key);
        self.cache_store.snapshots.insert(
            addr_key,
            CachedHeadphoneSnapshot::from_state(&updated_hp, Utc::now().timestamp_millis()),
        );
        self.storage.save_cache_store(&self.cache_store);

        let mut s = self.snapshot.lock().unwrap();
        s.headphone = updated_hp.clone();
        s.pending_changes = None;
        s.status_message = Some(format!(
            "Synced {} queued offline change(s) to {}",
            count, updated_hp.model_name
        ));
    }

    async fn handle_internal_event(&mut self, ev: InternalEvent) {
        match ev {
            InternalEvent::SonyMessage(table, payload) => {
                let hp = self.snapshot.lock().unwrap().headphone.clone();
                let next = if table == MdrDataType::DataMdrNo2 {
                    sony_v2::apply_table2_message(&hp, &payload)
                } else if hp.protocol_version == MdrProtocolVersion::V1 {
                    sony_v1::apply_table1_message(&hp, &payload)
                } else {
                    sony_v2::apply_table1_message(&hp, &payload)
                };
                self.apply_headphone_update(next, true);
            }
            InternalEvent::StateUpdate(st) => {
                self.apply_headphone_update(st, true);
            }
            InternalEvent::Disconnected => {
                self.teardown_session();
                let mut s = self.snapshot.lock().unwrap();
                s.connection_status = ConnectionStatus::Offline;
                s.fallback_notice = None;
            }
        }
    }

    fn apply_headphone_update(&mut self, updated: HeadphoneState, persist_snapshot: bool) {
        let now_ms = Utc::now().timestamp_millis();
        let locked_mac = self.snapshot.lock().unwrap().locked_playback_address.clone();
        if let Some(mac) = locked_mac {
            if let Some(playing) = updated.paired_devices.iter().find(|d| d.is_playing) {
                if !playing.address.eq_ignore_ascii_case(&mac)
                    && updated
                        .paired_devices
                        .iter()
                        .any(|d| d.address.eq_ignore_ascii_case(&mac) && d.is_connected)
                {
                    let _ = self.cmd_tx.send(AppCommand::SwitchAudioSource(mac));
                }
            }
        }

        let batt_lvl = updated.battery.effective_level();

        if let Some(lvl) = batt_lvl {
            if self.last_logged_battery != Some(lvl) {
                self.last_logged_battery = Some(lvl);
                let sample = BatterySample {
                    timestamp_ms: now_ms,
                    level: lvl,
                    is_charging: updated.battery.is_charging(),
                    nc_active: updated.noise_control.mode != NoiseControlMode::Off,
                    high_power_dsp: updated.codec.is_high_draw() || updated.dsee_extreme,
                    device_address: updated.bluetooth_address.to_uppercase(),
                };
                self.all_battery_samples.push(sample);
                if self.all_battery_samples.len() > 500 {
                    let drop_n = self.all_battery_samples.len() - 500;
                    self.all_battery_samples.drain(0..drop_n);
                }
                self.storage.save_battery_history(&self.all_battery_samples);
            }
        }

        let addr_key = updated.bluetooth_address.to_uppercase();
        let dev_samples: Vec<BatterySample> = if addr_key.is_empty() {
            self.all_battery_samples.clone()
        } else {
            let matching: Vec<BatterySample> = self
                .all_battery_samples
                .iter()
                .filter(|b| b.device_address.eq_ignore_ascii_case(&addr_key))
                .cloned()
                .collect();
            if matching.is_empty() {
                self.all_battery_samples.clone()
            } else {
                matching
            }
        };

        {
            let mut s = self.snapshot.lock().unwrap();
            let diffs = s
                .vault_snapshot
                .as_ref()
                .map(|v| v.diffs_from(&updated))
                .unwrap_or_default();
            s.headphone = updated.clone();
            s.last_seen_ms = Some(now_ms);
            s.vault_diffs = diffs;
            s.battery_history = dev_samples.clone();
            s.battery_projection = project_battery(
                updated.battery.effective_level(),
                updated.battery.is_charging(),
                updated.noise_control.mode != NoiseControlMode::Off,
                updated.codec.is_high_draw() || updated.dsee_extreme,
                s.headphone_age,
                &dev_samples,
                now_ms,
                Some(updated.device_profile().battery_baseline),
            );
        }

        if persist_snapshot && !addr_key.is_empty() {
            self.last_cached_base = updated.clone();
            self.cache_store.snapshots.insert(
                addr_key,
                CachedHeadphoneSnapshot::from_state(&updated, now_ms),
            );
            self.storage.save_cache_store(&self.cache_store);
        }
        self.snapshot_version.fetch_add(1, Ordering::Release);
    }

    async fn poll_battery_tick(&self) {
        let Some(active) = &self.active else { return };
        match active {
            ActiveSession::SonyArq { session } if session.is_connected() => {
                let hp = self.snapshot.lock().unwrap().headphone.clone();
                let caps = hp.capabilities();
                if hp.protocol_version == MdrProtocolVersion::V1 {
                    if caps.supports_dual_battery {
                        let _ = session.send(sony_v1::get_left_right_battery(), MdrDataType::DataMdr).await;
                        if caps.supports_case_battery {
                            let _ = session.send(sony_v1::get_cradle_battery(), MdrDataType::DataMdr).await;
                        }
                    } else if caps.supports_single_battery {
                        let _ = session.send(sony_v1::get_battery(), MdrDataType::DataMdr).await;
                    }
                } else if caps.supports_dual_battery {
                    let _ = session.send(sony_v2::get_left_right_battery(), MdrDataType::DataMdr).await;
                    if caps.supports_case_battery {
                        let _ = session.send(sony_v2::get_cradle_battery(), MdrDataType::DataMdr).await;
                    }
                } else if caps.supports_single_battery {
                    let _ = session.send(sony_v2::get_battery(), MdrDataType::DataMdr).await;
                }
            }
            ActiveSession::External(ext) if ext.is_connected() => {
                let _ = ext.poll_battery().await;
            }
            _ => {}
        }
    }

    fn queue_offline<F: FnOnce(&mut PendingOfflineChanges)>(&mut self, mutate: F) {
        let mut s = self.snapshot.lock().unwrap();
        let mut p = s.pending_changes.clone().unwrap_or_default();
        mutate(&mut p);
        let overlaid = p.apply_overlay(&s.headphone);
        let addr_key = overlaid.bluetooth_address.to_uppercase();
        s.pending_changes = Some(p.clone());
        s.headphone = overlaid;
        drop(s);

        self.cache_store.pending.insert(addr_key, p);
        self.storage.save_cache_store(&self.cache_store);
    }

    async fn handle_command(&mut self, cmd: AppCommand) {
        match cmd {
            AppCommand::RefreshDevices => {
                let pref = self.config.last_device_address.clone();
                let devices =
                    tokio::task::spawn_blocking(move || enumerate_paired_devices(pref.as_deref()))
                        .await
                        .unwrap_or_default();
                self.snapshot.lock().unwrap().paired_devices = devices;
            }
            AppCommand::ConnectDevice(addr) => {
                self.teardown_session();
                let pref = self.config.last_device_address.clone();
                let devices =
                    tokio::task::spawn_blocking(move || enumerate_paired_devices(pref.as_deref()))
                        .await
                        .unwrap_or_default();
                {
                    let mut s = self.snapshot.lock().unwrap();
                    s.paired_devices = devices.clone();
                    s.connection_status = ConnectionStatus::Connecting;
                    s.connection_hint = None;
                }
                self.request_repaint();
                if let Some(dev) = devices.iter().find(|d| d.address.eq_ignore_ascii_case(&addr)) {
                    if !self.try_connect_bonded(dev).await {
                        let mut s = self.snapshot.lock().unwrap();
                        s.connection_status = ConnectionStatus::Offline;
                        if s.connection_hint.is_none() {
                            s.connection_hint = Some(format!(
                                "Could not open control link to {}. Make sure it is powered on and connected to Windows.",
                                dev.name
                            ));
                        }
                    }
                } else {
                    self.snapshot.lock().unwrap().connection_status = ConnectionStatus::Offline;
                }
            }
            AppCommand::Disconnect => {
                self.teardown_session();
                let mut s = self.snapshot.lock().unwrap();
                s.connection_status = ConnectionStatus::Offline;
                s.fallback_notice = None;
            }
            AppCommand::SetNoiseControl {
                mode,
                ambient_level,
                focus_on_voice,
                dragging,
            } => {
                let hp = self.snapshot.lock().unwrap().headphone.clone();
                let next_nc = NoiseControlState {
                    mode,
                    ambient_level: ambient_level
                        .unwrap_or(hp.noise_control.ambient_level)
                        .clamp(1, 20),
                    focus_on_voice: focus_on_voice.unwrap_or(hp.noise_control.focus_on_voice),
                };
                if self.active.is_some() {
                    let mut next_hp = hp.clone();
                    next_hp.noise_control = next_nc.clone();
                    self.apply_headphone_update(next_hp, !dragging);
                    if let Err(e) = self.apply_noise_control_live(&next_nc, dragging).await {
                        self.snapshot.lock().unwrap().status_message = Some(e);
                    }
                } else {
                    self.queue_offline(|p| p.noise_control = Some(next_nc));
                }
            }
            AppCommand::CycleNoiseControl => {
                let hp = self.snapshot.lock().unwrap().headphone.clone();
                let next_mode = hp.noise_control.mode.next_in_cycle(hp.nc_amb_button_cycle);
                let next_nc = NoiseControlState {
                    mode: next_mode,
                    ..hp.noise_control.clone()
                };
                if self.active.is_some() {
                    let mut next_hp = hp.clone();
                    next_hp.noise_control = next_nc.clone();
                    self.apply_headphone_update(next_hp, true);
                    let _ = self.apply_noise_control_live(&next_nc, false).await;
                } else {
                    self.queue_offline(|p| p.noise_control = Some(next_nc));
                }
            }
            AppCommand::SetNcAmbButtonCycle(cycle) => {
                if self.active.is_some() {
                    let mut hp = self.snapshot.lock().unwrap().headphone.clone();
                    hp.nc_amb_button_cycle = cycle;
                    self.apply_headphone_update(hp, true);
                    let _ = self.apply_nc_amb_cycle_live(cycle).await;
                } else {
                    self.queue_offline(|p| {
                        p.nc_amb_button_cycle_wire = Some(cycle.wire_value() as i32)
                    });
                }
            }
            AppCommand::SetEqPreset(preset) => {
                if self.active.is_some() {
                    let mut hp = self.snapshot.lock().unwrap().headphone.clone();
                    hp.equalizer.preset_code = preset.wire_byte() as i32;
                    self.apply_headphone_update(hp, true);
                    let _ = self.apply_eq_preset_live(preset).await;
                } else {
                    self.queue_offline(|p| p.eq_preset_code = Some(preset.wire_byte() as i32));
                }
            }
            AppCommand::SetEqCustom {
                clear_bass,
                bands,
                dragging,
            } => {
                if self.active.is_some() {
                    let mut hp = self.snapshot.lock().unwrap().headphone.clone();
                    hp.equalizer.preset_code = EqualizerPreset::Manual.wire_byte() as i32;
                    hp.equalizer.clear_bass = clear_bass;
                    hp.equalizer.bands = bands.clone();
                    self.apply_headphone_update(hp, !dragging);
                    let _ = self.apply_eq_custom_live(clear_bass, &bands, dragging).await;
                } else {
                    self.queue_offline(|p| {
                        p.eq_preset_code = Some(EqualizerPreset::Manual.wire_byte() as i32);
                        p.eq_clear_bass = Some(clear_bass);
                        p.eq_bands = Some(bands);
                    });
                }
            }
            AppCommand::SaveCustomEqProfile {
                name,
                clear_bass,
                bands,
            } => {
                let trimmed = name.trim();
                if !trimmed.is_empty() {
                    let profile = CustomEqProfile {
                        id: format!("custom_{}", Utc::now().timestamp_millis()),
                        name: trimmed.to_string(),
                        clear_bass,
                        bands,
                    };
                    let mut s = self.snapshot.lock().unwrap();
                    s.custom_eq_profiles.push(profile);
                    self.storage.save_eq_profiles(&s.custom_eq_profiles);
                }
            }
            AppCommand::DeleteCustomEqProfile(id) => {
                let mut s = self.snapshot.lock().unwrap();
                s.custom_eq_profiles.retain(|p| p.id != id);
                self.storage.save_eq_profiles(&s.custom_eq_profiles);
            }
            AppCommand::SetDsee(enabled) => {
                if self.active.is_some() {
                    let mut hp = self.snapshot.lock().unwrap().headphone.clone();
                    hp.dsee_extreme = enabled;
                    self.apply_headphone_update(hp, true);
                    let _ = self.apply_dsee_live(enabled).await;
                } else {
                    self.queue_offline(|p| p.dsee_extreme = Some(enabled));
                }
            }
            AppCommand::SetConnectionPriority(prio) => {
                if self.active.is_some() {
                    let mut hp = self.snapshot.lock().unwrap().headphone.clone();
                    hp.connection_priority = prio;
                    self.apply_headphone_update(hp, true);
                    let _ = self.apply_connection_priority_live(prio).await;
                } else {
                    self.queue_offline(|p| p.connection_priority_wire = Some(prio.wire_byte() as i32));
                }
            }
            AppCommand::SetSpeakToChat(enabled) => {
                if self.active.is_some() {
                    let mut hp = self.snapshot.lock().unwrap().headphone.clone();
                    hp.speak_to_chat = enabled;
                    self.apply_headphone_update(hp, true);
                    let _ = self.apply_speak_to_chat_live(enabled).await;
                } else {
                    self.queue_offline(|p| p.speak_to_chat = Some(enabled));
                }
            }
            AppCommand::SetSpeakToChatConfig(sens, timeout) => {
                if self.active.is_some() {
                    let mut hp = self.snapshot.lock().unwrap().headphone.clone();
                    hp.speak_to_chat_sensitivity = sens;
                    hp.speak_to_chat_timeout = timeout;
                    self.apply_headphone_update(hp, true);
                    let _ = self.apply_speak_to_chat_config_live(sens, timeout).await;
                } else {
                    self.queue_offline(|p| {
                        p.speak_to_chat_sensitivity_wire = Some(sens.wire_byte() as i32);
                        p.speak_to_chat_timeout_wire = Some(timeout.wire_byte() as i32);
                    });
                }
            }
            AppCommand::SetWearingDetection(enabled) => {
                if self.active.is_some() {
                    let mut hp = self.snapshot.lock().unwrap().headphone.clone();
                    hp.wearing_detection = enabled;
                    self.apply_headphone_update(hp, true);
                    let _ = self.apply_wearing_detection_live(enabled).await;
                } else {
                    self.queue_offline(|p| p.wearing_detection = Some(enabled));
                }
            }
            AppCommand::SetTouchPanel(enabled) => {
                if self.active.is_some() {
                    let mut hp = self.snapshot.lock().unwrap().headphone.clone();
                    for g in &mut hp.general_settings {
                        if g.key == "TOUCH_PANEL_SETTING" {
                            g.is_on = enabled;
                        }
                    }
                    self.apply_headphone_update(hp, true);
                    let _ = self.apply_touch_panel_live(enabled).await;
                } else {
                    self.queue_offline(|p| p.touch_panel_enabled = Some(enabled));
                }
            }
            AppCommand::SetMultipoint(enabled) => {
                if self.active.is_some() {
                    let mut hp = self.snapshot.lock().unwrap().headphone.clone();
                    for g in &mut hp.general_settings {
                        if g.key == "MULTIPOINT_SETTING" {
                            g.is_on = enabled;
                        }
                    }
                    self.apply_headphone_update(hp, true);
                    let _ = self.apply_multipoint_live(enabled).await;
                } else {
                    self.queue_offline(|p| p.multipoint_enabled = Some(enabled));
                }
            }
            AppCommand::SetAutoPowerOff(mode) => {
                if self.active.is_some() {
                    let mut hp = self.snapshot.lock().unwrap().headphone.clone();
                    hp.auto_power_off = mode;
                    self.apply_headphone_update(hp, true);
                    let _ = self.apply_auto_power_off_live(mode).await;
                } else {
                    self.queue_offline(|p| p.auto_power_off_wire = Some(mode.wire_byte() as i32));
                }
            }
            AppCommand::SetVoiceAssistant(va) => {
                if self.active.is_some() {
                    let mut hp = self.snapshot.lock().unwrap().headphone.clone();
                    hp.voice_assistant = va;
                    self.apply_headphone_update(hp, true);
                    let _ = self.apply_voice_assistant_live(va).await;
                } else {
                    self.queue_offline(|p| p.voice_assistant_wire = Some(va.wire_byte() as i32));
                }
            }
            AppCommand::SetVoiceGuidance(enabled) => {
                if self.active.is_some() {
                    let mut hp = self.snapshot.lock().unwrap().headphone.clone();
                    hp.voice_guidance_enabled = enabled;
                    self.apply_headphone_update(hp, true);
                    let _ = self.apply_voice_guidance_live(enabled).await;
                } else {
                    self.queue_offline(|p| p.voice_guidance_enabled = Some(enabled));
                }
            }
            AppCommand::SetAssignablePreset(idx, preset) => {
                let session_opt = match &self.active {
                    Some(ActiveSession::SonyArq { session, .. }) => Some(Arc::clone(session)),
                    _ => None,
                };
                if let Some(session) = session_opt {
                    let mut hp = self.snapshot.lock().unwrap().headphone.clone();
                    if let Some(s) = hp.assignable_settings.get_mut(idx) {
                        s.preset = preset;
                    }
                    let list: Vec<AssignablePreset> =
                        hp.assignable_settings.iter().map(|s| s.preset).collect();
                    let cmd = if hp.protocol_version == MdrProtocolVersion::V1 {
                        sony_v1::set_assignable_settings(&list)
                    } else {
                        sony_v2::set_assignable_settings(&list)
                    };
                    self.apply_headphone_update(hp, true);
                    let _ = session.send(cmd, MdrDataType::DataMdr).await;
                }
            }
            AppCommand::SetQuickAccess(dbl, trp) => {
                if self.active.is_some() {
                    let mut hp = self.snapshot.lock().unwrap().headphone.clone();
                    hp.quick_access_double_tap = dbl;
                    hp.quick_access_triple_tap = trp;
                    self.apply_headphone_update(hp, true);
                    let _ = self.apply_quick_access_live(dbl, trp).await;
                } else {
                    self.queue_offline(|p| {
                        p.quick_access_double_wire = Some(dbl.0 as i32);
                        p.quick_access_triple_wire = Some(trp.0 as i32);
                    });
                }
            }
            AppCommand::SetPairingMode(enabled) => {
                if let Some(ActiveSession::SonyArq { session, .. }) = &self.active {
                    let _ = session
                        .send(sony_v2::set_pairing_mode(enabled), MdrDataType::DataMdrNo2)
                        .await;
                }
            }
            AppCommand::SetSourceSwitch(enabled) => {
                if let Some(ActiveSession::SonyArq { session, .. }) = &self.active {
                    let _ = session
                        .send(sony_v2::set_source_switch(enabled), MdrDataType::DataMdrNo2)
                        .await;
                }
            }
            AppCommand::SwitchAudioSource(mac) => {
                if let Some(ActiveSession::SonyArq { session, .. }) = &self.active {
                    let _ = session
                        .send(sony_v2::switch_audio_source(&mac), MdrDataType::DataMdrNo2)
                        .await;
                }
            }
            AppCommand::ToggleLockPlaybackSource(mac) => {
                let mut s = self.snapshot.lock().unwrap();
                if s.locked_playback_address
                    .as_ref()
                    .is_some_and(|m| m.eq_ignore_ascii_case(&mac))
                {
                    s.locked_playback_address = None;
                } else {
                    s.locked_playback_address = Some(mac);
                }
            }
            AppCommand::PerformDeviceAction(action, mac) => {
                if let Some(ActiveSession::SonyArq { session, .. }) = &self.active {
                    let _ = session
                        .send(
                            sony_v2::perform_device_action(action, &mac),
                            MdrDataType::DataMdrNo2,
                        )
                        .await;
                }
            }
            AppCommand::PowerOffHeadphones => {
                if let Some(ActiveSession::SonyArq { session, .. }) = &self.active {
                    let is_v1 = self.snapshot.lock().unwrap().headphone.protocol_version
                        == MdrProtocolVersion::V1;
                    let cmd = if is_v1 {
                        sony_v1::power_off_headphones()
                    } else {
                        sony_v2::power_off_headphones()
                    };
                    let _ = session.send(cmd, MdrDataType::DataMdr).await;
                    self.teardown_session();
                    self.snapshot.lock().unwrap().connection_status = ConnectionStatus::Offline;
                }
            }
            AppCommand::SaveAmbientScene {
                name,
                level,
                focus_on_voice,
            } => {
                let trimmed = name.trim();
                if !trimmed.is_empty() {
                    let scene = AmbientScene {
                        id: format!("scene_{}", Utc::now().timestamp_millis()),
                        name: trimmed.to_string(),
                        level: level.clamp(1, 20),
                        focus_on_voice,
                    };
                    let mut s = self.snapshot.lock().unwrap();
                    s.ambient_scenes.push(scene);
                    self.storage.save_ambient_scenes(&s.ambient_scenes);
                }
            }
            AppCommand::DeleteAmbientScene(id) => {
                let mut s = self.snapshot.lock().unwrap();
                s.ambient_scenes.retain(|sc| sc.id != id);
                self.storage.save_ambient_scenes(&s.ambient_scenes);
            }
            AppCommand::SaveVault => {
                let (hp, addr_key) = {
                    let s = self.snapshot.lock().unwrap();
                    (
                        s.headphone.clone(),
                        s.headphone.bluetooth_address.to_uppercase(),
                    )
                };
                let snap = SettingsVaultSnapshot::from_state(&hp, Utc::now().timestamp_millis());
                self.cache_store.vaults.insert(addr_key, snap.clone());
                self.storage.save_cache_store(&self.cache_store);
                let mut s = self.snapshot.lock().unwrap();
                s.vault_snapshot = Some(snap);
                s.vault_diffs.clear();
                s.status_message = Some("Saved current settings to Vault".into());
            }
            AppCommand::RestoreVault => {
                let vault = self.snapshot.lock().unwrap().vault_snapshot.clone();
                if let Some(v) = vault {
                    self.restore_vault(&v).await;
                }
            }
            AppCommand::ClearVault => {
                let addr_key = self
                    .snapshot
                    .lock()
                    .unwrap()
                    .headphone
                    .bluetooth_address
                    .to_uppercase();
                self.cache_store.vaults.remove(&addr_key);
                self.storage.save_cache_store(&self.cache_store);
                let mut s = self.snapshot.lock().unwrap();
                s.vault_snapshot = None;
                s.vault_diffs.clear();
            }
            AppCommand::SetAutoEnforceVault(enabled) => {
                self.config.auto_enforce_vault = enabled;
                self.storage.save_config(&self.config);
                self.snapshot.lock().unwrap().auto_enforce_vault = enabled;
            }
            AppCommand::ExportVault(path) => {
                let vault = self.snapshot.lock().unwrap().vault_snapshot.clone();
                if let Some(v) = vault {
                    let msg = match self.storage.export_vault_to_path(&path, &v) {
                        Ok(()) => format!("Exported Vault to {}", path.display()),
                        Err(e) => format!("Export failed: {}", e),
                    };
                    self.snapshot.lock().unwrap().status_message = Some(msg);
                }
            }
            AppCommand::ImportVault(path) => {
                match self.storage.import_vault_from_path(&path) {
                    Ok(v) => {
                        let addr_key = self
                            .snapshot
                            .lock()
                            .unwrap()
                            .headphone
                            .bluetooth_address
                            .to_uppercase();
                        self.cache_store.vaults.insert(addr_key, v.clone());
                        self.storage.save_cache_store(&self.cache_store);
                        let mut s = self.snapshot.lock().unwrap();
                        s.vault_diffs = v.diffs_from(&s.headphone);
                        s.vault_snapshot = Some(v);
                        s.status_message = Some("Imported Settings Vault JSON".into());
                    }
                    Err(e) => {
                        self.snapshot.lock().unwrap().status_message = Some(e);
                    }
                }
            }
            AppCommand::DiscardPendingChanges => {
                let addr_key = self
                    .snapshot
                    .lock()
                    .unwrap()
                    .headphone
                    .bluetooth_address
                    .to_uppercase();
                self.cache_store.pending.remove(&addr_key);
                self.storage.save_cache_store(&self.cache_store);
                let mut s = self.snapshot.lock().unwrap();
                s.pending_changes = None;
                if s.connection_status == ConnectionStatus::Offline {
                    s.headphone = self.last_cached_base.clone();
                }
            }
            AppCommand::SetThemeMode(mode) => {
                self.config.theme_mode = mode;
                self.storage.save_config(&self.config);
                self.snapshot.lock().unwrap().theme_mode = mode;
            }
            AppCommand::SetMinimizeToTray(enabled) => {
                self.config.minimize_to_tray = enabled;
                self.storage.save_config(&self.config);
                self.snapshot.lock().unwrap().minimize_to_tray = enabled;
            }
            AppCommand::SetHeadphoneAge(age) => {
                let addr_key = self
                    .snapshot
                    .lock()
                    .unwrap()
                    .headphone
                    .bluetooth_address
                    .to_uppercase();
                self.config.per_device_age.insert(addr_key, age);
                self.storage.save_config(&self.config);
                let mut s = self.snapshot.lock().unwrap();
                s.headphone_age = age;
                s.battery_projection = project_battery(
                    s.headphone.battery.effective_level(),
                    s.headphone.battery.is_charging(),
                    s.headphone.noise_control.mode != NoiseControlMode::Off,
                    s.headphone.codec.is_high_draw() || s.headphone.dsee_extreme,
                    age,
                    &s.battery_history,
                    Utc::now().timestamp_millis(),
                    Some(s.headphone.device_profile().battery_baseline),
                );
            }
            AppCommand::SetFinishOverride(ovr) => {
                let addr_key = self
                    .snapshot
                    .lock()
                    .unwrap()
                    .headphone
                    .bluetooth_address
                    .to_uppercase();
                self.config.per_device_finish.insert(addr_key, ovr);
                self.storage.save_config(&self.config);
                self.snapshot.lock().unwrap().finish_override = ovr;
            }
            AppCommand::RespondToAlert {
                message_type,
                accept,
            } => {
                self.snapshot.lock().unwrap().headphone.pending_alert = None;
                if let Some(ActiveSession::SonyArq { session, .. }) = &self.active {
                    let _ = session
                        .send(
                            sony_v2::respond_to_alert(message_type, accept),
                            MdrDataType::DataMdr,
                        )
                        .await;
                }
            }
            AppCommand::DismissAlert => {
                self.snapshot.lock().unwrap().headphone.pending_alert = None;
            }
            AppCommand::ClearStatusMessage => {
                self.snapshot.lock().unwrap().status_message = None;
            }
        }
    }

    async fn restore_vault(&mut self, vault: &SettingsVaultSnapshot) {
        let nc = NoiseControlState {
            mode: vault.noise_mode,
            ambient_level: vault.ambient_level,
            focus_on_voice: vault.focus_on_voice,
        };
        if self.active.is_some() {
            let caps = self.snapshot.lock().unwrap().headphone.capabilities();
            if caps.supports_noise_control {
                let _ = self.apply_noise_control_live(&nc, false).await;
            }
            if caps.supports_equalizer {
                if vault.eq_preset_code == EqualizerPreset::Manual.wire_byte() as i32 {
                    let _ = self
                        .apply_eq_custom_live(vault.eq_clear_bass, &vault.eq_bands, false)
                        .await;
                } else if let Some(preset) =
                    EqualizerPreset::from_byte((vault.eq_preset_code & 0xFF) as u8)
                {
                    let _ = self.apply_eq_preset_live(preset).await;
                }
            }
            if caps.supports_dsee {
                let _ = self.apply_dsee_live(vault.dsee_extreme).await;
            }
            if caps.supports_speak_to_chat {
                let _ = self.apply_speak_to_chat_live(vault.speak_to_chat).await;
            }
            if caps.supports_wearing_detection {
                let _ = self
                    .apply_wearing_detection_live(vault.wearing_detection)
                    .await;
            }
            if caps.supports_touch_panel {
                let _ = self.apply_touch_panel_live(vault.touch_panel_enabled).await;
            }
            if caps.supports_multipoint {
                let _ = self.apply_multipoint_live(vault.multipoint_enabled).await;
            }
            if let Some(cycle) = NcAmbButtonCycle::from_byte(vault.nc_amb_button_cycle_wire as u8)
            {
                if caps.supports_nc_amb_button_cycle {
                    let _ = self.apply_nc_amb_cycle_live(cycle).await;
                }
            }
            if let Some(apo) = AutoPowerOff::from_byte(vault.auto_power_off_wire as u8) {
                if caps.supports_auto_power_off {
                    let _ = self.apply_auto_power_off_live(apo).await;
                }
            }
            let mut hp = self.snapshot.lock().unwrap().headphone.clone();
            hp.noise_control = nc;
            hp.equalizer.preset_code = vault.eq_preset_code;
            hp.equalizer.clear_bass = vault.eq_clear_bass;
            hp.equalizer.bands = vault.eq_bands.clone();
            hp.dsee_extreme = vault.dsee_extreme;
            hp.speak_to_chat = vault.speak_to_chat;
            hp.wearing_detection = vault.wearing_detection;
            self.apply_headphone_update(hp, true);
            self.snapshot.lock().unwrap().status_message = Some("Restored Vault settings".into());
        } else {
            self.queue_offline(|p| {
                p.noise_control = Some(nc);
                p.eq_preset_code = Some(vault.eq_preset_code);
                p.eq_clear_bass = Some(vault.eq_clear_bass);
                p.eq_bands = Some(vault.eq_bands.clone());
                p.dsee_extreme = Some(vault.dsee_extreme);
                p.speak_to_chat = Some(vault.speak_to_chat);
                p.wearing_detection = Some(vault.wearing_detection);
                p.touch_panel_enabled = Some(vault.touch_panel_enabled);
                p.multipoint_enabled = Some(vault.multipoint_enabled);
                p.auto_power_off_wire = Some(vault.auto_power_off_wire);
                p.nc_amb_button_cycle_wire = Some(vault.nc_amb_button_cycle_wire);
            });
            self.snapshot.lock().unwrap().status_message =
                Some("Vault settings queued for next connection".into());
        }
    }

    // MARK: - Live Protocol Helpers

    async fn apply_noise_control_live(
        &self,
        nc: &NoiseControlState,
        dragging: bool,
    ) -> Result<(), String> {
        let Some(active) = &self.active else {
            return Err("Not connected".into());
        };
        match active {
            ActiveSession::SonyArq { session, .. } => {
                let hp = self.snapshot.lock().unwrap().headphone.clone();
                let sub = hp.capabilities().nc_control_subtype;
                let payload = if hp.protocol_version == MdrProtocolVersion::V1 {
                    sony_v1::set_noise_control(nc, sub)
                } else {
                    sony_v2::set_noise_control(nc, dragging, sub)
                };
                if dragging {
                    session.send_latest("nc", payload, MdrDataType::DataMdr);
                    Ok(())
                } else {
                    session
                        .send(payload, MdrDataType::DataMdr)
                        .await
                        .map_err(|e| e.to_string())
                }
            }
            ActiveSession::External(ext) => {
                if !dragging {
                    ext.set_noise_control(nc).await.map_err(|e| e.to_string())?;
                }
                Ok(())
            }
        }
    }

    async fn apply_nc_amb_cycle_live(&self, cycle: NcAmbButtonCycle) -> Result<(), String> {
        if let Some(ActiveSession::SonyArq { session, .. }) = &self.active {
            session
                .send(sony_v2::set_nc_amb_button_cycle(cycle), MdrDataType::DataMdr)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    async fn apply_eq_preset_live(&self, preset: EqualizerPreset) -> Result<(), String> {
        let Some(active) = &self.active else { return Ok(()) };
        match active {
            ActiveSession::SonyArq { session, .. } => {
                let is_v1 = self.snapshot.lock().unwrap().headphone.protocol_version
                    == MdrProtocolVersion::V1;
                let payload = if is_v1 {
                    sony_v1::set_eq_preset(preset.wire_byte() as i32)
                } else {
                    sony_v2::set_eq_preset(preset.wire_byte() as i32)
                };
                session
                    .send(payload, MdrDataType::DataMdr)
                    .await
                    .map_err(|e| e.to_string())
            }
            ActiveSession::External(ext) => ext.set_eq_preset(preset).await.map_err(|e| e.to_string()),
        }
    }

    async fn apply_eq_custom_live(
        &self,
        clear_bass: i32,
        bands: &[i32],
        dragging: bool,
    ) -> Result<(), String> {
        let Some(active) = &self.active else { return Ok(()) };
        match active {
            ActiveSession::SonyArq { session, .. } => {
                let hp = self.snapshot.lock().unwrap().headphone.clone();
                let payload = if hp.protocol_version == MdrProtocolVersion::V1 {
                    sony_v1::set_eq_custom(
                        clear_bass,
                        bands,
                        EqualizerPreset::Manual.wire_byte() as i32,
                        hp.equalizer.step_count,
                        hp.equalizer.has_clear_bass,
                    )
                } else {
                    sony_v2::set_eq_custom(
                        clear_bass,
                        bands,
                        EqualizerPreset::Manual.wire_byte() as i32,
                        hp.equalizer.step_count,
                        hp.equalizer.has_clear_bass,
                    )
                };
                if dragging {
                    session.send_latest("eq", payload, MdrDataType::DataMdr);
                    Ok(())
                } else {
                    session
                        .send(payload, MdrDataType::DataMdr)
                        .await
                        .map_err(|e| e.to_string())
                }
            }
            ActiveSession::External(ext) => {
                if !dragging {
                    ext.set_eq_custom(clear_bass, bands)
                        .await
                        .map_err(|e| e.to_string())?;
                }
                Ok(())
            }
        }
    }

    async fn apply_dsee_live(&self, enabled: bool) -> Result<(), String> {
        if let Some(ActiveSession::SonyArq { session, .. }) = &self.active {
            let is_v1 =
                self.snapshot.lock().unwrap().headphone.protocol_version == MdrProtocolVersion::V1;
            let cmd = if is_v1 {
                sony_v1::set_dsee(enabled)
            } else {
                sony_v2::set_dsee(enabled)
            };
            session
                .send(cmd, MdrDataType::DataMdr)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    async fn apply_connection_priority_live(&self, prio: ConnectionPriority) -> Result<(), String> {
        if let Some(ActiveSession::SonyArq { session, .. }) = &self.active {
            let is_v1 =
                self.snapshot.lock().unwrap().headphone.protocol_version == MdrProtocolVersion::V1;
            let cmd = if is_v1 {
                sony_v1::set_connection_priority(prio)
            } else {
                sony_v2::set_connection_priority(prio)
            };
            session
                .send(cmd, MdrDataType::DataMdr)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    async fn apply_speak_to_chat_live(&self, enabled: bool) -> Result<(), String> {
        if let Some(ActiveSession::SonyArq { session, .. }) = &self.active {
            session
                .send(sony_v2::set_speak_to_chat(enabled), MdrDataType::DataMdr)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    async fn apply_speak_to_chat_config_live(
        &self,
        sens: SpeakToChatSensitivity,
        timeout: SpeakToChatTimeout,
    ) -> Result<(), String> {
        if let Some(ActiveSession::SonyArq { session, .. }) = &self.active {
            session
                .send(
                    sony_v2::set_speak_to_chat_config(sens, timeout),
                    MdrDataType::DataMdr,
                )
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    async fn apply_wearing_detection_live(&self, enabled: bool) -> Result<(), String> {
        if let Some(ActiveSession::SonyArq { session, .. }) = &self.active {
            let is_v1 =
                self.snapshot.lock().unwrap().headphone.protocol_version == MdrProtocolVersion::V1;
            let cmd = if is_v1 {
                sony_v1::set_wearing_detection(enabled)
            } else {
                sony_v2::set_wearing_detection(enabled)
            };
            session
                .send(cmd, MdrDataType::DataMdr)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    async fn apply_touch_panel_live(&self, enabled: bool) -> Result<(), String> {
        let Some(active) = &self.active else { return Ok(()) };
        match active {
            ActiveSession::SonyArq { session, .. } => {
                let slot = self
                    .snapshot
                    .lock()
                    .unwrap()
                    .headphone
                    .touch_panel_setting_id()
                    .unwrap_or(0xD1);
                session
                    .send(sony_v2::set_general_setting(slot, enabled), MdrDataType::DataMdr)
                    .await
                    .map_err(|e| e.to_string())
            }
            ActiveSession::External(ext) => ext
                .set_touch_panel_enabled(enabled)
                .await
                .map_err(|e| e.to_string()),
        }
    }

    async fn apply_multipoint_live(&self, enabled: bool) -> Result<(), String> {
        let Some(active) = &self.active else { return Ok(()) };
        match active {
            ActiveSession::SonyArq { session, .. } => {
                let slot = self
                    .snapshot
                    .lock()
                    .unwrap()
                    .headphone
                    .multipoint_setting_id()
                    .unwrap_or(0xD2);
                session
                    .send(sony_v2::set_general_setting(slot, enabled), MdrDataType::DataMdr)
                    .await
                    .map_err(|e| e.to_string())
            }
            ActiveSession::External(ext) => ext
                .set_multipoint_enabled(enabled)
                .await
                .map_err(|e| e.to_string()),
        }
    }

    async fn apply_auto_power_off_live(&self, mode: AutoPowerOff) -> Result<(), String> {
        let Some(active) = &self.active else { return Ok(()) };
        match active {
            ActiveSession::SonyArq { session, .. } => {
                let hp = self.snapshot.lock().unwrap().headphone.clone();
                let cmd = if hp.protocol_version == MdrProtocolVersion::V1 {
                    sony_v1::set_auto_power_off(mode, hp.auto_power_off_last_timer_byte)
                } else {
                    sony_v2::set_auto_power_off(
                        mode,
                        hp.auto_power_off_last_timer_byte,
                        hp.capabilities().auto_power_off_subtype,
                    )
                };
                session
                    .send(cmd, MdrDataType::DataMdr)
                    .await
                    .map_err(|e| e.to_string())
            }
            ActiveSession::External(ext) => ext
                .set_auto_power_off(mode)
                .await
                .map_err(|e| e.to_string()),
        }
    }

    async fn apply_voice_assistant_live(&self, va: VoiceAssistant) -> Result<(), String> {
        if let Some(ActiveSession::SonyArq { session, .. }) = &self.active {
            session
                .send(sony_v2::set_voice_assistant(va), MdrDataType::DataMdr)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    async fn apply_quick_access_live(
        &self,
        dbl: QuickAccessService,
        trp: QuickAccessService,
    ) -> Result<(), String> {
        if let Some(ActiveSession::SonyArq { session, .. }) = &self.active {
            session
                .send(sony_v2::set_quick_access(dbl, trp), MdrDataType::DataMdr)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    async fn apply_voice_guidance_live(&self, enabled: bool) -> Result<(), String> {
        let Some(active) = &self.active else { return Ok(()) };
        match active {
            ActiveSession::SonyArq { session, .. } => {
                let hp = self.snapshot.lock().unwrap().headphone.clone();
                if hp.protocol_version == MdrProtocolVersion::V1 {
                    session
                        .send(
                            sony_v1::set_voice_guidance(enabled, hp.voice_guidance_language_code),
                            MdrDataType::DataMdr,
                        )
                        .await
                        .map_err(|e| e.to_string())
                } else {
                    session
                        .send(
                            sony_v2::set_voice_guidance(enabled, hp.voice_guidance_language_code),
                            MdrDataType::DataMdrNo2,
                        )
                        .await
                        .map_err(|e| e.to_string())
                }
            }
            ActiveSession::External(ext) => ext
                .set_voice_guidance(enabled)
                .await
                .map_err(|e| e.to_string()),
        }
    }
}
