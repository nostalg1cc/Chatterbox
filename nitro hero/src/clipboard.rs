//! Reads clipboard text directly. GPUI's reader tries once and returns only the first
//! format it recognises, so paste silently failed when another app (password manager,
//! clipboard history) held the clipboard or when an image format was listed before text.
#[cfg(windows)]
pub fn read_text() -> Option<String> {
    use std::{ffi::c_void, ptr, thread, time::Duration};
    const CF_UNICODETEXT: u32 = 13;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn OpenClipboard(owner: *mut c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn GetClipboardData(format: u32) -> *mut c_void;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GlobalLock(memory: *mut c_void) -> *mut c_void;
        fn GlobalUnlock(memory: *mut c_void) -> i32;
    }

    for attempt in 0..10 {
        if attempt > 0 {
            thread::sleep(Duration::from_millis(15));
        }
        unsafe {
            if OpenClipboard(ptr::null_mut()) == 0 {
                continue;
            }
            let mut text = None;
            let handle = GetClipboardData(CF_UNICODETEXT);
            if !handle.is_null() {
                let data = GlobalLock(handle) as *const u16;
                if !data.is_null() {
                    let mut len = 0;
                    while *data.add(len) != 0 {
                        len += 1;
                    }
                    text = Some(String::from_utf16_lossy(std::slice::from_raw_parts(data, len)));
                    GlobalUnlock(handle);
                }
            }
            CloseClipboard();
            return text;
        }
    }
    None
}

#[cfg(not(windows))]
pub fn read_text() -> Option<String> {
    None
}

/// What can be attached from the clipboard.
pub enum ClipboardMedia {
    /// Image bytes in a format `image` can read.
    Image(Vec<u8>),
    /// A file copied in Explorer.
    File(std::path::PathBuf),
}

/// Turns a clipboard DIB, which has no file header, into a readable BMP.
fn dib_to_bmp(dib: &[u8]) -> Option<Vec<u8>> {
    if dib.len() < 40 {
        return None;
    }
    let header = u32::from_le_bytes(dib[0..4].try_into().ok()?);
    let bits = u16::from_le_bytes(dib[14..16].try_into().ok()?) as u32;
    let compression = u32::from_le_bytes(dib[16..20].try_into().ok()?);
    let used = u32::from_le_bytes(dib[32..36].try_into().ok()?);
    let colors = if used != 0 {
        used
    } else if bits <= 8 {
        1 << bits
    } else {
        0
    };
    // BI_BITFIELDS keeps three colour masks after a plain 40-byte header.
    let masks = if compression == 3 && header == 40 { 12 } else { 0 };
    let offset = 14 + header + masks + colors * 4;
    let mut bmp = Vec::with_capacity(14 + dib.len());
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&(14 + dib.len() as u32).to_le_bytes());
    bmp.extend_from_slice(&[0; 4]);
    bmp.extend_from_slice(&offset.to_le_bytes());
    bmp.extend_from_slice(dib);
    Some(bmp)
}

#[cfg(windows)]
pub fn read_media() -> Option<ClipboardMedia> {
    use std::{ffi::c_void, ptr, thread, time::Duration};
    const CF_DIB: u32 = 8;
    const CF_HDROP: u32 = 15;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn OpenClipboard(owner: *mut c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn GetClipboardData(format: u32) -> *mut c_void;
        fn RegisterClipboardFormatW(name: *const u16) -> u32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GlobalLock(memory: *mut c_void) -> *mut c_void;
        fn GlobalUnlock(memory: *mut c_void) -> i32;
        fn GlobalSize(memory: *mut c_void) -> usize;
    }
    #[link(name = "shell32")]
    unsafe extern "system" {
        fn DragQueryFileW(drop: *mut c_void, index: u32, file: *mut u16, length: u32) -> u32;
    }

    unsafe fn copy(handle: *mut c_void) -> Option<Vec<u8>> {
        unsafe {
            if handle.is_null() {
                return None;
            }
            let data = GlobalLock(handle) as *const u8;
            if data.is_null() {
                return None;
            }
            let bytes = std::slice::from_raw_parts(data, GlobalSize(handle)).to_vec();
            GlobalUnlock(handle);
            Some(bytes)
        }
    }

    let png_name: Vec<u16> = "PNG".encode_utf16().chain(Some(0)).collect();
    for attempt in 0..10 {
        if attempt > 0 {
            thread::sleep(Duration::from_millis(15));
        }
        unsafe {
            if OpenClipboard(ptr::null_mut()) == 0 {
                continue;
            }
            let drop = GetClipboardData(CF_HDROP);
            let mut found = None;
            if !drop.is_null() {
                let length = DragQueryFileW(drop, 0, ptr::null_mut(), 0);
                if length > 0 {
                    let mut path = vec![0u16; length as usize + 1];
                    DragQueryFileW(drop, 0, path.as_mut_ptr(), length + 1);
                    path.truncate(length as usize);
                    found = Some(ClipboardMedia::File(String::from_utf16_lossy(&path).into()));
                }
            }
            if found.is_none() {
                // PNG keeps transparency, which screenshots and browsers often offer too.
                let png = RegisterClipboardFormatW(png_name.as_ptr());
                found = copy(GetClipboardData(png)).map(ClipboardMedia::Image);
            }
            if found.is_none() {
                found = copy(GetClipboardData(CF_DIB))
                    .and_then(|dib| dib_to_bmp(&dib))
                    .map(ClipboardMedia::Image);
            }
            CloseClipboard();
            return found;
        }
    }
    None
}

#[cfg(not(windows))]
pub fn read_media() -> Option<ClipboardMedia> {
    None
}

#[cfg(test)]
mod tests {
    use super::dib_to_bmp;

    #[test]
    fn a_clipboard_bitmap_becomes_a_readable_image() {
        // 2x1, 32-bit, bottom-up: a blue pixel, then a red one.
        let mut dib = Vec::new();
        dib.extend_from_slice(&40u32.to_le_bytes());
        dib.extend_from_slice(&2i32.to_le_bytes());
        dib.extend_from_slice(&1i32.to_le_bytes());
        dib.extend_from_slice(&1u16.to_le_bytes());
        dib.extend_from_slice(&32u16.to_le_bytes());
        dib.extend_from_slice(&[0; 24]);
        dib.extend_from_slice(&[255, 0, 0, 255, 0, 0, 255, 255]);
        let image = image::load_from_memory(&dib_to_bmp(&dib).unwrap()).unwrap().to_rgb8();
        assert_eq!((image.width(), image.height()), (2, 1));
        assert_eq!(image.get_pixel(0, 0).0, [0, 0, 255]);
        assert_eq!(image.get_pixel(1, 0).0, [255, 0, 0]);
    }

    #[test]
    fn a_truncated_bitmap_is_refused() {
        assert!(dib_to_bmp(&[0; 10]).is_none());
    }
}
