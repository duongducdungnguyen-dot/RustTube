# RustTube — Native Windows YouTube Client Migration Plan

A native Windows 11 desktop YouTube application engineered in pure Rust using **egui + eframe** for a lightweight immediate-mode GUI (<60MB idle RAM), embedded **libmpv** with DirectX/GPU hardware acceleration, **yt-dlp** for stream extraction, **Google OAuth 2.0 PKCE** loopback flow with Windows Credential Manager (`keyring`), and **SQLite** (`rusqlite`) for offline-first local state and YouTube account synchronization.

---

## 1. Existing Codebase Audit & Preservation Matrix

| Existing Module | Current Status | Action | Rationale |
|---|---|---|---|
| `crates/auth` | Working PKCE loopback server, keyring store, token refresh | **Preserve & Enhance** | Robust OAuth2 PKCE implementation with `prompt="select_account consent"` and secure Windows Credential Manager integration. |
| `crates/api` | YouTube Data API client (channel, subscriptions, playlists, likes, rate) | **Preserve & Expand** | Add playlist creation/update, add/delete playlist items, subscribe/unsubscribe, and pagination sync workers. |
| `crates/ytdlp` | Asynchronous tokio child process for `ytsearch`, dump-json, stream URL | **Preserve** | Clean subprocess architecture isolating YouTube cipher handling and network extraction from the GUI thread. |
| `crates/player` | Dynamic `libmpv` loading (`MpvLibrary`, `MpvInstance`), HWDEC, wid binding | **Preserve & Integrate** | True native C playback engine without webview overhead. |
| `crates/cache` | In-memory + flat JSON files for history and watch later | **Migrate to SQLite** | Replace flat JSON with indexed SQLite (`rusqlite`) for atomic transactions, sync tracking, and fast queries. |
| `crates/core` | Models (`VideoItem`, `ChannelItem`, etc.), `AppConfig`, `YouTubeUrlParser` | **Preserve & Expand** | Retain shared data contracts; add SQLite serialization and sync metadata. |
| `crates/app` (Slint UI) | Slint UI DSL & callbacks | **Migrate to egui** | Switch to `egui + eframe` for complete zero-dependency immediate-mode GUI, custom theme, embedded canvas, and low memory. |
| Web companion (`server.ts`, `App.tsx`) | Vite/Express development mock/preview | **Decouple** | The native Rust executable is 100% standalone and requires zero Node.js or Express runtime. |

---

## 2. Target Architecture & Crate Structure

```
Cargo.toml (Workspace root)
├── crates/core/
│   ├── models.rs            # VideoItem, ChannelItem, PlaylistItem, PlaybackStatus, StreamInfo
│   ├── config.rs            # AppConfig (config.toml loader & defaults)
│   ├── events.rs            # Async UiCommand & AppEvent message passing
│   └── url_parser.rs        # YouTube URL and ID parser
│
├── crates/storage/          # SQLite Local Storage (rusqlite)
│   ├── database.rs          # Connection pool, schema initialization, PRAGMA optimizations
│   ├── migrations.rs        # Schema versioning (history, watch_later, subscriptions, playlists, sync_meta)
│   ├── history_repo.rs      # Local Watch History queries & cleanup
│   ├── watch_later_repo.rs  # Local Watch Later queue
│   └── sync_repo.rs         # Cached subscriptions, playlists, liked videos with last_sync timestamp
│
├── crates/auth/             # Google OAuth2 PKCE & Windows Credential Manager
│   ├── pkce.rs              # Loopback HTTP server (127.0.0.1:port), PKCE S256, token exchange
│   ├── keyring_store.rs     # Windows Credential Manager token storage via keyring crate
│   └── session.rs           # Session restoration, token refresh loop, logout revocation
│
├── crates/api/              # YouTube Data API v3 Client
│   ├── client.rs            # reqwest client with rate limiting, timeouts, and error handling
│   ├── channel.rs           # My channel and profile metadata
│   ├── subscriptions.rs     # Subscriptions retrieval with nextPageToken pagination
│   ├── playlists.rs         # Playlist list, playlist items, create playlist, add/remove item
│   └── ratings.rs           # Liked videos list & rate video endpoint
│
├── crates/ytdlp/            # yt-dlp Subprocess Runner
│   ├── process.rs           # yt-dlp process execution, JSON streaming, browser cookie injection
│   └── stream.rs            # Audio/video URL resolution, formats, and quality selector
│
├── crates/player/           # libmpv Playback Engine
│   ├── mpv.rs               # libmpv FFI bindings and dynamic library loader
│   └── controller.rs        # Async player state, seek, volume, speed, mute, status poll
│
└── crates/app/              # Native egui + eframe Desktop Application
    ├── main.rs              # Application entry point, tokio runtime initialization, eframe run
    ├── app.rs               # eframe::App state, update() loop, event pump
    ├── theme.rs             # YouTube Obsidian Dark theme styling, colors, font configuration
    ├── ui/
    │   ├── sidebar.rs       # Navigation sidebar (Home, Search, Subscriptions, Playlists, Likes, Watch Later, History, Settings)
    │   ├── header.rs        # Top search bar with URL detection, account pill, and sync status
    │   ├── video_grid.rs    # Responsive, virtualized video card grid with thumbnail caching
    │   ├── watch_view.rs    # Dedicated video playback page, player controls, description, up next
    │   ├── subscriptions.rs # Subscribed channels list and channel feeds
    │   ├── playlists_view.rs# User playlists view, video lists, and playlist creation dialog
    │   ├── history_view.rs  # Local watch history management
    │   ├── watch_later_view.rs # Local watch later queue
    │   └── settings_view.rs # Google account connection, browser cookies, cache clearing, about
    └── cache/
        └── thumbnail_loader.rs # Asynchronous egui texture manager with memory & disk cache
```

---

## 3. Database Schema (SQLite via `rusqlite`)

The application uses a local SQLite database at `cache/rusttube.db` with WAL mode (`PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;`):

1. **`watch_history`**:
   - `video_id` TEXT PRIMARY KEY, `title` TEXT NOT NULL, `channel_name` TEXT, `channel_id` TEXT, `duration_seconds` INTEGER, `duration_string` TEXT, `view_count` INTEGER, `thumbnail_url` TEXT, `watched_at` INTEGER NOT NULL.
2. **`watch_later`**:
   - `video_id` TEXT PRIMARY KEY, `title` TEXT NOT NULL, `channel_name` TEXT, `channel_id` TEXT, `duration_seconds` INTEGER, `duration_string` TEXT, `view_count` INTEGER, `thumbnail_url` TEXT, `added_at` INTEGER NOT NULL.
3. **`cached_subscriptions`**:
   - `channel_id` TEXT PRIMARY KEY, `title` TEXT NOT NULL, `description` TEXT, `thumbnail_url` TEXT, `synced_at` INTEGER NOT NULL.
4. **`cached_playlists`**:
   - `playlist_id` TEXT PRIMARY KEY, `title` TEXT NOT NULL, `description` TEXT, `item_count` INTEGER, `thumbnail_url` TEXT, `synced_at` INTEGER NOT NULL.
5. **`cached_playlist_items`**:
   - `id` TEXT PRIMARY KEY, `playlist_id` TEXT NOT NULL, `video_id` TEXT NOT NULL, `title` TEXT, `channel_name` TEXT, `thumbnail_url` TEXT, `position` INTEGER, FOREIGN KEY(playlist_id) REFERENCES cached_playlists(playlist_id) ON DELETE CASCADE.
6. **`cached_liked_videos`**:
   - `video_id` TEXT PRIMARY KEY, `title` TEXT NOT NULL, `channel_name` TEXT, `channel_id` TEXT, `thumbnail_url` TEXT, `view_count` INTEGER, `synced_at` INTEGER NOT NULL.
7. **`sync_metadata`**:
   - `key` TEXT PRIMARY KEY, `last_synced_at` INTEGER NOT NULL, `etag` TEXT.

---

## 4. Multi-Stage Implementation Plan

### Stage 1: Workspace Dependencies & SQLite Crate Setup
- Update root `Cargo.toml` with `eframe`, `egui`, `egui_extras`, `rusqlite`, and image codecs.
- Create `crates/storage` with `rusqlite` database manager, migrations, and repository implementations (`history`, `watch_later`, `sync`).

### Stage 2: YouTube API Expansion & Sync Engine
- Expand `crates/api` to implement full playlist management (`create_playlist`, `insert_playlist_item`, `delete_playlist_item`).
- Implement channel subscription toggle (`subscribe_channel`, `unsubscribe_channel`).
- Build asynchronous synchronization worker in `crates/core` / `crates/api` that runs non-blocking tasks with cancellation tokens.

### Stage 3: egui + eframe Application Shell & Design System
- Create `crates/app` with `eframe::App` implementation.
- Design YouTube Obsidian dark theme (palette `#0E0F14`, `#15161C`, `#1F212D`, YouTube Crimson `#FF0033`).
- Implement navigation sidebar (Home, Search, Subscriptions, Playlists, Liked Videos, Watch Later, History, Settings).

### Stage 4: Video Grid, Search & Async Thumbnail Loading
- Build responsive card grid with title, channel, view count, duration badge, and quick Watch Later toggle.
- Implement asynchronous image loading into egui textures with memory LRU and disk caching.
- Add instant search bar with YouTube URL detection (`youtu.be`, `youtube.com/watch`, `/shorts/`, `/playlist`).

### Stage 5: Playback Subsystem Integration
- Connect `PlayerController` (`libmpv` + `yt-dlp`) with egui.
- Build playback HUD: scrubber slider with time display, play/pause, volume slider, playback speed (0.5x - 2.0x), quality selector (360p - 1080p/4K), and fullscreen mode.

### Stage 6: Google Account & Account Synchronization UI
- Connect Google OAuth2 PKCE login/logout buttons in Settings and Header.
- Display authenticated user profile (name, email, avatar).
- Render synchronized Subscriptions, user Playlists (with playlist video inspector), and Liked Videos from SQLite cache.
- Provide manual "Sync Now" button with progress indicator and last sync timestamp.

### Stage 7: Local Storage UI & Data Integrity
- Implement local Watch History page with clear history action and item removal.
- Implement local Watch Later queue with reordering and removal.
- Verify that browsing Home or switching tabs never adds unplayed videos to history.

### Stage 8: Build Verification, Compilation & Standalone Executable
- Verify workspace builds cleanly with `cargo check` and `cargo build --release`.
- Confirm zero dependency on Node.js / Express for the native Windows executable.
