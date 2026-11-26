use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::{broadcast, oneshot, Mutex as AsyncMutex};

use crate::protocol::mdr_frame::{
    encode_frame, MdrDataType, MdrFrame, MdrStreamDecoder,
};
use crate::protocol::transport::{BluetoothTransport, ProtocolError};

#[derive(Clone, Debug)]
pub struct IncomingMdrMessage {
    pub table: MdrDataType,
    pub payload: Vec<u8>,
}

struct PendingWaiter {
    id: u64,
    table: MdrDataType,
    expected_opcode: u8,
    expected_subtype: Option<u8>,
    tx: oneshot::Sender<Result<Vec<u8>, ProtocolError>>,
}

struct WaiterState {
    next_id: u64,
    ack_waiter: Option<oneshot::Sender<Result<(), ProtocolError>>>,
    response_waiters: Vec<PendingWaiter>,
}

pub struct MdrArqSession {
    transport: Arc<dyn BluetoothTransport>,
    ack_timeout: Duration,
    max_retries: usize,
    send_mutex: AsyncMutex<()>,
    write_mutex: AsyncMutex<()>,
    decoder: MdrStreamDecoder,
    tx_seq: AtomicU8,
    last_rx_seq: Mutex<Option<u8>>,
    waiters: Mutex<WaiterState>,
    messages_tx: broadcast::Sender<IncomingMdrMessage>,
    disconnect_tx: broadcast::Sender<()>,
    reader_active: Arc<AtomicBool>,
    reader_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    latest_payloads: Arc<Mutex<HashMap<String, (MdrDataType, Vec<u8>)>>>,
    active_coalesce_keys: Arc<Mutex<HashSet<String>>>,
}

impl MdrArqSession {
    pub fn new(transport: Arc<dyn BluetoothTransport>) -> Arc<Self> {
        let (messages_tx, _) = broadcast::channel(64);
        let (disconnect_tx, _) = broadcast::channel(8);
        Arc::new(Self {
            transport,
            ack_timeout: Duration::from_millis(750),
            max_retries: 5,
            send_mutex: AsyncMutex::new(()),
            write_mutex: AsyncMutex::new(()),
            decoder: MdrStreamDecoder::new(),
            tx_seq: AtomicU8::new(0),
            last_rx_seq: Mutex::new(None),
            waiters: Mutex::new(WaiterState {
                next_id: 1,
                ack_waiter: None,
                response_waiters: Vec::new(),
            }),
            messages_tx,
            disconnect_tx,
            reader_active: Arc::new(AtomicBool::new(false)),
            reader_handle: Mutex::new(None),
            latest_payloads: Arc::new(Mutex::new(HashMap::new())),
            active_coalesce_keys: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    pub fn subscribe_messages(&self) -> broadcast::Receiver<IncomingMdrMessage> {
        self.messages_tx.subscribe()
    }

    pub fn subscribe_disconnect(&self) -> broadcast::Receiver<()> {
        self.disconnect_tx.subscribe()
    }

    pub fn is_connected(&self) -> bool {
        self.transport.is_connected() && self.reader_active.load(Ordering::SeqCst)
    }

    pub async fn connect(
        self: &Arc<Self>,
        address: &str,
        service_uuid: &str,
    ) -> Result<(), ProtocolError> {
        self.disconnect();
        let transport = Arc::clone(&self.transport);
        let addr = address.to_string();
        let uuid = service_uuid.to_string();
        tokio::task::spawn_blocking(move || transport.connect(&addr, &uuid))
            .await
            .map_err(|e| ProtocolError::Io(e.to_string()))??;

        self.tx_seq.store(0, Ordering::SeqCst);
        *self.last_rx_seq.lock().unwrap() = None;
        self.decoder.clear();
        self.start_reader();
        Ok(())
    }

    fn start_reader(self: &Arc<Self>) {
        self.reader_active.store(true, Ordering::SeqCst);
        let session = Arc::clone(self);
        let handle = tokio::spawn(async move {
            let mut buf = vec![0u8; 1024];
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
                        let frames = session.decoder.feed(&chunk);
                        for frame in frames {
                            session.handle_frame(frame).await;
                        }
                    }
                    _ => break,
                }
                let _ = &mut buf;
            }
            session.reader_active.store(false, Ordering::SeqCst);
            session.fail_all_pending("Bluetooth link closed");
            let _ = session.disconnect_tx.send(());
        });
        *self.reader_handle.lock().unwrap() = Some(handle);
    }

    pub fn disconnect(&self) {
        self.reader_active.store(false, Ordering::SeqCst);
        if let Some(handle) = self.reader_handle.lock().unwrap().take() {
            handle.abort();
        }
        self.transport.close();
        self.decoder.clear();
        self.fail_all_pending("Disconnected");
    }

    pub fn close(&self) {
        self.disconnect();
    }

    pub async fn send(&self, payload: Vec<u8>, table: MdrDataType) -> Result<(), ProtocolError> {
        if !self.transport.is_connected() {
            return Err(ProtocolError::NotConnected);
        }

        let _send_guard = self.send_mutex.lock().await;
        let seq = self.tx_seq.load(Ordering::SeqCst) & 0x01;
        let frame = MdrFrame {
            frame_type: table,
            sequence: seq,
            payload,
        };
        let encoded = encode_frame(&frame)?;

        for _attempt in 0..self.max_retries {
            let (tx, rx) = oneshot::channel();
            {
                let mut w = self.waiters.lock().unwrap();
                w.ack_waiter = Some(tx);
            }

            {
                let _write_guard = self.write_mutex.lock().await;
                let transport = Arc::clone(&self.transport);
                let bytes = encoded.clone();
                tokio::task::spawn_blocking(move || transport.write(&bytes))
                    .await
                    .map_err(|e| ProtocolError::Io(e.to_string()))??;
            }

            if let Ok(Ok(Ok(()))) = tokio::time::timeout(self.ack_timeout, rx).await {
                return Ok(());
            }
        }

        {
            let mut w = self.waiters.lock().unwrap();
            w.ack_waiter = None;
        }
        Err(ProtocolError::Timeout(format!(
            "Timeout waiting for ACK after {} attempts",
            self.max_retries
        )))
    }

    pub async fn query(
        &self,
        payload: Vec<u8>,
        expected_opcode: u8,
        expected_subtype: Option<u8>,
        table: MdrDataType,
        timeout: Duration,
    ) -> Result<Vec<u8>, ProtocolError> {
        let (tx, rx) = oneshot::channel();
        let waiter_id = {
            let mut w = self.waiters.lock().unwrap();
            let id = w.next_id;
            w.next_id += 1;
            w.response_waiters.push(PendingWaiter {
                id,
                table,
                expected_opcode,
                expected_subtype,
                tx,
            });
            id
        };

        if let Err(e) = self.send(payload, table).await {
            let mut w = self.waiters.lock().unwrap();
            w.response_waiters.retain(|pw| pw.id != waiter_id);
            return Err(e);
        }

        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(res)) => res,
            Ok(Err(_)) => Err(ProtocolError::Io("Response waiter dropped".into())),
            Err(_) => {
                let mut w = self.waiters.lock().unwrap();
                w.response_waiters.retain(|pw| pw.id != waiter_id);
                Err(ProtocolError::Timeout(format!(
                    "Timeout waiting for reply 0x{:02X}",
                    expected_opcode
                )))
            }
        }
    }

    pub fn send_latest(self: &Arc<Self>, key: &str, payload: Vec<u8>, table: MdrDataType) {
        {
            let mut map = self.latest_payloads.lock().unwrap();
            map.insert(key.to_string(), (table, payload));
        }
        {
            let mut active = self.active_coalesce_keys.lock().unwrap();
            if !active.insert(key.to_string()) {
                return;
            }
        }

        let session = Arc::clone(self);
        let key_owned = key.to_string();
        tokio::spawn(async move {
            loop {
                let next = {
                    let mut map = session.latest_payloads.lock().unwrap();
                    map.remove(&key_owned)
                };
                match next {
                    Some((tbl, data)) => {
                        if session.send(data, tbl).await.is_err() {
                            break;
                        }
                    }
                    None => {
                        let mut active = session.active_coalesce_keys.lock().unwrap();
                        let map = session.latest_payloads.lock().unwrap();
                        if map.contains_key(&key_owned) {
                            continue;
                        }
                        active.remove(&key_owned);
                        break;
                    }
                }
            }
            session
                .active_coalesce_keys
                .lock()
                .unwrap()
                .remove(&key_owned);
        });
    }

    async fn handle_frame(&self, frame: MdrFrame) {
        if frame.frame_type == MdrDataType::Ack {
            self.tx_seq.store(frame.sequence & 0x01, Ordering::SeqCst);
            let ack_tx = {
                let mut w = self.waiters.lock().unwrap();
                w.ack_waiter.take()
            };
            if let Some(tx) = ack_tx {
                let _ = tx.send(Ok(()));
            }
            return;
        }

        let ack_frame = MdrFrame {
            frame_type: MdrDataType::Ack,
            sequence: 1 - (frame.sequence & 0x01),
            payload: Vec::new(),
        };
        if let Ok(ack_bytes) = encode_frame(&ack_frame) {
            let _write_guard = self.write_mutex.lock().await;
            let transport = Arc::clone(&self.transport);
            let res = tokio::task::spawn_blocking(move || transport.write(&ack_bytes)).await;
            if !matches!(res, Ok(Ok(()))) {
                return;
            }
        }

        {
            let mut last = self.last_rx_seq.lock().unwrap();
            if *last == Some(frame.sequence) {
                return;
            }
            *last = Some(frame.sequence);
        }

        let payload = frame.payload;
        if payload.is_empty() {
            return;
        }

        let opcode = payload[0];
        let subtype = payload.get(1).copied();

        let matched = {
            let mut w = self.waiters.lock().unwrap();
            let pos = w.response_waiters.iter().position(|pw| {
                pw.table == frame.frame_type
                    && pw.expected_opcode == opcode
                    && (pw.expected_subtype.is_none() || pw.expected_subtype == subtype)
            });
            pos.map(|idx| w.response_waiters.remove(idx))
        };

        if let Some(pw) = matched {
            let _ = pw.tx.send(Ok(payload.clone()));
        }

        let _ = self.messages_tx.send(IncomingMdrMessage {
            table: frame.frame_type,
            payload,
        });
    }

    fn fail_all_pending(&self, reason: &str) {
        let mut w = self.waiters.lock().unwrap();
        if let Some(ack_tx) = w.ack_waiter.take() {
            let _ = ack_tx.send(Err(ProtocolError::Io(reason.to_string())));
        }
        for pw in w.response_waiters.drain(..) {
            let _ = pw.tx.send(Err(ProtocolError::Io(reason.to_string())));
        }
    }
}
