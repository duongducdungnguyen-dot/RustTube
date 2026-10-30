# RustTube

A high-performance, distraction-free personal YouTube desktop client built in Rust 2021 for Windows 11. Designed specifically for low RAM usage (<150 MB idle), instant startup, native GPU decoding (`hwdec=auto-safe`), and direct Google OAuth2 + Windows Credential Manager integration.

---

## Key Features

- **Blazing Fast**: Slint native GUI compiled directly to native code without Chromium, Electron, or WebViews.
- **Hardware-Accelerated Playback**: Embedded `libmpv` with hardware decoding (`hwdec=auto-safe`) utilizing your NVIDIA RTX / AMD Radeon GPU.
- **Dedicated Watch Page & Dual-Stream Playback**: Automatically merges separate 1080p/4K DASH video and high-bitrate audio streams synchronously via mpv without re-encoding lag.
- **Minimal RAM Footprint**: Idle memory consumption consistently under 150 MB.
- **yt-dlp Extraction**: Asynchronous subprocess extraction via `tokio::process` with JSON parsing and `--cookies-from-browser` support (Chrome, Edge, Firefox).
- **Google Account Sync**: OAuth2 Authorization Code flow with PKCE on loopback `http://127.0.0.1:8989/callback`. Refresh tokens are securely stored in the Windows Credential Manager via `keyring`.
- **Subscriptions & Playlists**: Official YouTube Data API v3 integration for Subscriptions, Playlists, and Liked videos.
- **Local Watch Later & History**: Private, persistent local queues stored locally without pretending to sync restricted non-API YouTube features.
- **Keyboard Shortcuts**: `Space` (Play/Pause), `J`/`K`/`L` (Back 10s / Pause / Forward 10s), `F` (Fullscreen), `M` (Mute), `Arrow Keys` (Seek / Volume).

---

## Prerequisites

- **OS**: Windows 11 x86_64
- **Rust Toolchain**: `rustup default stable-x86_64-pc-windows-msvc`
- **C++ Build Tools**: Visual Studio 2022 C++ Build Tools (or Desktop development with C++)
- **Binaries**: `yt-dlp.exe` and `libmpv-2.dll` in `./bin/` (downloaded automatically by `setup.ps1`)

---

## Google Cloud Console Configuration (OAuth2 PKCE)

To connect your personal Google / YouTube account:

1. Visit the [Google Cloud Console](https://console.cloud.google.com/).
2. Create or select a project (e.g. `Personal-RustTube`).
3. Under **APIs & Services** > **Library**, search for and enable:
   - **YouTube Data API v3**
4. Under **APIs & Services** > **OAuth consent screen**:
   - User Type: **External** (in Testing mode is fine for personal use).
   - Add your personal Gmail address to the **Test users** list.
   - Add scopes:
     - `https://www.googleapis.com/auth/youtube.readonly`
     - `https://www.googleapis.com/auth/youtube`
5. Under **APIs & Services** > **Credentials**:
   - Click **Create Credentials** -> **OAuth client ID**.
   - Application type: **Desktop app**.
   - Name: `RustTube Windows Client`.
   - Authorized redirect URIs: `http://127.0.0.1:8989/callback` and `http://localhost:8989/callback`.
6. Copy the **Client ID** (e.g. `123456789-abcdef.apps.googleusercontent.com`).
7. Paste it into `config.toml`:
   ```toml
   [auth]
   google_client_id = "123456789-abcdef.apps.googleusercontent.com"
   redirect_port = 8989
   keyring_service_name = "RustTube_Google_OAuth"
   ```

---

## Quick Start (Windows 11)

1. Open PowerShell in the project directory and run the setup script:
   ```powershell
   powershell -ExecutionPolicy Bypass -File setup.ps1
   ```
   *This automatically creates `./bin/` and downloads `yt-dlp.exe` and `libmpv-2.dll`.*

2. Initialize your configuration:
   ```powershell
   Copy-Item config.example.toml config.toml
   ```

3. Build and launch:
   ```powershell
   cargo run --release
   ```

---

## Architecture

The project is structured as a modular Cargo workspace:

- `crates/app`: Slint UI templates and main Tokio event loop; manages native HWND binding to mpv.
- `crates/core`: Application state models, URL parsers, configuration loading, and event definitions.
- `crates/player`: `libmpv` render API wrapper, audio/video dual-stream loading, atomic pause cycle, volume, speed, and seek handling.
- `crates/ytdlp`: Asynchronous `yt-dlp` process controller, stream URL resolution, format inspection, and search queries.
- `crates/auth`: PKCE OAuth2 flow, loopback callback HTTP listener with 120s timeout, and Windows Credential Manager token storage.
- `crates/api`: YouTube Data API v3 client for channel details, subscriptions, and playlists.
- `crates/cache`: Disk LRU thumbnail cache and local persistent JSON history / watch later stores.
- `server.ts` & `src/App.tsx`: Full-stack web companion providing live interactive preview and testing.
