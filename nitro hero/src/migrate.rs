//! One-time carry-over from the Tauri app on first run. Read-only, best effort: the old
//! data is left in place, and anything not found just means defaults or a normal sign-in.
use crate::state::{AppTheme, Preferences};
use serde_json::Value;
use std::path::{Path, PathBuf};

const PREFERENCES_KEY: &[u8] = b"dislight-preferences-v1";
const SESSION_KEY_END: &[u8] = b"-auth-token";

fn storage_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
        .join("io.github.nostalg1cc.dislight/EBWebView/Default/Local Storage/leveldb");
    dir.is_dir().then_some(dir)
}

pub fn has_legacy_storage() -> bool {
    storage_dir().is_some()
}

/// The storage's files, newest first. Newer writes win.
fn storage_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.ends_with(".log") || name.ends_with(".ldb")
        })
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .collect();
    files.sort_by(|a, b| b.0.cmp(&a.0));
    files.into_iter().map(|(_, path)| path).collect()
}

/// The last whole JSON object stored right after this key. Compressed table blocks simply
/// don't match, and an object cut off by a block boundary fails to parse and is skipped.
fn json_after(bytes: &[u8], key: &[u8]) -> Option<Value> {
    let mut end = bytes.len();
    while let Some(found) = bytes[..end].windows(key.len()).rposition(|window| window == key) {
        end = found;
        let after = found + key.len();
        // A length and a latin-1 flag sit between the key and the value.
        let Some(offset) = bytes[after..bytes.len().min(after + 12)].iter().position(|b| *b == b'{') else {
            continue;
        };
        let start = after + offset;
        let mut depth = 0usize;
        let mut in_string = false;
        let mut escaped = false;
        for (index, byte) in bytes[start..].iter().enumerate() {
            if in_string {
                match byte {
                    _ if escaped => escaped = false,
                    b'\\' => escaped = true,
                    b'"' => in_string = false,
                    _ => {}
                }
                continue;
            }
            match byte {
                b'"' => in_string = true,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        if let Ok(value) = serde_json::from_slice(&bytes[start..=start + index]) {
                            return Some(value);
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
    }
    None
}

fn apply(saved: &Value, preferences: &mut Preferences) {
    let number = |name: &str| saved[name].as_u64();
    let flag = |name: &str| saved[name].as_bool();
    if let Some(theme) = saved.get("theme").and_then(|theme| serde_json::from_value::<AppTheme>(theme.clone()).ok()) {
        preferences.theme = theme;
    }
    // Browser device ids mean nothing to the native engine, so the devices stay on default.
    if let Some(value) = number("inputVolume") { preferences.input_volume = value.min(100) as u8; }
    if let Some(value) = number("outputVolume") { preferences.output_volume = value.min(100) as u8; }
    if let Some(value) = number("partnerVoiceBoost") { preferences.partner_volume = value.min(200) as u8; }
    if let Some(value) = number("soundboardVolume") { preferences.soundboard_volume = value.min(100) as u8; }
    if let Some(value) = number("interfaceSoundVolume") { preferences.interface_sound_volume = value.min(100) as u8; }
    if let Some(value) = number("voiceHudScale") { preferences.voice_hud_scale = value.clamp(50, 150) as u8; }
    if let Some(value) = flag("enterToSend") { preferences.enter_to_send = value; }
    if let Some(value) = flag("compactMessages") { preferences.compact_messages = value; }
    if let Some(value) = flag("showMediaPreviews") { preferences.show_media_previews = value; }
    if let Some(value) = flag("interfaceSounds") { preferences.interface_sounds = value; }
    if let Some(value) = flag("voiceHudShowNames") { preferences.voice_hud_show_names = value; }
    if let Some(list) = saved["pinnedSoundIds"].as_array() {
        preferences.pinned_sound_ids =
            list.iter().filter_map(|id| id.as_str().map(String::from)).collect();
    }
}

/// True if settings were found and applied.
pub fn import_preferences(preferences: &mut Preferences) -> bool {
    let Some(dir) = storage_dir() else { return false };
    for file in storage_files(&dir) {
        let Ok(bytes) = std::fs::read(&file) else { continue };
        if let Some(saved) = json_after(&bytes, PREFERENCES_KEY) {
            apply(&saved, preferences);
            return true;
        }
    }
    false
}

/// The old app's refresh token, so updating doesn't sign anyone out. It's only ever
/// exchanged for a session, which is then sealed like any other sign-in.
pub fn legacy_refresh_token() -> Option<String> {
    let dir = storage_dir()?;
    for file in storage_files(&dir) {
        let Ok(bytes) = std::fs::read(&file) else { continue };
        if let Some(saved) = json_after(&bytes, SESSION_KEY_END)
            && let Some(token) = saved["refresh_token"].as_str()
        {
            return Some(token.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checks this machine's real old-app data can be read, printing nothing private.
    #[test]
    #[ignore]
    fn reads_the_installed_apps_data() {
        let mut preferences = Preferences::default();
        println!("settings found: {}", import_preferences(&mut preferences));
        println!("theme: {}", preferences.theme.label());
        println!("sign-in token found: {}", legacy_refresh_token().is_some_and(|token| token.len() > 8));
    }

    #[test]
    fn newest_complete_value_is_found_between_storage_bytes() {
        let mut bytes = b"\x00junk dislight-preferences-v1\xe1\x06\x01{\"theme\":\"old\"}\x00".to_vec();
        bytes.extend_from_slice(b"more dislight-preferences-v1\xe1\x06\x01{\"theme\":\"sky\",\"a\":\"}\"}tail");
        assert_eq!(json_after(&bytes, PREFERENCES_KEY).unwrap()["theme"], "sky");
    }

    #[test]
    fn a_cut_off_value_falls_back_to_an_older_one() {
        let mut bytes = b"dislight-preferences-v1\x01{\"theme\":\"old\"}".to_vec();
        bytes.extend_from_slice(b" dislight-preferences-v1\x01{\"theme\":\"new");
        assert_eq!(json_after(&bytes, PREFERENCES_KEY).unwrap()["theme"], "old");
    }

    #[test]
    fn nothing_is_found_in_unrelated_or_compressed_bytes() {
        assert!(json_after(b"dislight-preferences-v1\x01\xb4\n\x00compressed", PREFERENCES_KEY).is_none());
    }

    #[test]
    fn settings_map_with_limits_and_devices_stay_on_default() {
        let mut preferences = Preferences::default();
        apply(
            &serde_json::json!({"theme":"sky","outputVolume":500,"voiceHudScale":10,"enterToSend":false,
                "inputDeviceId":"browser-id","pinnedSoundIds":["a"]}),
            &mut preferences,
        );
        assert_eq!(preferences.theme, AppTheme::Sky);
        assert_eq!(preferences.output_volume, 100);
        assert_eq!(preferences.voice_hud_scale, 50);
        assert!(!preferences.enter_to_send);
        assert_eq!(preferences.input_device_id, "default");
        assert_eq!(preferences.pinned_sound_ids, ["a"]);
    }
}
