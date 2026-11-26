# Murmur — Headphone Control for PC

**Control your Bluetooth headphones right from Windows — no sign-in, no internet, and no bulky background software.**

Normally, headphone apps only exist on your phone, or force you to create an account just to change Noise Cancelling or tweak your EQ. **Murmur** is a tiny (**4.55 MB**) Windows app that puts your headphones' real controls straight on your desktop and in your system tray.

## What You Can Do
- **Noise Cancelling & Ambient Sound:** Switch modes or fine-tune how much outside sound comes in, with one-click voice focus and quick presets.
- **Equalizer & Bass:** Pick from built-in sound presets or drag the sliders yourself to save your own sound profiles.
- **Real Battery & Time Left:** See accurate battery percentages (including Left, Right, and Charging Case for earbuds) plus an estimate of how many hours are left.
- **Quick One-Tap Switches:** Toggle **Speak-to-Chat**, **Wearing Detection** (auto-pause), **Touch Controls**, **Multipoint**, and **DSEE** upscaling without touching your phone.
- **Change Settings Even When Offline:** Edit your settings anytime — Murmur saves them and applies them automatically next time your headphones connect.
- **Sits Quietly in Your Tray:** Close the window to keep Murmur in your Windows taskbar tray (using **0% CPU** in the background) so you can switch Noise Cancelling on a call in one right-click.
- **Dark & Light Modes:** Automatically matches Windows, with custom color finishes for the interactive 3D headphone view.

## Which Headphones Work?
- **Full Control (Noise Cancelling, EQ, Battery & Smart Features):**
  - **Sony:** WH-1000XM6, WH-1000XM5, WH-1000XM4, WH-1000XM3, WH-1000XM2, WF-1000XM5, WF-1000XM4, WF-1000XM3, ULT WEAR, WH-CH720N, LinkBuds S, and more.
  - **Bose:** QuietComfort 45, QuietComfort Ultra, NC 700, QC35 II, QuietComfort Earbuds.
  - **Anker Soundcore:** Life Q30, Q35, Q45, Space One, Liberty 4, and more.
  - **Samsung Galaxy Buds:** Buds2, Buds2 Pro, Buds Pro, Buds Live, Buds FE, Buds3 Pro.
- **Live Battery & Charging (Left / Right / Case %):**
  - **Apple AirPods & Beats:** AirPods (1/2/3/4), AirPods Pro (1/2/3), AirPods Max, Powerbeats Pro, Beats Studio Pro, Beats Fit Pro, Beats Studio Buds/+, Beats Solo 4. *(Microsoft restricts active Noise Cancelling control over AirPods on Windows without kernel test-mode drivers, so Murmur keeps things safe and driver-free.)*

## How to Use
1. Connect your headphones to Windows as usual (**Windows Settings → Bluetooth & devices**).
2. Open **Murmur** (`murmur.exe`) — it finds and connects to your headphones automatically.
3. Have **Murmur on Android** too? Use **Save Vault / Export** to copy your exact favorite settings between your phone and PC with one JSON file.

---

## Technical Details & Developer Reference

<details>
<summary>Protocol Support, Architecture & Building from Source</summary>

### Architecture
- **Language & UI:** Pure Rust (`eframe` / `egui 0.31`, OpenGL via `glow 0.16` / `egui_glow`). Zero Electron, zero WebView2 runtime, single standalone binary (`~4.55 MB` with full LTO, `opt-level = "s"`, `strip = true`, `panic = "abort"`).
- **3D Hardware Renderer:** Procedural over-ear (`WH-1000XM`) and dual-earbud (`WF-1000XM`) meshes lit with Cook-Torrance GGX PBR shaders and ACES tone mapping (`#version 330 core`), paced at 30 FPS passive / 60 FPS interactive drag and paused when minimized, unfocused, or in the tray (`0.0%` hidden CPU).
- **Bluetooth Stack:**
  - Native Windows WinSock RFCOMM (`AF_BTH` / `BTHPROTO_RFCOMM`) for Sony MDR V2 (Table 1 & 2) & MDR V1, Bose BMAP, Anker Soundcore, and Samsung Galaxy Buds.
  - Driverless WinRT BLE Advertisement Watcher (`BluetoothLEAdvertisementWatcher`) decoding Apple Continuity Proximity Pairing (`0x004C`, `0x07 0x19`) payloads for AirPods & Beats battery/charging telemetry without kernel drivers or Secure Boot changes.
  - Local offline queue (`PendingOfflineChanges`), EWMA battery drain calibration, and `SettingsVaultSnapshot` camelCase JSON parity with **Murmur for Android** stored in `%APPDATA%\Murmur`.

### Build on Windows
Requires Rust (`stable-msvc`) and Windows 10/11 SDK:

```powershell
cargo test
cargo build --release
```

Outputs `target\release\murmur.exe`.

</details>
