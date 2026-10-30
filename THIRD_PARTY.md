# Third-Party Dependencies and Licenses

RustTube links against and uses the following open-source libraries and utilities:

### Direct Crates
- **slint** (GPLv3 / Royalty-Free Desktop / Commercial): Modern GUI toolkit compiled directly to native graphics.
- **tokio** (MIT): Asynchronous runtime for I/O, subprocesses, and background task scheduling.
- **serde / serde_json** (MIT / Apache-2.0): High-speed JSON serialization and deserialization.
- **reqwest** (MIT / Apache-2.0): Async HTTP client with rustls TLS engine.
- **oauth2** (MIT / Apache-2.0): Standards-compliant OAuth 2.0 PKCE client.
- **keyring** (MIT / Apache-2.0): Secure credential storage integrating with Windows Credential Manager.
- **image** (MIT / Apache-2.0): Image decoding engine for thumbnail rasterization.
- **windows-sys** (MIT / Apache-2.0): Direct raw Win32 bindings for window handle and render surface integration.
- **libloading** (ISC): Dynamic library symbol loader for libmpv.

### External Binaries & Runtime Libraries
- **yt-dlp** (The Unlicense): Command-line audio/video extraction tool.
- **libmpv** (LGPLv2.1+ / GPLv2+): Media player core library providing hardware-accelerated video decoding via DirectX / Direct3D / OpenGL.
