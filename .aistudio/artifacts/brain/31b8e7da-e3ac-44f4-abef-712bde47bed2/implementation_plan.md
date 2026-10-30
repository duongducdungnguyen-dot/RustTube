# RustTube — High-Performance Native YouTube Desktop Client

A lightweight, personal native Windows 11 desktop YouTube client engineered in Rust (edition 2021) utilizing Slint UI for zero-bloat GPU rendering, embedded `libmpv` with `hwdec=auto-safe`, `yt-dlp` async process extraction, Google OAuth2 with PKCE, and local Windows Credential Manager integration. Designed exclusively for single-user personal efficiency with sub-150MB idle memory consumption.

---

## User Review & Critical Decisions

> [!IMPORTANT]
> RustTube is tailored strictly for personal desktop use on Windows 11 (AMD Ryzen 5 6600H, NVIDIA RTX 3050 Ti, 8GB RAM). Zero webview/Chromium overhead, zero background telemetry, and zero multi-user abstraction.

- **Confirmed Architectural Layout**: Multi-crate Cargo workspace (`app`, `core`, `player`, `ytdlp`, `auth`, `api`, `cache`) separating UI rendering from async worker tasks and video decoding.
- **Video Decoding Strategy**: `libmpv` render API with DirectX/GPU hardware acceleration (`hwdec=auto-safe`) with fallback to software decoding.
- **Authentication & Security**: Google OAuth2 Authorization Code flow with PKCE on loopback `127.0.0.1`. Refresh tokens encrypted in Windows Credential Manager using the `keyring` crate. Browser cookies extracted via `--cookies-from-browser` (Chrome, Edge, Firefox) without saving plain session tokens.
- **Local Persistence & Privacy**: Standalone local SQLite/JSON storage for Watch Later and Watch History (no fake sync of private non-API YouTube features), official YouTube Data API v3 for Subscriptions, Playlists, and Likes.

---

## 1. Overview & Core Concept

- **What It Does**: A standalone Windows desktop application that streams YouTube videos without the RAM and CPU weight of Chrome/Electron. Provides instant search, subscriptions feed, playlist management, quality selection up to 4K, hardware-accelerated playback, and low-latency keyboard shortcuts.
- **Target Audience / Environment**: Personal power-user workstation on Windows 11 x86_64 with discrete/integrated GPU switching, targeting <150MB idle RAM.
- **Key Value**: 5x to 10x lighter memory footprint than browser tabs, distraction-free native dark interface, robust playback without ads or tracking scripts, and offline-first cache for media metadata.

---

## 2. User Experience & Visual Design

### Key User Flows
1. **Cold Start & Home**: Application launches instantaneously (<400ms). Home view presents a clean responsive grid of recent uploads from subscribed channels (when authenticated) or trending/recommended videos via cached feed.
2. **Instant Search**: User presses `/` or types in the search bar. `yt-dlp` streaming search query (`ytsearchN:`) paginates results with thumbnail caching, displaying title, duration, channel, and view count.
3. **Hardware Playback**: Clicking a video transitions to the playback view with embedded `libmpv`. Keyboard shortcuts (`Space`, `J/K/L`, `F`, `M`, arrow keys) control scrubbing, volume, speed (0.25x - 2.0x), subtitles, and resolution selector (360p to 4K).
4. **Google Account & Sync**: Settings modal offers a one-click "Connect Google Account" opening default browser with PKCE loopback listener. Once authorized, subscriptions and personal playlists populate instantly.
5. **Local History & Watch Later**: Quick-add videos to private local queues independent of external account limits.

### Visual Identity & Theme
- **Theme**: Deep obsidian dark mode (`#0F0F12` background, `#1A1A22` elevated cards, `#262635` borders, `#FF0033` YouTube crimson accent, `#E0E0E8` crisp typography).
- **Typography**: Clean native Windows Segoe UI Variable / Inter typography with strict hierarchy.
- **Layout**: Compact collapsable sidebar navigation (Home, Subscriptions, Playlists, History, Watch Later, Settings) paired with an adaptive responsive grid.

---

## 3. Key Product Decisions & Trade-Offs

- **Slint UI over Tauri/WebView2/Electron**:
  - *Chosen Approach*: Slint native Rust compiled UI.
  - *Why*: Direct native compilation without embedded Chromium, compiling directly to native window handles with minimal RAM (<30MB baseline).
- **Embedded libmpv Render API vs External Window**:
  - *Chosen Approach*: Embedded `libmpv` rendering into Slint canvas/window handle.
  - *Why*: Seamless overlay controls, custom HUD, fullscreen transitions without window tearing or OS border artifacts.
- **yt-dlp Tokio Subprocess Execution**:
  - *Chosen Approach*: Asynchronous child processes communicating over pipes with JSON streaming.
  - *Why*: Isolates network and stream extraction from UI thread, automatically handles YouTube cipher updates, and avoids C binding vulnerabilities.
- **Local vs Remote State**:
  - *Chosen Approach*: Local SQLite/bincode disk cache for watch history and thumbnail bitmaps.
  - *Why*: Zero cloud dependencies, zero latency, functional offline access for metadata.

---

## 4. Technical Architecture & Data Strategy

```
┌────────────────────────────────────────────────────────────────────────┐
│                        RustTube Application                            │
├────────────────────────────────────────────────────────────────────────┤
│                                                                        │
│  ┌───────────────────────────────────┐    ┌─────────────────────────┐  │
│  │           Slint UI (app)          │    │      libmpv (player)    │  │
│  │  - Responsive Grid & Nav Sidebar  │◄──►│  - hwdec=auto-safe      │  │
│  │  - Video HUD & Shortcuts (J/K/L)  │    │  - Render API / Surface │  │
│  └─────────────────▲─────────────────┘    └────────────▲────────────┘  │
│                    │ mpsc Channels (Events & Commands) │               │
│  ┌─────────────────▼───────────────────────────────────▼────────────┐  │
│  │                      RustTube Core Runtime                       │  │
│  │  - Event Loop & State Orchestrator (tokio::runtime)              │  │
│  └───────▲──────────────▲──────────────────▲────────────────▲───────┘  │
│          │              │                  │                │          │
│  ┌───────▼──────┐┌──────▼────────┐┌────────▼───────┐┌───────▼───────┐  │
│  │ ytdlp Engine ││  Google Auth  ││  YouTube API   ││ Cache & Store │  │
│  │ - Search     ││ - PKCE Code   ││ - Subscriptions││ - Thumbnails  │  │
│  │ - Extraction ││ - Loopback    ││ - Playlists    ││ - History     │  │
│  │ - Cookies    ││ - Keyring Enc ││ - Likes        ││ - Watch Later │  │
│  └──────────────┘└───────────────┘└────────────────┘└───────────────┘  │
└────────────────────────────────────────────────────────────────────────┘
```

### Modular Workspace Crates
1. `crates/app`: Slint UI definition (`.slint`), event pump, window management, and input bindings.
2. `crates/core`: Application state, action dispatchers, event contracts, and async orchestration.
3. `crates/player`: `libmpv` initialization, render API context, hardware acceleration flags (`hwdec=auto-safe`), audio/video event handling.
4. `crates/ytdlp`: Process executor for `yt-dlp.exe`, JSON metadata parsing, cookie passing (`--cookies-from-browser`).
5. `crates/auth`: PKCE OAuth2 flow, loopback HTTP server (`127.0.0.1`), token refresh loop, Windows Credential Manager storage via `keyring`.
6. `crates/api`: YouTube Data API v3 client for subscriptions, playlists, and ratings.
7. `crates/cache`: Disk & memory LRU thumbnail cache, local watch history, and watch later stores.
8. Supporting files: `setup.ps1`, `config.example.toml`, `README.md`, `THIRD_PARTY.md`, `.gitignore`, and interactive AI Studio web companion.
