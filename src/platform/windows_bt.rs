use std::ffi::c_void;
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, Once};

use windows::core::GUID;
use windows::Win32::Devices::Bluetooth::{
    BluetoothFindDeviceClose, BluetoothFindFirstDevice, BluetoothFindNextDevice,
    BLUETOOTH_DEVICE_INFO, BLUETOOTH_DEVICE_SEARCH_PARAMS,
};
use windows::Win32::Foundation::{BOOL, COLORREF, HANDLE, HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_CAPTION_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE,
};
use windows::Win32::Networking::WinSock::{
    closesocket, connect, recv, send, shutdown, socket, WSAStartup, INVALID_SOCKET, SD_BOTH,
    SOCKET, SOCK_STREAM, WSADATA,
};
use windows::Win32::System::Registry::{
    RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowRect, GetWindowThreadProcessId, IsWindowVisible,
};

use crate::models::MdrProtocolVersion;
use crate::protocol::transport::{classify_device, BluetoothTransport, BondedDevice, ProtocolError};

const AF_BTH: u16 = 32;
const BTHPROTO_RFCOMM: i32 = 3;

#[repr(C, packed)]
#[allow(non_snake_case)]
struct SOCKADDR_BTH {
    addressFamily: u16,
    btAddr: u64,
    serviceClassId: GUID,
    port: u32,
}

static WSA_INIT: Once = Once::new();

fn ensure_wsa_started() {
    WSA_INIT.call_once(|| unsafe {
        let mut data = WSADATA::default();
        let _ = WSAStartup(0x0202, &mut data);
    });
}

fn format_bth_address(addr: u64) -> String {
    let bytes = addr.to_be_bytes();
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]
    )
}

fn parse_bth_address(address: &str) -> Result<u64, ProtocolError> {
    let hex: String = address
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect();
    if hex.len() != 12 {
        return Err(ProtocolError::Io(format!(
            "Invalid Bluetooth MAC address: {}",
            address
        )));
    }
    u64::from_str_radix(&hex, 16)
        .map_err(|_| ProtocolError::Io(format!("Invalid Bluetooth MAC address: {}", address)))
}

fn parse_guid(uuid: &str) -> Result<GUID, ProtocolError> {
    let clean: String = uuid.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if clean.len() != 32 {
        return Err(ProtocolError::Io(format!("Invalid service UUID: {}", uuid)));
    }
    let data1 = u32::from_str_radix(&clean[0..8], 16)
        .map_err(|_| ProtocolError::Io(format!("Invalid UUID: {}", uuid)))?;
    let data2 = u16::from_str_radix(&clean[8..12], 16)
        .map_err(|_| ProtocolError::Io(format!("Invalid UUID: {}", uuid)))?;
    let data3 = u16::from_str_radix(&clean[12..16], 16)
        .map_err(|_| ProtocolError::Io(format!("Invalid UUID: {}", uuid)))?;
    let mut data4 = [0u8; 8];
    for i in 0..8 {
        data4[i] = u8::from_str_radix(&clean[16 + i * 2..18 + i * 2], 16)
            .map_err(|_| ProtocolError::Io(format!("Invalid UUID: {}", uuid)))?;
    }
    Ok(GUID::from_values(data1, data2, data3, data4))
}

pub fn enumerate_paired_devices(preferred_address: Option<&str>) -> Vec<BondedDevice> {
    let mut devices = Vec::new();
    unsafe {
        let search_params = BLUETOOTH_DEVICE_SEARCH_PARAMS {
            dwSize: size_of::<BLUETOOTH_DEVICE_SEARCH_PARAMS>() as u32,
            fReturnAuthenticated: BOOL(1),
            fReturnRemembered: BOOL(1),
            fReturnUnknown: BOOL(0),
            fReturnConnected: BOOL(1),
            fIssueInquiry: BOOL(0),
            cTimeoutMultiplier: 0,
            hRadio: HANDLE::default(),
        };
        let mut device_info = BLUETOOTH_DEVICE_INFO {
            dwSize: size_of::<BLUETOOTH_DEVICE_INFO>() as u32,
            ..Default::default()
        };

        let Ok(find_handle) = BluetoothFindFirstDevice(&search_params, &mut device_info) else {
            return devices;
        };
        if find_handle.is_invalid() {
            return devices;
        }

        loop {
            let name_len = device_info
                .szName
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(device_info.szName.len());
            let name = String::from_utf16_lossy(&device_info.szName[..name_len]);
            let addr = format_bth_address(device_info.Address.Anonymous.ullLong);
            let is_connected = device_info.fConnected.as_bool();
            let cod = device_info.ulClassofDevice;

            if let Some(dev) = classify_device(&name, &addr, is_connected, cod) {
                devices.push(dev);
            }

            device_info = BLUETOOTH_DEVICE_INFO {
                dwSize: size_of::<BLUETOOTH_DEVICE_INFO>() as u32,
                ..Default::default()
            };
            if BluetoothFindNextDevice(find_handle, &mut device_info).is_err() {
                break;
            }
        }
        let _ = BluetoothFindDeviceClose(find_handle);
    }

    devices.sort_by(|a, b| {
        let a_pref = preferred_address.is_some_and(|p| p.eq_ignore_ascii_case(&a.address));
        let b_pref = preferred_address.is_some_and(|p| p.eq_ignore_ascii_case(&b.address));
        let a_prop = a.detected_protocol != MdrProtocolVersion::GenericBt;
        let b_prop = b.detected_protocol != MdrProtocolVersion::GenericBt;
        b.is_connected
            .cmp(&a.is_connected)
            .then_with(|| b_pref.cmp(&a_pref))
            .then_with(|| b_prop.cmp(&a_prop))
            .then_with(|| a.name.cmp(&b.name))
    });
    devices
}

pub fn is_windows_dark_theme() -> bool {
    unsafe {
        let subkey = windows::core::w!(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"
        );
        let value = windows::core::w!("AppsUseLightTheme");
        let mut data: u32 = 0;
        let mut data_size: u32 = size_of::<u32>() as u32;
        let status = RegGetValueW(
            HKEY_CURRENT_USER,
            subkey,
            value,
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut u32 as *mut c_void),
            Some(&mut data_size),
        );
        if status.is_ok() {
            data == 0
        } else {
            true
        }
    }
}

pub fn apply_windows_titlebar_theme(dark: bool, caption_rgb: (u8, u8, u8)) {
    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let state = &mut *(lparam.0 as *mut (u32, HWND));
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == state.0 && IsWindowVisible(hwnd).as_bool() {
            let mut r = RECT::default();
            if GetWindowRect(hwnd, &mut r).is_ok()
                && (r.right - r.left) > 300
                && (r.bottom - r.top) > 300
            {
                state.1 = hwnd;
                return BOOL(0);
            }
        }
        BOOL(1)
    }

    unsafe {
        let mut state = (std::process::id(), HWND::default());
        let _ = EnumWindows(Some(enum_proc), LPARAM(&mut state as *mut _ as isize));
        let hwnd = state.1;
        if hwnd.is_invalid() || hwnd.0.is_null() {
            return;
        }
        let dark_val: BOOL = BOOL(if dark { 1 } else { 0 });
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark_val as *const BOOL as *const c_void,
            size_of::<BOOL>() as u32,
        );
        let colorref = COLORREF(
            ((caption_rgb.2 as u32) << 16)
                | ((caption_rgb.1 as u32) << 8)
                | (caption_rgb.0 as u32),
        );
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_CAPTION_COLOR,
            &colorref as *const COLORREF as *const c_void,
            size_of::<COLORREF>() as u32,
        );
    }
}

pub struct WindowsRfcommTransport {
    socket: Mutex<Option<SOCKET>>,
    connected: AtomicBool,
}

impl Default for WindowsRfcommTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowsRfcommTransport {
    pub fn new() -> Self {
        Self {
            socket: Mutex::new(None),
            connected: AtomicBool::new(false),
        }
    }
}

impl Drop for WindowsRfcommTransport {
    fn drop(&mut self) {
        self.close();
    }
}

impl BluetoothTransport for WindowsRfcommTransport {
    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    fn connect(&self, address: &str, service_uuid: &str) -> Result<(), ProtocolError> {
        self.close();
        ensure_wsa_started();

        let bt_addr = parse_bth_address(address)?;
        let guid = parse_guid(service_uuid)?;

        unsafe {
            let sock = socket(AF_BTH as i32, SOCK_STREAM, BTHPROTO_RFCOMM)
                .map_err(|e| ProtocolError::Io(format!("Failed to create RFCOMM socket: {}", e)))?;
            if sock == INVALID_SOCKET {
                return Err(ProtocolError::Io("Invalid RFCOMM socket".into()));
            }

            let sockaddr = SOCKADDR_BTH {
                addressFamily: AF_BTH,
                btAddr: bt_addr,
                serviceClassId: guid,
                port: 0, // Let Windows resolve RFCOMM channel via SDP
            };

            let rc = connect(
                sock,
                &sockaddr as *const SOCKADDR_BTH as *const _,
                size_of::<SOCKADDR_BTH>() as i32,
            );
            if rc != 0 {
                let err = windows::Win32::Networking::WinSock::WSAGetLastError();
                let _ = closesocket(sock);
                return Err(ProtocolError::Io(format!(
                    "RFCOMM connect to {} failed (WSA error {})",
                    address, err.0
                )));
            }

            *self.socket.lock().unwrap() = Some(sock);
            self.connected.store(true, Ordering::SeqCst);
        }
        Ok(())
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize, ProtocolError> {
        let sock = {
            let guard = self.socket.lock().unwrap();
            match *guard {
                Some(s) => s,
                None => return Err(ProtocolError::NotConnected),
            }
        };
        let n = unsafe { recv(sock, buf, Default::default()) };
        if n > 0 {
            Ok(n as usize)
        } else if n == 0 {
            self.connected.store(false, Ordering::SeqCst);
            Err(ProtocolError::Io("RFCOMM peer closed connection".into()))
        } else {
            let err = unsafe { windows::Win32::Networking::WinSock::WSAGetLastError() };
            self.connected.store(false, Ordering::SeqCst);
            Err(ProtocolError::Io(format!("RFCOMM recv error {}", err.0)))
        }
    }

    fn write(&self, bytes: &[u8]) -> Result<(), ProtocolError> {
        let sock = {
            let guard = self.socket.lock().unwrap();
            match *guard {
                Some(s) => s,
                None => return Err(ProtocolError::NotConnected),
            }
        };
        let mut offset = 0;
        while offset < bytes.len() {
            let n = unsafe { send(sock, &bytes[offset..], Default::default()) };
            if n <= 0 {
                let err = unsafe { windows::Win32::Networking::WinSock::WSAGetLastError() };
                self.connected.store(false, Ordering::SeqCst);
                return Err(ProtocolError::Io(format!("RFCOMM send error {}", err.0)));
            }
            offset += n as usize;
        }
        Ok(())
    }

    fn close(&self) {
        self.connected.store(false, Ordering::SeqCst);
        let sock = self.socket.lock().unwrap().take();
        if let Some(s) = sock {
            unsafe {
                let _ = shutdown(s, SD_BOTH);
                let _ = closesocket(s);
            }
        }
    }
}
