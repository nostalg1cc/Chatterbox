# Builds the native voice engine and places it next to Hero's executable.
# Its WebRTC library needs the static C runtime and short paths, so it builds separately.
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
$flags = $env:RUSTFLAGS
try {
    $env:RUSTFLAGS = '-C target-feature=+crt-static'
    $target = Join-Path $env:TEMP 'nh'
    cargo build --bin nitro-voice-engine --manifest-path (Join-Path $root 'tools\voice-engine\Cargo.toml') --target-dir $target
    if ($LASTEXITCODE -ne 0) { throw 'The voice engine failed to build.' }
    Copy-Item (Join-Path $target 'debug\nitro-voice-engine.exe') (Join-Path $root 'target\debug\') -Force
} finally {
    $env:RUSTFLAGS = $flags
}
