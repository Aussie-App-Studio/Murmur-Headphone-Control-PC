# Murmur — Headphone Control for PC

**Strictly offline native Windows headphone control written in Rust.**

No Electron, no WebView2, no accounts, and no background cloud telemetry — compiled into a single **4.55 MB** standalone Windows executable with a hardware-accelerated OpenGL 3D showcase and a system tray quick controller.

## Features
- **3D PBR Headphone Showcase:** Procedural over-ear (`WH-1000XM`) and earbud (`WF-1000XM`) 3D meshes lit with Cook-Torrance GGX shading, ACES tone mapping, drag-to-spin inertia, custom finish swatches, and mode-reactive studio aura.
- **Full Native Bluetooth Protocol Support (Windows RFCOMM):**
  - **Sony MDR V2** (`WH-1000XM6`, `WH-1000XM5`, `WH-1000XM4`, `WF-1000XM5`, `WF-1000XM4`, `ULT WEAR`, `WH-CH720N`, `LinkBuds S`, etc.) — Acoustic Control (ANC / Ambient / Off + Focus on Voice + 20-step precision scrubber), 5/8/10-band Equalizer + Clear Bass, Speak-to-Chat, DSEE Extreme, Multipoint audio switching, Touch Controls, Wear Detection, Auto Power-Off, NC/AMB Button Cycle.
  - **Sony MDR V1** (`WH-1000XM3`, `WH-1000XM2`, `WH-H900N`, `MDR-1000X`, etc.)
  - **Bose BMAP** (`QC45`, `QC Ultra`, `NC 700`, `QC35 II`, `QC Earbuds`, etc.)
  - **Anker Soundcore** (`Q30`, `Q35`, `Q45`, `Space One`, `Liberty 4`, etc.)
  - **Samsung Galaxy Buds** (`Buds2`, `Buds2 Pro`, `Buds Pro`, `Buds Live`, `Buds FE`, `Buds3 Pro`, etc.)
- **Apple AirPods & Beats Battery Monitor (Driverless BLE):**
  - Passive Windows WinRT BLE watcher reads live Left / Right / Case battery percentages and charging status via Apple Continuity (`0x004C`) advertisements — no kernel driver, no Secure Boot changes, and no Windows Test Mode required.
- **Offline Queue & Settings Vault:**
  - Make changes while your headphones are off and Murmur automatically pushes queued settings on the next connection.
  - Export and import `SettingsVaultSnapshot` JSON files directly compatible with **Murmur for Android**.
- **Low-Overhead System Tray:**
  - Minimize to tray on close for instant right-click ANC / Ambient / Off switching at **0% idle CPU/GPU**.

## Building on Windows

Requires Rust (`stable-msvc`) and Windows 10/11 SDK:

```powershell
cargo test
cargo build --release
```

The standalone binary outputs to `target\release\murmur.exe`.
