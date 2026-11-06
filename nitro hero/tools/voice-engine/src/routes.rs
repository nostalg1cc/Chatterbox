//! Strict device routing: the SDK's GUID setter silently falls back to index 0.
//! Validate against current enumeration before selecting by index instead.
use libwebrtc::peer_connection_factory::{PeerConnectionFactory, native::PeerConnectionFactoryExt};

pub enum Route<'a> {
    SystemDefault,
    DeviceGuid(&'a str),
}
pub fn resolve(route: Route<'_>, guids: &[String]) -> Result<u16, &'static str> {
    match route {
        Route::SystemDefault if !guids.is_empty() => Ok(0),
        Route::SystemDefault => Err("No audio devices available"),
        Route::DeviceGuid(guid) => {
            if guid.is_empty() {
                return Err("Empty device identifier is not a default route");
            }
            let mut matches = guids
                .iter()
                .enumerate()
                .filter(|(_, id)| id.as_str() == guid);
            let index = matches
                .next()
                .ok_or("Selected device unavailable; choose Default explicitly")?
                .0;
            if matches.next().is_some() {
                return Err("Ambiguous device identifier");
            }
            u16::try_from(index).map_err(|_| "Device index unsupported")
        }
    }
}
pub fn input(factory: &PeerConnectionFactory, route: Route<'_>) -> Result<(), &'static str> {
    let guids = (0..factory.recording_devices().max(0) as u16)
        .map(|index| factory.recording_device_guid(index))
        .collect::<Vec<_>>();
    let index = resolve(route, &guids)?;
    if factory.set_recording_device(index) {
        Ok(())
    } else {
        Err("Microphone route selection failed")
    }
}
pub fn output(factory: &PeerConnectionFactory, route: Route<'_>) -> Result<(), &'static str> {
    let guids = (0..factory.playout_devices().max(0) as u16)
        .map(|index| factory.playout_device_guid(index))
        .collect::<Vec<_>>();
    let index = resolve(route, &guids)?;
    if factory.set_playout_device(index) {
        Ok(())
    } else {
        Err("Playback route selection failed")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_guid_survives_reorder_and_never_falls_back_silently() {
        let devices = vec!["default".into(), "mic-b".into(), "mic-a".into()];
        assert_eq!(resolve(Route::DeviceGuid("mic-a"), &devices), Ok(2));
        assert_eq!(
            resolve(
                Route::DeviceGuid("mic-a"),
                &["mic-a".into(), "default".into()]
            ),
            Ok(0)
        );
        assert!(resolve(Route::DeviceGuid("missing"), &devices).is_err());
        assert!(resolve(Route::DeviceGuid(""), &devices).is_err());
        assert!(resolve(Route::DeviceGuid("dup"), &["dup".into(), "dup".into()]).is_err());
        assert_eq!(resolve(Route::SystemDefault, &devices), Ok(0));
        assert!(resolve(Route::SystemDefault, &[]).is_err());
    }
}
