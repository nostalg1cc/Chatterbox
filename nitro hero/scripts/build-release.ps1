# Builds the signed-update candidate: release app, release voice engine, NSIS installer.
# Publishes nothing. Signing takes the key from the environment, as the Tauri release does:
# TAURI_SIGNING_PRIVATE_KEY and TAURI_SIGNING_PRIVATE_KEY_PASSWORD. Without them, it stops
# after building the unsigned installer, which the updater would refuse.
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
$repo = Split-Path $root
$version = (Select-String -Path (Join-Path $root 'Cargo.toml') -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$out = Join-Path $root 'release\out'
$payload = Join-Path $out 'payload'
Remove-Item $out -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $payload | Out-Null

cargo build --release --locked --manifest-path (Join-Path $root 'Cargo.toml')
if ($LASTEXITCODE -ne 0) { throw 'The app failed to build.' }
$flags = $env:RUSTFLAGS
try {
    $env:RUSTFLAGS = '-C target-feature=+crt-static'
    $target = Join-Path $env:TEMP 'nh'
    cargo build --release --bin nitro-voice-engine --manifest-path (Join-Path $root 'tools\voice-engine\Cargo.toml') --target-dir $target
    if ($LASTEXITCODE -ne 0) { throw 'The voice engine failed to build.' }
} finally {
    $env:RUSTFLAGS = $flags
}
Copy-Item (Join-Path $root 'target\release\nitro-hero.exe') (Join-Path $payload 'nitro.exe')
Copy-Item (Join-Path $target 'release\nitro-voice-engine.exe') $payload
# Video compression needs it. Its build is described in tools\ffmpeg.
$ffmpeg = Join-Path $root 'tools\ffmpeg\out\ffmpeg.exe'
if (-not (Test-Path $ffmpeg)) { throw 'tools\ffmpeg\out\ffmpeg.exe is missing. See tools\ffmpeg\build-ffmpeg.sh.' }
Copy-Item $ffmpeg (Join-Path $payload 'nitro-ffmpeg.exe')
Copy-Item (Join-Path $root 'tools\ffmpeg\FFMPEG-NOTICE.txt') $payload

$makensis = Join-Path $env:LOCALAPPDATA 'tauri\NSIS\makensis.exe'
if (-not (Test-Path $makensis)) { $makensis = Join-Path $env:LOCALAPPDATA 'tauri\NSIS\Bin\makensis.exe' }
if (-not (Test-Path $makensis)) { throw 'makensis not found. A Tauri build installs it to %LOCALAPPDATA%\tauri\NSIS.' }
$installer = Join-Path $out "Nitro_${version}_x64-setup.exe"
& $makensis /V2 "/DVERSION=$version" "/DOUTPUT=$installer" "/DPAYLOAD=$payload" "/DICON=$(Join-Path $root 'assets\nitro.ico')" (Join-Path $root 'release\nitro.nsi')
if ($LASTEXITCODE -ne 0) { throw 'The installer failed to build.' }
Write-Output "Built $installer"

# Without the password as well, the signer waits for someone to type it, forever.
if (-not $env:TAURI_SIGNING_PRIVATE_KEY -or -not $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD) {
    Write-Output 'Not signed: TAURI_SIGNING_PRIVATE_KEY or its password is not set.'
    return
}
Push-Location $repo
try {
    npx tauri signer sign $installer
    if ($LASTEXITCODE -ne 0) { throw 'Signing failed.' }
} finally {
    Pop-Location
}
Write-Output "Signed $installer.sig"
