//! The app icon follows the theme while the app runs. Windows has no icon theming of its
//! own, but a window can replace its icon at any time, so each theme's is drawn here from
//! one vector shape: the theme's background behind its accent, as in the design frames.
use crate::state::AppTheme;
use image::RgbaImage;

/// The shared shape and effects. Each theme fills in its colours.
const TEMPLATE: &str = include_str!("../assets/app-icon.svg");

/// How much lighter the background gets toward the bottom, measured from the design frames.
const DEPTH: f32 = 0.234;

struct IconColors {
    background: u32,
    bottom: u32,
    mark: u32,
    edge: u32,
}

/// Lightens in HSL, which keeps the hue and saturation as they are.
fn lighter(hex: u32, amount: f32) -> u32 {
    let channel = |shift: u32| ((hex >> shift) & 255) as f32 / 255.;
    let (r, g, b) = (channel(16), channel(8), channel(0));
    let (high, low) = (r.max(g).max(b), r.min(g).min(b));
    let light = (high + low) / 2.;
    let chroma = high - low;
    let saturation = if chroma == 0. { 0. } else { chroma / (1. - (2. * light - 1.).abs()) };
    let hue = if chroma == 0. {
        0.
    } else if high == r {
        ((g - b) / chroma).rem_euclid(6.)
    } else if high == g {
        (b - r) / chroma + 2.
    } else {
        (r - g) / chroma + 4.
    };
    let light = (light + amount).min(0.6);
    let chroma = (1. - (2. * light - 1.).abs()) * saturation;
    let x = chroma * (1. - (hue.rem_euclid(2.) - 1.).abs());
    let m = light - chroma / 2.;
    let (r, g, b) = match hue as u32 {
        0 => (chroma, x, 0.),
        1 => (x, chroma, 0.),
        2 => (0., chroma, x),
        3 => (0., x, chroma),
        4 => (x, 0., chroma),
        _ => (chroma, 0., x),
    };
    let byte = |value: f32| (((value + m) * 255.).round() as u32).min(255);
    byte(r) << 16 | byte(g) << 8 | byte(b)
}

/// Double brightness, which is what the frames' outline measures as.
fn doubled(hex: u32) -> u32 {
    let channel = |shift: u32| (((hex >> shift) & 255) * 2).min(255);
    channel(16) << 16 | channel(8) << 8 | channel(0)
}

fn colors(theme: AppTheme) -> IconColors {
    // Default has no colour of its own to show, so it stays black and white.
    let (background, mark) = if theme == AppTheme::DefaultDark {
        (0x000000, 0xffffff)
    } else {
        let palette = theme.palette();
        (palette.stage, palette.accent)
    };
    IconColors { background, bottom: lighter(background, DEPTH), mark, edge: doubled(mark) }
}

pub fn svg(theme: AppTheme) -> String {
    let colors = colors(theme);
    TEMPLATE
        .replace("{BACKGROUND}", &format!("#{:06X}", colors.background))
        .replace("{BOTTOM}", &format!("#{:06X}", colors.bottom))
        .replace("{MARK}", &format!("#{:06X}", colors.mark))
        .replace("{EDGE}", &format!("#{:06X}", colors.edge))
        .replace("{GLOW_BLUR}", "5")
        .replace("{GLOW}", "0.36")
}

pub fn compose(theme: AppTheme, size: u32) -> Option<RgbaImage> {
    use resvg::{tiny_skia, usvg};
    let tree = usvg::Tree::from_str(&svg(theme), &usvg::Options::default()).ok()?;
    let mut canvas = tiny_skia::Pixmap::new(size, size)?;
    let scale = size as f32 / tree.size().width();
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut canvas.as_mut());
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    for pixel in canvas.pixels() {
        let pixel = pixel.demultiply();
        pixels.extend_from_slice(&[pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()]);
    }
    RgbaImage::from_raw(size, size, pixels)
}

#[cfg(windows)]
mod native {
    use std::ffi::c_void;
    #[link(name = "user32")]
    unsafe extern "system" {
        pub fn CreateIcon(
            instance: *mut c_void,
            width: i32,
            height: i32,
            planes: u8,
            bits: u8,
            and_mask: *const u8,
            xor_mask: *const u8,
        ) -> isize;
        pub fn DestroyIcon(icon: isize) -> i32;
        pub fn SendMessageW(window: isize, message: u32, wparam: usize, lparam: isize) -> isize;
        pub fn SetClassLongPtrW(window: isize, index: i32, value: isize) -> isize;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    }
    pub const WM_SETICON: u32 = 0x0080;
    pub const GCLP_HICON: i32 = -14;
    pub const GCLP_HICONSM: i32 = -34;
}

#[cfg(windows)]
fn native_icon(theme: AppTheme, size: u32) -> Option<isize> {
    use native::*;
    let mut pixels = compose(theme, size)?;
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    // All clear: the colour image's own alpha decides what shows.
    let mask = vec![0u8; (size.div_ceil(16) * 2 * size) as usize];
    let icon = unsafe {
        CreateIcon(
            GetModuleHandleW(std::ptr::null()),
            size as i32,
            size as i32,
            1,
            32,
            mask.as_ptr(),
            pixels.as_ptr(),
        )
    };
    (icon != 0).then_some(icon)
}

/// Points the window at this theme's icon. The icon inside the exe, which a pinned shortcut
/// shows before the app has started, stays the Default one.
#[cfg(windows)]
pub fn apply(window: &gpui::Window, theme: AppTheme) {
    use native::*;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Ok(handle) = HasWindowHandle::window_handle(window) else { return };
    let RawWindowHandle::Win32(raw) = handle.as_raw() else { return };
    let hwnd = raw.hwnd.get();
    // Drawing takes a moment, so it's off the UI thread, and a newer choice replaces this one.
    static LATEST: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let request = LATEST.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    std::thread::spawn(move || {
        for (kind, class_slot, size) in [(1usize, GCLP_HICON, 64), (0, GCLP_HICONSM, 32)] {
            if LATEST.load(std::sync::atomic::Ordering::SeqCst) != request {
                return;
            }
            let Some(icon) = native_icon(theme, size) else { continue };
            unsafe {
                let old = SendMessageW(hwnd, WM_SETICON, kind, icon);
                SetClassLongPtrW(hwnd, class_slot, icon);
                if old != 0 {
                    DestroyIcon(old);
                }
            }
        }
    });
}

#[cfg(not(windows))]
pub fn apply(_: &gpui::Window, _: AppTheme) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn contains(icon: &RgbaImage, hex: u32) -> bool {
        let color = [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255];
        icon.pixels().any(|pixel| pixel.0 == color)
    }

    #[test]
    fn your_frames_come_out_with_their_measured_colours() {
        let blue = colors(AppTheme::DeepBlue);
        assert_eq!((blue.background, blue.mark), (0x0D1C3F, 0x4A71C9));
        assert_eq!(blue.edge, 0x94E2FF);
        let purple = colors(AppTheme::Fuchsia);
        assert_eq!((purple.background, purple.mark), (0x510B51, 0xB14CB1));
        assert_eq!(purple.edge, 0xFF98FF);
        // Within a little rounding of what the frames fade to at the very bottom.
        let close = |a: u32, b: u32| (0..3).all(|i| ((a >> (i * 8)) & 255).abs_diff((b >> (i * 8)) & 255) <= 8);
        assert!(close(blue.bottom, 0x2249A5), "{:06X}", blue.bottom);
        assert!(close(purple.bottom, 0xB719B7), "{:06X}", purple.bottom);
    }

    #[test]
    fn every_theme_draws_its_two_colours_inside_a_rounded_square() {
        for theme in AppTheme::ALL {
            let icon = compose(theme, 128).unwrap();
            let mark = colors(theme).mark;
            assert!(contains(&icon, mark), "{} mark", theme.label());
            assert_eq!(icon.get_pixel(0, 0).0[3], 0, "{} corner", theme.label());
        }
    }

    /// Writes one SVG per theme, the exe's icon file and a sheet to look over.
    /// Run again after changing `assets/app-icon.svg` or the theme colours.
    #[test]
    #[ignore]
    fn write_icon_files() {
        let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        std::fs::create_dir_all(folder.join("app-icons")).unwrap();
        for theme in AppTheme::ALL {
            std::fs::write(folder.join(format!("app-icons/{}.svg", theme.id())), svg(theme)).unwrap();
        }
        // An .ico is a small directory followed by one bitmap per size. These are plain
        // bitmaps, because the NSIS installer builder refuses PNG entries.
        let sizes = [256u32, 64, 48, 32, 24, 16];
        let images: Vec<Vec<u8>> = sizes
            .iter()
            .map(|size| {
                let icon = compose(AppTheme::DefaultDark, *size).unwrap();
                let mut bitmap = Vec::new();
                bitmap.extend_from_slice(&40u32.to_le_bytes());
                bitmap.extend_from_slice(&size.to_le_bytes());
                bitmap.extend_from_slice(&(size * 2).to_le_bytes());
                bitmap.extend_from_slice(&1u16.to_le_bytes());
                bitmap.extend_from_slice(&32u16.to_le_bytes());
                bitmap.extend_from_slice(&[0; 24]);
                for row in icon.rows().rev() {
                    for pixel in row {
                        let [r, g, b, a] = pixel.0;
                        bitmap.extend_from_slice(&[b, g, r, a]);
                    }
                }
                // The legacy transparency mask, unused beside 32-bit alpha but required.
                bitmap.resize(bitmap.len() + (size.div_ceil(32) * 4 * size) as usize, 0);
                bitmap
            })
            .collect();
        let mut ico = vec![0, 0, 1, 0, sizes.len() as u8, 0];
        let mut offset = 6 + 16 * sizes.len() as u32;
        for (size, png) in sizes.iter().zip(&images) {
            ico.extend_from_slice(&[*size as u8, *size as u8, 0, 0, 1, 0, 32, 0]);
            ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
            ico.extend_from_slice(&offset.to_le_bytes());
            offset += png.len() as u32;
        }
        for png in &images {
            ico.extend_from_slice(png);
        }
        std::fs::write(folder.join("nitro.ico"), ico).unwrap();

        let (columns, cell) = (10u32, 84u32);
        let rows = (AppTheme::ALL.len() as u32).div_ceil(columns);
        let mut sheet = RgbaImage::from_pixel(columns * cell, rows * cell, image::Rgba([48, 48, 52, 255]));
        for (index, theme) in AppTheme::ALL.into_iter().enumerate() {
            let (x, y) = (index as u32 % columns * cell + 6, index as u32 / columns * cell + 6);
            image::imageops::overlay(&mut sheet, &compose(theme, 72).unwrap(), x as i64, y as i64);
        }
        sheet.save(folder.join("app-icons/sheet.png")).unwrap();
        // For comparing with the design frames at their own size.
        if let Ok(out) = std::env::var("NITRO_ICON_COMPARE") {
            compose(AppTheme::DeepBlue, 2000).unwrap().save(format!("{out}/mine169.png")).unwrap();
            compose(AppTheme::Fuchsia, 2000).unwrap().save(format!("{out}/mine168.png")).unwrap();
        }
    }
}
