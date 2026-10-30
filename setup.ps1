# Setup script for RustTube on Windows 11
$ErrorActionPreference = "Stop"

Write-Host "=== RustTube Dependency Setup ===" -ForegroundColor Cyan

$BinDir = Join-Path $PSScriptRoot "bin"
if (-not (Test-Path $BinDir)) {
    New-Item -ItemType Directory -Path $BinDir | Out-Null
}

# 1. Download official yt-dlp.exe
$YtDlpPath = Join-Path $BinDir "yt-dlp.exe"
if (-not (Test-Path $YtDlpPath)) {
    Write-Host "Downloading yt-dlp.exe from official GitHub releases..." -ForegroundColor Yellow
    $YtDlpUrl = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
    Invoke-WebRequest -Uri $YtDlpUrl -OutFile $YtDlpPath
    Write-Host "yt-dlp.exe saved to $YtDlpPath" -ForegroundColor Green
} else {
    Write-Host "yt-dlp.exe already present." -ForegroundColor Green
}

# 2. Check or download libmpv
$MpvDllPath = Join-Path $BinDir "libmpv-2.dll"
$MpvLibPath = Join-Path $BinDir "libmpv.lib"
if ((-not (Test-Path $MpvDllPath)) -or (-not (Test-Path $MpvLibPath))) {
    Write-Host "Downloading libmpv Windows build..." -ForegroundColor Yellow
    $ZipPath = Join-Path $BinDir "mpv-dev.7z"
    $MpvUrl = "https://sourceforge.net/projects/mpv-player-windows/files/libmpv/mpv-dev-x86_64-v3-latest.7z/download"
    try {
        Invoke-WebRequest -Uri $MpvUrl -OutFile $ZipPath
        if (Get-Command 7z -ErrorAction SilentlyContinue) {
            7z x $ZipPath -o"$BinDir" -y | Out-Null
            Remove-Item $ZipPath -Force
            Write-Host "Extracted libmpv successfully." -ForegroundColor Green
        } else {
            Write-Host "Please extract $ZipPath into $BinDir (requires 7-Zip). Ensure libmpv-2.dll and mpv.lib are in $BinDir" -ForegroundColor Yellow
        }
    } catch {
        Write-Host "Note: Obtain libmpv-2.dll and mpv.lib from https://sourceforge.net/projects/mpv-player-windows/files/libmpv/ and place in bin/" -ForegroundColor Yellow
    }
} else {
    Write-Host "libmpv-2.dll and mpv.lib already present." -ForegroundColor Green
}

# 3. Initialize config.toml if missing
$ConfigFile = Join-Path $PSScriptRoot "config.toml"
$ConfigExample = Join-Path $PSScriptRoot "config.example.toml"
if (-not (Test-Path $ConfigFile)) {
    Copy-Item $ConfigExample $ConfigFile
    Write-Host "Created config.toml from config.example.toml. Configure your Google OAuth client ID." -ForegroundColor Cyan
}

Write-Host "Setup completed! Build and run with:" -ForegroundColor Green
Write-Host "  cargo run --release" -ForegroundColor White
