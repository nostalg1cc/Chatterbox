//! Windows endpoint metering: Device tracks don't forward local PCM to track sinks.
//! Reads only endpoint peak values while the ADM owns capture. No additional
//! capture stream, audio recording or playback. Remote tracks keep their PCM meter.
use serde_json::{Value, json};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}, mpsc::SyncSender};
use std::time::Duration;

/// Low noise gate with generous speech headroom; visual response only.
fn display_level(peak: f32) -> f32 {
    if !peak.is_finite() { return 0.; }
    ((peak - 0.005) / (0.14 - 0.005)).clamp(0., 1.).powf(1.25)
}

pub struct LocalMeter {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl LocalMeter {
    pub fn start(endpoint: String, events: SyncSender<Value>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let cancelled = stop.clone();
        let thread = std::thread::spawn(move || run(endpoint, events, cancelled));
        Self { stop, thread: Some(thread) }
    }
}
impl Drop for LocalMeter {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() { let _ = thread.join(); }
    }
}

/// This process's own volume on an output device, which only this call uses. Windows has
/// no way to go past 100%, and the engine's WebRTC library has no per-track volume.
#[cfg(windows)]
pub fn set_output_level(endpoint: &str, level: f32) {
    use windows::{core::PCWSTR, Win32::{Media::Audio::{
        eCommunications, eRender, IAudioSessionManager, IMMDeviceEnumerator, ISimpleAudioVolume,
        MMDeviceEnumerator,
    }, System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED}}};
    let endpoint = endpoint.to_string();
    // Its own thread, for a COM apartment that doesn't touch WebRTC's.
    let _ = std::thread::spawn(move || unsafe {
        if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() {
            return;
        }
        let _ = (|| -> windows::core::Result<()> {
            let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let device = if endpoint.is_empty() {
                enumerator.GetDefaultAudioEndpoint(eRender, eCommunications)?
            } else {
                let id: Vec<u16> = endpoint.encode_utf16().chain(Some(0)).collect();
                enumerator.GetDevice(PCWSTR(id.as_ptr()))?
            };
            let manager: IAudioSessionManager = device.Activate(CLSCTX_ALL, None)?;
            let volume: ISimpleAudioVolume = manager.GetSimpleAudioVolume(None, false.into())?;
            volume.SetMasterVolume(level, std::ptr::null())
        })();
        CoUninitialize();
    });
}

#[cfg(not(windows))]
pub fn set_output_level(_: &str, _: f32) {}

#[cfg(windows)]
fn run(endpoint: String, events: SyncSender<Value>, stop: Arc<AtomicBool>) {
    use windows::{core::PCWSTR, Win32::{Media::Audio::{
        IMMDeviceEnumerator, MMDeviceEnumerator, Endpoints::IAudioMeterInformation,
    }, System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED}}};
    unsafe {
        if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() {
            let _ = events.try_send(json!({"event":"local-meter","status":"com-unavailable"}));
            return;
        }
        let result = (|| -> windows::core::Result<()> {
            let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let id: Vec<u16> = endpoint.encode_utf16().chain(Some(0)).collect();
            let device = enumerator.GetDevice(PCWSTR(id.as_ptr()))?;
            let meter: IAudioMeterInformation = device.Activate(CLSCTX_ALL, None)?;
            let _ = events.try_send(json!({"event":"local-meter","status":"ready"}));
            while !stop.load(Ordering::Acquire) {
                let peak = meter.GetPeakValue()?;
                let level = display_level(peak);
                let _ = events.try_send(json!({"event":"level","side":"local","epoch":"","level":level,"raw_peak":peak}));
                std::thread::sleep(Duration::from_millis(50));
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = events.try_send(json!({"event":"local-meter","status":"endpoint-unavailable"}));
        }
        // Interfaces were dropped before leaving their COM apartment.
        CoUninitialize();
    }
}

#[cfg(not(windows))]
fn run(_: String, events: SyncSender<Value>, _: Arc<AtomicBool>) {
    let _ = events.try_send(json!({"event":"local-meter","status":"unsupported"}));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn silence_quiet_speech_and_loud_speech_follow_nitro_response() {
        assert_eq!(display_level(0.), 0.);
        assert_eq!(display_level(0.004), 0.);
        let quiet = display_level(0.01);
        let normal = display_level(0.0253499);
        assert!(quiet > 0. && quiet < normal);
        assert!(normal > 0.08 && normal < 0.12);
        assert!(display_level(0.06) < 0.4);
        assert_eq!(display_level(0.14), 1.);
        assert_eq!(display_level(2.), 1.);
        assert_eq!(display_level(f32::NAN), 0.);
    }
}
