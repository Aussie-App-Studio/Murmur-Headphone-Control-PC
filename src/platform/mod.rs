#[cfg(windows)]
pub mod windows_bt;

#[cfg(windows)]
pub use windows_bt::{
    apply_windows_titlebar_theme, enumerate_paired_devices, is_windows_dark_theme,
    WindowsRfcommTransport,
};

#[cfg(not(windows))]
mod stub {
    use crate::protocol::{BluetoothTransport, BondedDevice, ProtocolError};

    pub fn enumerate_paired_devices(_preferred_address: Option<&str>) -> Vec<BondedDevice> {
        Vec::new()
    }

    pub fn is_windows_dark_theme() -> bool {
        true
    }

    pub fn apply_windows_titlebar_theme(_dark: bool, _caption_rgb: (u8, u8, u8)) {}

    pub struct WindowsRfcommTransport;
    impl WindowsRfcommTransport {
        pub fn new() -> Self {
            Self
        }
    }

    impl BluetoothTransport for WindowsRfcommTransport {
        fn is_connected(&self) -> bool {
            false
        }
        fn connect(&self, _address: &str, _service_uuid: &str) -> Result<(), ProtocolError> {
            Err(ProtocolError::Io("Windows Bluetooth is only available on Windows".into()))
        }
        fn read(&self, _buf: &mut [u8]) -> Result<usize, ProtocolError> {
            Err(ProtocolError::NotConnected)
        }
        fn write(&self, _bytes: &[u8]) -> Result<(), ProtocolError> {
            Err(ProtocolError::NotConnected)
        }
        fn close(&self) {}
    }
}

#[cfg(not(windows))]
pub use stub::{
    apply_windows_titlebar_theme, enumerate_paired_devices, is_windows_dark_theme,
    WindowsRfcommTransport,
};
