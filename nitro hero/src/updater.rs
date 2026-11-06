//! Over-the-air updates, trusting the same manifest and signing key as the Tauri app.
//! Nothing runs unless its signature checks out against the key built in here.
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
};

const MANIFEST: &str = "https://github.com/nostalg1cc/Chatterbox/releases/latest/download/latest.json";
/// `plugins.updater.pubkey` from `src-tauri/tauri.conf.json`. It must not change, or the
/// next update can't be verified.
const PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEQwN0U4RDQ4OUU0ODAzRTgKUldUb0EwaWVTSTErMEtuRUhsUGhrazVDMFhCMnE4Q2Q5MjlhZGFSS1oxTUNCeWxPRlAxa01XNzMK";
const MAX_INSTALLER_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub struct Update {
    pub version: String,
    pub url: String,
    pub signature: String,
}

fn version_parts(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.trim_start_matches('v').split('.');
    let number = |part: Option<&str>| part?.split('-').next()?.parse().ok();
    Some((number(parts.next())?, number(parts.next())?, number(parts.next())?))
}

fn newer(candidate: &str, current: &str) -> bool {
    matches!((version_parts(candidate), version_parts(current)), (Some(a), Some(b)) if a > b)
}

fn from_manifest(manifest: &Value, current: &str) -> Option<Update> {
    let version = manifest["version"].as_str()?;
    if !newer(version, current) {
        return None;
    }
    let platform = &manifest["platforms"]["windows-x86_64"];
    let url = platform["url"].as_str()?;
    // Only ever over HTTPS, from the project's own releases.
    if !url.starts_with("https://github.com/nostalg1cc/Chatterbox/releases/download/") {
        return None;
    }
    Some(Update {
        version: version.to_string(),
        url: url.to_string(),
        signature: platform["signature"].as_str()?.to_string(),
    })
}

/// `Ok(None)` when already up to date.
pub fn check() -> Result<Option<Update>, String> {
    let response = ureq::get(MANIFEST)
        .call()
        .map_err(|_| "Couldn't check for updates.".to_string())?;
    let manifest: Value = serde_json::from_reader(response.into_reader())
        .map_err(|_| "The update information was invalid.".to_string())?;
    Ok(from_manifest(&manifest, env!("CARGO_PKG_VERSION")))
}

fn decode(text: &str) -> Result<String, String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(text.trim())
        .map_err(|_| "Malformed signature data.".to_string())?;
    String::from_utf8(bytes).map_err(|_| "Malformed signature data.".to_string())
}

pub fn verify(installer: &[u8], signature: &str, public_key: &str) -> Result<(), String> {
    let key = minisign_verify::PublicKey::decode(&decode(public_key)?)
        .map_err(|_| "The update key is invalid.".to_string())?;
    let signature = minisign_verify::Signature::decode(&decode(signature)?)
        .map_err(|_| "The update signature is invalid.".to_string())?;
    key.verify(installer, &signature, true)
        .map_err(|_| "The update failed its signature check, so it wasn't installed.".to_string())
}

/// Downloads and verifies the installer. `progress` counts thousandths of the download.
pub fn download(update: &Update, progress: Arc<AtomicU32>) -> Result<PathBuf, String> {
    let response = ureq::get(&update.url)
        .call()
        .map_err(|_| "The update couldn't be downloaded.".to_string())?;
    let total: u64 = response.header("Content-Length").and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut reader = std::io::Read::take(response.into_reader(), MAX_INSTALLER_BYTES + 1);
    let mut bytes = Vec::with_capacity(total.min(MAX_INSTALLER_BYTES) as usize);
    let mut chunk = vec![0; 256 * 1024];
    loop {
        let count = std::io::Read::read(&mut reader, &mut chunk)
            .map_err(|_| "The update couldn't be downloaded.".to_string())?;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..count]);
        if total > 0 {
            progress.store((bytes.len() as u64 * 1000 / total).min(1000) as u32, Ordering::Relaxed);
        }
    }
    if bytes.len() as u64 > MAX_INSTALLER_BYTES {
        return Err("The update was larger than expected.".into());
    }
    // Before anything is written to disk as an executable.
    verify(&bytes, &update.signature, PUBLIC_KEY)?;
    let path = std::env::temp_dir().join(format!("Nitro_{}_x64-setup.exe", update.version));
    std::fs::write(&path, bytes).map_err(|_| "The update couldn't be saved.".to_string())?;
    Ok(path)
}

/// Starts the installer the way the Tauri updater does. The app must quit right after.
pub fn install(installer: &std::path::Path) -> Result<(), String> {
    std::process::Command::new(installer)
        .args(["/S", "/R", "/UPDATE"])
        .spawn()
        .map(|_| ())
        .map_err(|_| "The update couldn't start.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn manifest(version: &str, url: &str) -> Value {
        json!({"version":version,"platforms":{"windows-x86_64":{"url":url,"signature":"sig"}}})
    }
    const GOOD: &str = "https://github.com/nostalg1cc/Chatterbox/releases/download/v9/setup.exe";

    #[test]
    fn only_newer_versions_are_offered() {
        assert!(newer("0.2.0", "0.1.72"));
        assert!(newer("v0.10.0", "0.9.9"));
        assert!(!newer("0.1.72", "0.2.0"));
        assert!(!newer("0.2.0", "0.2.0"));
        assert!(!newer("rubbish", "0.2.0"));
        assert!(from_manifest(&manifest("0.1.72", GOOD), "0.2.0").is_none());
        assert_eq!(from_manifest(&manifest("0.3.0", GOOD), "0.2.0").unwrap().version, "0.3.0");
    }

    #[test]
    fn downloads_from_elsewhere_are_refused() {
        assert!(from_manifest(&manifest("9.0.0", "https://example.com/setup.exe"), "0.2.0").is_none());
        assert!(from_manifest(&manifest("9.0.0", "http://github.com/nostalg1cc/Chatterbox/releases/download/x"), "0.2.0").is_none());
    }

    /// The real signature of the 0.1.72 manifest parses with the real key, so Hero's checks
    /// understand what the existing signing process produces.
    #[test]
    fn existing_signatures_parse_and_tampering_fails() {
        let latest: Value = serde_json::from_str(include_str!("../../updates/latest.json")).unwrap();
        let signature = latest["platforms"]["windows-x86_64"]["signature"].as_str().unwrap();
        let error = verify(b"not the installer", signature, PUBLIC_KEY).unwrap_err();
        assert!(error.contains("signature check"), "{error}");
    }

    #[test]
    fn built_in_key_matches_the_installed_apps_key() {
        let config: Value = serde_json::from_str(include_str!("../../src-tauri/tauri.conf.json")).unwrap();
        assert_eq!(config["plugins"]["updater"]["pubkey"].as_str(), Some(PUBLIC_KEY));
    }
}
