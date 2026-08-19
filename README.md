# KidsTime Pro (Rust)

Parental control for Windows with remote phone monitoring. Rewritten in Rust for small binary size and native performance.

## Features

- Colored timer (green / yellow / red) with daily and session limits
- Screen lock when time is up
- Mobile web panel (auto-refresh every 5s)
- Active application tracking
- App usage statistics per day
- Activity log
- Remote lock and add time from phone
- Cloud relay for access from anywhere (no port forwarding)
- Local WiFi access
- Schedule support

## Build from source

### Prerequisites

- [Rust toolchain](https://rustup.rs/) (`rustup default stable-msvc`)
- Windows 10/11

### Build

```bash
cd kidstime-rs
cargo build --release
```

The binary will be at `target/release/kidstime-pro.exe` (~3-5 MB).

## Build .exe (one-liner)

```bash
cargo build --release --target x86_64-pc-windows-msvc
```

## Quick start

1. Run `kidstime-pro.exe`
2. Set admin password on first launch
3. Open the web URL shown in the app on your phone
4. Enter the token to connect

## Remote access

### Cloud relay (recommended)

1. Deploy `relay_server.py` (from the Python version) on any VPS
2. In Settings, enable Cloud Access
3. Enter relay URL and device code
4. Open `https://your-server/DEVICE_CODE` on your phone

### Local network

Open `http://LOCAL_IP:8080` on your phone (same WiFi).

## Tech stack

- **GUI**: [egui/eframe](https://github.com/emilk/egui)
- **Web server**: [axum](https://github.com/tokio-rs/axum)
- **WebSocket relay**: [tokio-tungstenite](https://github.com/snapview/tokio-tungstenite)
- **Windows API**: [windows-rs](https://github.com/microsoft/windows-rs)
- **Serialization**: [serde](https://serde.rs/) + [serde_json](https://github.com/serde-rs/json)

## Binary size comparison

| Version  | Size    |
|----------|---------|
| Python (PyInstaller) | ~40 MB |
| Rust (release, stripped) | ~3-5 MB |

## License

MIT