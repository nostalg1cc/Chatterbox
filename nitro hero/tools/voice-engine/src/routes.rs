//! Device routing by endpoint id. The list's order is arbitrary: index 0 is just the first
//! endpoint, not the Windows default, so the default is looked up and matched by id.
use libwebrtc::peer_connection_factory::{PeerConnectionFactory, native::PeerConnectionFactoryExt};

/// The chosen endpoint, or Windows' default when none is chosen or it has been unplugged.
/// The saved choice stays in preferences, so it comes back with the device.
pub fn resolve(wanted: &str, guids: &[String], system_default: Option<&str>) -> Result<u16, &'static str> {
    if guids.is_empty() {
        return Err("No audio devices available");
    }
    let find = |id: &str| guids.iter().position(|guid| guid == id);
    let index = (!wanted.is_empty()).then(|| find(wanted)).flatten()
        .or_else(|| system_default.and_then(find))
        .unwrap_or(0);
    u16::try_from(index).map_err(|_| "Device index unsupported")
}

#[cfg(windows)]
fn system_default(capture: bool) -> Option<String> {
    use windows::Win32::{
        Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator, eCapture, eConsole, eRender},
        System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize},
    };
    // Its own thread, for a COM apartment that doesn't touch WebRTC's.
    std::thread::spawn(move || unsafe {
        if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() {
            return None;
        }
        let id = (|| -> windows::core::Result<Option<String>> {
            let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let device = enumerator.GetDefaultAudioEndpoint(if capture { eCapture } else { eRender }, eConsole)?;
            let id = device.GetId()?;
            let text = id.to_string().ok();
            CoTaskMemFree(Some(id.0 as *const _));
            Ok(text)
        })()
        .ok()
        .flatten();
        CoUninitialize();
        id
    })
    .join()
    .ok()
    .flatten()
}
#[cfg(not(windows))]
fn system_default(_: bool) -> Option<String> {
    None
}

/// Selects the microphone and returns the endpoint actually used.
pub fn input(factory: &PeerConnectionFactory, wanted: &str) -> Result<String, &'static str> {
    let guids = (0..factory.recording_devices().max(0) as u16)
        .map(|index| factory.recording_device_guid(index))
        .collect::<Vec<_>>();
    let index = resolve(wanted, &guids, system_default(true).as_deref())?;
    if factory.set_recording_device(index) {
        Ok(guids[index as usize].clone())
    } else {
        Err("Microphone route selection failed")
    }
}
/// Selects the output and returns the endpoint actually used.
pub fn output(factory: &PeerConnectionFactory, wanted: &str) -> Result<String, &'static str> {
    let guids = (0..factory.playout_devices().max(0) as u16)
        .map(|index| factory.playout_device_guid(index))
        .collect::<Vec<_>>();
    let index = resolve(wanted, &guids, system_default(false).as_deref())?;
    if factory.set_playout_device(index) {
        Ok(guids[index as usize].clone())
    } else {
        Err("Playback route selection failed")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_chosen_device_is_found_wherever_it_sits() {
        let devices = vec!["first".to_string(), "mic-b".into(), "mic-a".into()];
        assert_eq!(resolve("mic-a", &devices, Some("first")), Ok(2));
        assert_eq!(resolve("mic-a", &["mic-a".into(), "first".into()], None), Ok(0));
    }
    #[test]
    fn default_means_the_windows_default_not_the_first_entry() {
        let devices = vec!["first".to_string(), "windows-default".into()];
        assert_eq!(resolve("", &devices, Some("windows-default")), Ok(1));
        assert_eq!(resolve("", &devices, None), Ok(0));
        assert_eq!(resolve("", &devices, Some("gone")), Ok(0));
    }
    #[test]
    fn an_unplugged_device_uses_the_windows_default() {
        let devices = vec!["first".to_string(), "windows-default".into()];
        assert_eq!(resolve("unplugged", &devices, Some("windows-default")), Ok(1));
        assert!(resolve("", &[], None).is_err());
    }
}
