use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;

use crate::models::{AudioCodec, HeadphoneCapabilities, HeadphoneState, MdrProtocolVersion};
use crate::protocol::transport::{HeadphoneSession, ProtocolError};

pub struct GenericBluetoothSession {
    connected: AtomicBool,
    state: Mutex<HeadphoneState>,
    state_tx: broadcast::Sender<HeadphoneState>,
    disconnect_tx: broadcast::Sender<()>,
}

impl GenericBluetoothSession {
    pub fn new() -> Arc<Self> {
        let (state_tx, _) = broadcast::channel(8);
        let (disconnect_tx, _) = broadcast::channel(4);
        Arc::new(Self {
            connected: AtomicBool::new(false),
            state: Mutex::new(HeadphoneState::default()),
            state_tx,
            disconnect_tx,
        })
    }
}

#[async_trait::async_trait]
impl HeadphoneSession for Arc<GenericBluetoothSession> {
    fn protocol_version(&self) -> MdrProtocolVersion {
        MdrProtocolVersion::GenericBt
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
        self.connected.store(true, Ordering::SeqCst);
        let mut st = self.state.lock().unwrap();
        *st = HeadphoneState {
            model_name: if device_name.trim().is_empty() {
                "Bluetooth Audio".to_string()
            } else {
                device_name.trim().to_string()
            },
            firmware_version: "Standard Bluetooth".to_string(),
            bluetooth_address: address.to_string(),
            protocol_version: MdrProtocolVersion::GenericBt,
            codec: AudioCodec::Aac,
            custom_capabilities: Some(HeadphoneCapabilities::generic_bluetooth()),
            ..Default::default()
        };
        Ok(st.clone())
    }

    async fn poll_battery(&self) -> Result<(), ProtocolError> {
        Ok(())
    }

    fn close(&self) {
        self.connected.store(false, Ordering::SeqCst);
    }
}
