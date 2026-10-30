# RustTube Windows 11 Setup Script
$ErrorActionPreference = "Stop"

$BinDir = Join-Path $PSScriptRoot "bin"
if (-not (Test-Path $BinDir)) {
    New-Item -ItemType Directory -Path $BinDir | Out-Null
}

Write-Host "Downloading latest yt-dlp.exe..." -ForegroundColor Cyan
$YtDlpUrl = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
Invoke-WebRequest -Uri $YtDlpUrl -OutFile (Join-Path $BinDir "yt-dlp.exe")

Write-Host "Ensuring user data directories exist in %APPDATA%\RustTube and %LOCALAPPDATA%\RustTube..." -ForegroundColor Cyan
$AppDataDir = Join-Path $env:APPDATA "RustTube"
$LocalAppDataDir = Join-Path $env:LOCALAPPDATA "RustTube\data"
if (-not (Test-Path $AppDataDir)) { New-Item -ItemType Directory -Path $AppDataDir -Force | Out-Null }
if (-not (Test-Path $LocalAppDataDir)) { New-Item -ItemType Directory -Path $LocalAppDataDir -Force | Out-Null }

$ConfigFile = Join-Path $AppDataDir "config.toml"
if (-not (Test-Path $ConfigFile)) {
    Copy-Item (Join-Path $PSScriptRoot "config.example.toml") $ConfigFile
    Write-Host "Created default config at $ConfigFile" -ForegroundColor Green
}

Write-Host "Setup complete. Ensure libmpv-2.dll is placed in ./bin/ before running cargo run --release." -ForegroundColor Green
