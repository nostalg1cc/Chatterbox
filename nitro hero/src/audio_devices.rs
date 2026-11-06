use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Default)]
pub struct SystemAudioDevices {
    pub inputs: Vec<AudioDevice>,
    pub outputs: Vec<AudioDevice>,
}

#[cfg(windows)]
mod win_audio {
    use super::{AudioDevice, SystemAudioDevices};
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    #[repr(C, packed)]
    struct WAVEINCAPSW {
        w_mid: u16,
        w_pid: u16,
        v_driver_version: u32,
        sz_pname: [u16; 32],
        dw_formats: u32,
        w_channels: u16,
        w_reserved1: u16,
    }

    #[repr(C, packed)]
    struct WAVEOUTCAPSW {
        w_mid: u16,
        w_pid: u16,
        v_driver_version: u32,
        sz_pname: [u16; 32],
        dw_formats: u32,
        w_channels: u16,
        w_reserved1: u16,
        dw_support: u32,
    }

    #[link(name = "winmm")]
    unsafe extern "system" {
        fn waveInGetNumDevs() -> u32;
        fn waveInGetDevCapsW(u_device_id: usize, pwic: *mut WAVEINCAPSW, cbwic: u32) -> u32;
        fn waveOutGetNumDevs() -> u32;
        fn waveOutGetDevCapsW(u_device_id: usize, pwoc: *mut WAVEOUTCAPSW, cbwoc: u32) -> u32;
    }

    type HKEY = isize;
    const HKEY_LOCAL_MACHINE: HKEY = -2147483646isize; // 0x80000002
    const KEY_READ: u32 = 0x20019;
    const REG_SZ: u32 = 1;
    const REG_DWORD: u32 = 4;

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegOpenKeyExW(
            hkey: HKEY,
            lpsubkey: *const u16,
            uloptions: u32,
            samdesired: u32,
            phkresult: *mut HKEY,
        ) -> i32;
        fn RegEnumKeyExW(
            hkey: HKEY,
            dwindex: u32,
            lpname: *mut u16,
            lpcchname: *mut u32,
            lpreserved: *const u32,
            lpclass: *mut u16,
            lpcchclass: *mut u32,
            lpftlastwritetime: *mut u64,
        ) -> i32;
        fn RegQueryValueExW(
            hkey: HKEY,
            lpvaluename: *const u16,
            lpreserved: *const u32,
            lptype: *mut u32,
            lpdata: *mut u8,
            lpcbdata: *mut u32,
        ) -> i32;
        fn RegCloseKey(hkey: HKEY) -> i32;
    }

    fn to_wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    fn from_wide_slice(slice: &[u16]) -> String {
        let len = slice.iter().position(|&c| c == 0).unwrap_or(slice.len());
        OsString::from_wide(&slice[..len])
            .to_string_lossy()
            .trim()
            .to_string()
    }

    fn query_reg_string(hkey: HKEY, value_name: &str) -> Option<String> {
        let wide_name = to_wide(value_name);
        let mut data_type = 0u32;
        let mut size = 0u32;
        unsafe {
            if RegQueryValueExW(
                hkey,
                wide_name.as_ptr(),
                std::ptr::null(),
                &mut data_type,
                std::ptr::null_mut(),
                &mut size,
            ) != 0
                || data_type != REG_SZ
                || size < 2
            {
                return None;
            }
            let mut buf = vec![0u16; (size as usize + 1) / 2];
            if RegQueryValueExW(
                hkey,
                wide_name.as_ptr(),
                std::ptr::null(),
                &mut data_type,
                buf.as_mut_ptr() as *mut u8,
                &mut size,
            ) != 0
            {
                return None;
            }
            let s = from_wide_slice(&buf);
            if s.is_empty() { None } else { Some(s) }
        }
    }

    fn query_reg_dword(hkey: HKEY, value_name: &str) -> Option<u32> {
        let wide_name = to_wide(value_name);
        let mut data_type = 0u32;
        let mut val = 0u32;
        let mut size = 4u32;
        unsafe {
            if RegQueryValueExW(
                hkey,
                wide_name.as_ptr(),
                std::ptr::null(),
                &mut data_type,
                &mut val as *mut u32 as *mut u8,
                &mut size,
            ) == 0
                && data_type == REG_DWORD
            {
                Some(val)
            } else {
                None
            }
        }
    }

    fn enumerate_mmdevices(flow: &str) -> Vec<AudioDevice> {
        let base_path = format!(
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\MMDevices\\Audio\\{}",
            flow
        );
        let wide_base = to_wide(&base_path);
        let mut root_key: HKEY = 0;
        let mut out = Vec::new();
        unsafe {
            if RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                wide_base.as_ptr(),
                0,
                KEY_READ,
                &mut root_key,
            ) != 0
            {
                return out;
            }
            let mut idx = 0u32;
            loop {
                let mut name_buf = [0u16; 128];
                let mut name_len = name_buf.len() as u32;
                let res = RegEnumKeyExW(
                    root_key,
                    idx,
                    name_buf.as_mut_ptr(),
                    &mut name_len,
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                );
                if res != 0 {
                    break;
                }
                idx += 1;
                let guid = from_wide_slice(&name_buf[..name_len as usize]);
                if guid.is_empty() {
                    continue;
                }
                let ep_wide = to_wide(&guid);
                let mut ep_key: HKEY = 0;
                if RegOpenKeyExW(root_key, ep_wide.as_ptr(), 0, KEY_READ, &mut ep_key) == 0 {
                    let state = query_reg_dword(ep_key, "DeviceState").unwrap_or(0);
                    if state == 1 {
                        let props_wide = to_wide("Properties");
                        let mut props_key: HKEY = 0;
                        if RegOpenKeyExW(ep_key, props_wide.as_ptr(), 0, KEY_READ, &mut props_key)
                            == 0
                        {
                            // {a45c254e-df1c-4efd-8020-67d146a850e0},2 = Endpoint description (e.g., "Microphone", "Speakers")
                            // {b3f8fa53-0004-438e-9003-51a46e139bfc},6 = Adapter friendly name (e.g., "Realtek(R) Audio")
                            let desc = query_reg_string(
                                props_key,
                                "{a45c254e-df1c-4efd-8020-67d146a850e0},2",
                            );
                            let adapter = query_reg_string(
                                props_key,
                                "{b3f8fa53-0004-438e-9003-51a46e139bfc},6",
                            );
                            let full_name = match (desc, adapter) {
                                (Some(d), Some(a)) if !d.contains(&a) => format!("{} ({})", d, a),
                                (Some(d), _) => d,
                                (None, Some(a)) => a,
                                (None, None) => String::new(),
                            };
                            if !full_name.is_empty()
                                && !out.iter().any(|existing: &AudioDevice| existing.name == full_name)
                            {
                                out.push(AudioDevice {
                                    id: guid.clone(),
                                    name: full_name,
                                });
                            }
                            RegCloseKey(props_key);
                        }
                    }
                    RegCloseKey(ep_key);
                }
            }
            RegCloseKey(root_key);
        }
        out
    }

    pub fn enumerate() -> SystemAudioDevices {
        let mut inputs = vec![AudioDevice {
            id: "default".into(),
            name: "Default microphone".into(),
        }];
        let mut outputs = vec![AudioDevice {
            id: "default".into(),
            name: "Default speakers".into(),
        }];

        let mm_inputs = enumerate_mmdevices("Capture");
        if !mm_inputs.is_empty() {
            inputs.extend(mm_inputs);
        } else {
            unsafe {
                let count = waveInGetNumDevs();
                for i in 0..count {
                    let mut caps: WAVEINCAPSW = std::mem::zeroed();
                    if waveInGetDevCapsW(
                        i as usize,
                        &mut caps,
                        std::mem::size_of::<WAVEINCAPSW>() as u32,
                    ) == 0
                    {
                        let raw_pname = std::ptr::addr_of!(caps.sz_pname);
                        let slice = std::slice::from_raw_parts(raw_pname as *const u16, 32);
                        let name = from_wide_slice(slice);
                        if !name.is_empty() {
                            inputs.push(AudioDevice {
                                id: format!("wavein-{i}"),
                                name,
                            });
                        }
                    }
                }
            }
        }

        let mm_outputs = enumerate_mmdevices("Render");
        if !mm_outputs.is_empty() {
            outputs.extend(mm_outputs);
        } else {
            unsafe {
                let count = waveOutGetNumDevs();
                for i in 0..count {
                    let mut caps: WAVEOUTCAPSW = std::mem::zeroed();
                    if waveOutGetDevCapsW(
                        i as usize,
                        &mut caps,
                        std::mem::size_of::<WAVEOUTCAPSW>() as u32,
                    ) == 0
                    {
                        let raw_pname = std::ptr::addr_of!(caps.sz_pname);
                        let slice = std::slice::from_raw_parts(raw_pname as *const u16, 32);
                        let name = from_wide_slice(slice);
                        if !name.is_empty() {
                            outputs.push(AudioDevice {
                                id: format!("waveout-{i}"),
                                name,
                            });
                        }
                    }
                }
            }
        }

        SystemAudioDevices { inputs, outputs }
    }
}

pub fn enumerate_audio_devices() -> SystemAudioDevices {
    #[cfg(windows)]
    {
        win_audio::enumerate()
    }
    #[cfg(not(windows))]
    {
        SystemAudioDevices {
            inputs: vec![AudioDevice {
                id: "default".into(),
                name: "Default microphone".into(),
            }],
            outputs: vec![AudioDevice {
                id: "default".into(),
                name: "Default speakers".into(),
            }],
        }
    }
}
