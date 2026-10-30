# RustTube

A high-performance, distraction-free personal YouTube desktop client built in Rust 2021 for Windows 11. Designed specifically for low RAM usage (<150 MB idle), instant startup, native GPU decoding (`hwdec=auto-safe`), and direct Google OAuth2 + Windows Credential Manager integration.

## Key Features

- **Blazing Fast**: Slint native rendering compiled directly to native code without Chromium or WebViews.
- **Hardware-Accelerated Playback**: Embedded `libmpv` with hardware decoding (`hwdec=auto-safe`) prioritizing NVIDIA RTX / AMD Radeon GPU decoding.
- **Minimal RAM Footprint**: Idle memory consumption under 150 MB.
- **yt-dlp Extraction**: Asynchronous subprocess extraction via `tokio::process` with JSON parsing and `--cookies-from-browser` support (Chrome, Edge, Firefox).
- **Google Account Sync**: OAuth2 Authorization Code flow with PKCE on loopback `127.0.0.1`. Refresh tokens are securely stored in the Windows Credential Manager via `keyring`.
- **Subscriptions & Playlists**: Official YouTube Data API v3 integration for Subscriptions, Playlists, and Liked videos.
- **Local Watch Later & History**: Private, persistent local queues stored locally without pretending to sync restricted non-API YouTube features.
- **Keyboard Shortcuts**: `Space` (Play/Pause), `J`/`K`/`L` (Back 10s / Pause / Forward 10s), `F` (Fullscreen), `M` (Mute), `Arrow Keys` (Seek / Volume).

## Requirements

- Windows 11 x86_64
- Rust toolchain (`rustup default stable-x86_64-pc-windows-msvc`)
- Visual Studio 2022 C++ Build Tools
- `yt-dlp.exe` and `libmpv-2.dll` in `./bin/` (automatic via `setup.ps1`)

## Quick Start

1. Run the dependency setup script:
   ```powershell
   powershell -ExecutionPolicy Bypass -File setup.ps1
   ```

2. Configure your Google OAuth client:
   Copy `config.example.toml` to `config.toml` and set your `google_client_id`.

3. Build and launch:
   ```powershell
   cargo run --release
   ```

## Architecture

The project is structured as a modular Cargo workspace:
- `crates/app`: Slint UI templates and main application loop.
- `crates/core`: Application state models and event messaging.
- `crates/player`: `libmpv` render API wrapper and playback control.
- `crates/ytdlp`: Async `yt-dlp` child process manager and parser.
- `crates/auth`: PKCE OAuth2 flow, loopback server, and Windows Credential Manager keyring.
- `crates/api`: YouTube Data API v3 client.
- `crates/cache`: Disk LRU thumbnail cache and local history storage.
