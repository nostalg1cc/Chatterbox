use gpui::{
    Animation, AnimationExt, App, Bounds, Canvas, ClickEvent, Div,
    FocusHandle, Hsla, MouseButton, Pixels, SharedString, Stateful, Transformation, Window,
    canvas, div, hsla, img, percentage, prelude::*, px, relative, rgba, svg,
};
use crate::state::palette;
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// The shared accent. Every accent role uses this value.
pub const ACCENT: u32 = 0x0a84ff;

// How dark or light a surface is depends only on what it sits on:
//   MATERIAL_SUBTLE / MATERIAL_PANEL  directly on the background or content: the darkest.
//                                     The composer, call module, tabs, pills, menus and cards.
//   MATERIAL_DARK                     on top of one of those: a step lighter. Buttons and rows.
//   0x34343a                          selected: the lightest.
// Colour is only ever for meaning: accent for the main action, green for live, red for danger.
pub const MATERIAL_DARK: u32 = 0x282828;
pub const MATERIAL_SUBTLE: u32 = 0x202022;
pub const MATERIAL_PANEL: u32 = 0x1e1e20;
pub const MATERIAL_GREEN: u32 = 0x00b339;
/// The one height for standalone controls: tabs, buttons, pills, fields and the composer.
pub const CONTROL_HEIGHT: f32 = 44.;
/// The one height for anything sitting inside a control, or a small button beside text.
pub const INNER_HEIGHT: f32 = 36.;
pub const MATERIAL_RED: u32 = 0xff3333;

const TEXT: u32 = 0xf2f2f3;
const MUTED: u32 = 0xa1a1aa;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StrokeKind {
    Neutral,
    Colored,
    AccentFocus,
}

/// Maps Nitro's colour roles to the active theme. Status and personal-art colours
/// outside these roles stay unchanged; only Default keeps the normal blue accent.
pub fn themed(hex: u32) -> u32 {
    themed_with(palette(), hex)
}

fn themed_with(p: crate::state::Palette, hex: u32) -> u32 {
    match hex {
        ACCENT => p.accent,
        MATERIAL_DARK => p.surface,
        MATERIAL_SUBTLE => p.subtle,
        MATERIAL_PANEL => p.panel,
        0x34343a | 0x343438 | 0x323238 | 0x303036 | 0x26262b | 0x2e2e34 | 0x2a2a30 | 0x2c2d2d
        | 0x2a2a2e | 0x38383d | 0x252626 | 0x3d4a58 => p.raised,
        0x141416 | 0x161618 | 0x191919 => p.panel,
        0x121318 | 0x18181b => p.media,
        0x0d0d0d => p.stage,
        0x29292c | 0x525259 => p.hairline,
        0xf2f2f3 | 0xf5f5f7 | 0xf3f3f4 | 0xf1f1f3 | 0xe8e8ea | 0xe4e4e7 | 0xd9d9d9 | 0xd8d8df
        | 0xd4d4d8 => p.text,
        0xc4c4cc | 0xc2c2c8 | 0xb4b4be | 0xa1a1aa | 0x9e9ea8 | 0x8e8e98 | 0x71717a | 0x6d6d73
        | 0x6b6b6b => p.muted,
        0x80bfff | 0x9cc9ff | 0xb1d9ff | 0xa8d5ff | 0xc6e1fa | 0x9dffb8 => p.accent_soft,
        0x6cb4ff => p.accent_line,
        0x163824 => p.selected,
        0x173d5c => if crate::state::active_is_default() { hex } else { p.selected },
        // Red text would be hard to read on the brighter themes, so it lightens there.
        0xff6467 | 0xff6b6b | 0xff858d => readable_red(p.surface),
        _ => hex,
    }
}

fn relative_luminance(hex: u32) -> f32 {
    let channel = |value: u32| {
        let value = value as f32 / 255.;
        if value <= 0.04045 { value / 12.92 } else { ((value + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * channel((hex >> 16) & 255) + 0.7152 * channel((hex >> 8) & 255) + 0.0722 * channel(hex & 255)
}

fn readable_red(surface: u32) -> u32 {
    let background = relative_luminance(surface);
    for red in [0xff6467, 0xff8a8d, 0xffb0b2, 0xffd0d1] {
        if (relative_luminance(red) + 0.05) / (background + 0.05) >= 3.2 {
            return red;
        }
    }
    0xffe6e6
}

/// Every colour in the app goes through this, so components follow the active theme.
pub fn rgb(hex: u32) -> gpui::Rgba {
    gpui::rgb(themed(hex))
}

/// Whether every surface is drawn as glass. Development builds have a switch for comparing.
static GLASS_UI: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

thread_local! {
    /// Overrides the switch for part of the interface, for the side-by-side comparison.
    static GLASS_FORCED: Cell<Option<bool>> = const { Cell::new(None) };
}

pub fn glass_ui() -> bool {
    GLASS_FORCED.with(Cell::get).unwrap_or_else(|| GLASS_UI.load(std::sync::atomic::Ordering::Relaxed))
}

/// Builds and paints `content` as glass or not, whatever the switch says. Surfaces decide when
/// they're built and when they paint, so it's set for both.
pub fn with_glass(id: &'static str, on: bool, build: impl FnOnce() -> Div) -> Stateful<Div> {
    GLASS_FORCED.with(|forced| forced.set(Some(on)));
    let content = build();
    GLASS_FORCED.with(|forced| forced.set(None));
    div()
        .id(id)
        .child(canvas(|_, _, _| (), move |_, _, _, _| GLASS_FORCED.with(|forced| forced.set(Some(on)))).absolute().size_0())
        .child(content)
        .child(canvas(|_, _, _| (), |_, _, _, _| GLASS_FORCED.with(|forced| forced.set(None))).absolute().size_0())
}

pub fn set_glass_ui(on: bool) {
    GLASS_UI.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// How one piece of glass differs from the standard settings.
#[derive(Clone, Copy, Debug)]
pub struct GlassLook {
    /// A colour, and how strongly it shows.
    pub tint: Option<(u32, f32)>,
    pub highlight: f32,
    pub blur: f32,
    pub lift: f32,
    /// The crisp top-lit line inside the edge, as on the solid surfaces. 1 matches them.
    pub rim: f32,
    /// The dark line outside the edge, as on the solid surfaces. 1 matches them.
    pub ring: f32,
}

impl GlassLook {
    pub const STANDARD: Self = Self { tint: None, highlight: 1., blur: 1., lift: 1., rim: 0., ring: 0. };
    /// Carved in: darker, with a quiet rim. What a sliding piece sits in.
    pub const WELL: Self = Self { tint: Some((0x000000, 0.24)), highlight: 1.5, blur: 1.4, lift: 0.7, rim: 0.9, ring: 1. };
    /// A bright slab raised above a well: a strong rim and a real shadow.
    pub const SLAB: Self = Self { tint: Some((0xffffff, 0.15)), highlight: 5., blur: 0.6, lift: 0.9, rim: 1.1, ring: 0.3 };
    /// A slider's handle.
    pub const KNOB: Self = Self { tint: Some((0xffffff, 0.14)), highlight: 4., blur: 0.5, lift: 0.9, rim: 1.1, ring: 0.3 };
    /// A large panel or menu: darker and more diffused, as text has to read on it.
    pub const PANEL: Self = Self { tint: Some((0xffffff, 0.05)), highlight: 2., blur: 2.4, lift: 1.3, rim: 1., ring: 1. };
    /// An ordinary button or pill. The standard settings suit a busy backdrop. Interface surfaces
    /// often sit on flat colour, where they need more rim and a little body to have an edge.
    pub const SURFACE: Self = Self { tint: Some((0xffffff, 0.04)), highlight: 2., blur: 1., lift: 1.2, rim: 1., ring: 1. };

    fn tinted(colour: u32) -> Self {
        Self { tint: Some((colour, 0.54)), highlight: 2., blur: 1., lift: 1.2, rim: 1.64, ring: 1. }
    }

    /// The glass standing in for one of the app's existing surfaces.
    /// `role` is the colour before theming. A theme can tint its neutral surfaces, and those must
    /// stay clear glass, not become coloured buttons.
    fn for_surface(role: u32, fill: u32, height: f32) -> Self {
        let colour: Hsla = gpui::rgb(role).into();
        if colour.s > 0.3 {
            return Self::tinted(fill);
        }
        let on_the_background = matches!(role, MATERIAL_SUBTLE | MATERIAL_PANEL);
        match role {
            // Tall surfaces hold text over whatever's behind them, so they diffuse it more.
            _ if on_the_background && height >= 120. => Self { tint: Some((fill, 0.42)), ..Self::PANEL },
            _ if on_the_background => Self::WELL,
            0x34343a | 0x323238 | 0x303036 | 0x2a2a30 => Self::SLAB,
            _ if height >= 120. => Self { tint: Some((fill, 0.42)), ..Self::PANEL },
            _ => Self::SURFACE,
        }
    }
}

fn paint_look(bounds: Bounds<Pixels>, radius: f32, look: GlassLook, window: &mut Window) {
    let tint = look.tint.map(|(colour, strength)| (gpui::rgb(colour), strength));
    window.paint_glass(bounds, px(radius), tint, look.highlight, look.blur, look.lift, look.rim, look.ring);
}

/// Paints Nitro's 3-layer surface + light-catching gradient ring + outer 1.25px black contact ring.
pub fn material_canvas(fill_hex: u32, radius: f32, stroke: StrokeKind) -> Canvas<()> {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let role_hex = fill_hex;
            let fill_hex = themed(fill_hex);
            let r = radius
                .min(f32::from(bounds.size.height) / 2.)
                .min(f32::from(bounds.size.width) / 2.);
            if glass_ui() {
                paint_look(bounds, r, GlassLook::for_surface(role_hex, fill_hex, f32::from(bounds.size.height)), window);
                return;
            }

            // 1. Outer 1.25px black (80% opacity) contact ring separating surface from canvas.
            let mut ring = gpui::fill(bounds.dilate(px(1.25)), rgba(0x000000cc));
            ring.corner_radii = px(r + 1.25).into();
            window.paint_quad(ring);

            // 2. Base fill clipped to border-box so the translucent gradient ring composites
            //    against the surface's own color rather than the page behind it.
            let mut base = gpui::fill(bounds, rgb(fill_hex));
            base.corner_radii = px(r).into();
            window.paint_quad(base);

            // 3. 5-stop 180deg top-lit gradient stroke (0%, 50%, 75%, 88%, 100%).
            let stops: [(f32, Hsla); 5] = match stroke {
                StrokeKind::Neutral => [
                    (0.00, hsla(0., 0., 1., 0.28)),
                    (0.50, hsla(0., 0., 1., 0.12)),
                    (0.75, hsla(0., 0., 1., 0.03)),
                    (0.88, hsla(0., 0., 1., 0.12)),
                    (1.00, hsla(0., 0., 1., 0.28)),
                ],
                StrokeKind::Colored => [
                    (0.00, hsla(0., 0., 1., 0.46)),
                    (0.50, hsla(0., 0., 1., 0.22)),
                    (0.75, hsla(0., 0., 1., 0.07)),
                    (0.88, hsla(0., 0., 1., 0.22)),
                    (1.00, hsla(0., 0., 1., 0.46)),
                ],
                StrokeKind::AccentFocus => [
                    (0.00, rgba((palette().accent_soft << 8) | 0x8c).into()),
                    (0.50, hsla(0., 0., 1., 0.14)),
                    (0.75, hsla(0., 0., 1., 0.04)),
                    (0.88, hsla(0., 0., 1., 0.14)),
                    (1.00, rgba((palette().accent_soft << 8) | 0x8c).into()),
                ],
            };
            for pair in stops.windows(2) {
                let ((start, color_a), (end, color_b)) = (pair[0], pair[1]);
                let clip = gpui::Bounds::new(
                    gpui::point(
                        bounds.origin.x,
                        bounds.origin.y + bounds.size.height * start,
                    ),
                    gpui::size(bounds.size.width, bounds.size.height * (end - start)),
                );
                let gradient = gpui::linear_gradient(
                    180.,
                    gpui::linear_color_stop(color_a, start),
                    gpui::linear_color_stop(color_b, end),
                );
                let mut stroke_quad = gpui::fill(bounds, gradient);
                stroke_quad.corner_radii = px(r).into();
                window.with_content_mask(Some(gpui::ContentMask { bounds: clip }), |w| {
                    w.paint_quad(stroke_quad);
                });
            }

            // 4. Interior fill clipped to padding-box (inset 1.25px), leaving only the 1.25px
            //    gradient stroke ring exposed around the perimeter.
            let inner = bounds.inset(px(1.25));
            let inner_r = (r - 1.25).max(0.);
            let mut center = gpui::fill(inner, rgb(fill_hex));
            center.corner_radii = px(inner_r).into();
            window.paint_quad(center);

            // Subtle top/bottom interior convex sheen from button.md.
            let sheen = gpui::linear_gradient(
                180.,
                gpui::linear_color_stop(hsla(0., 0., 1., 0.035), 0.),
                gpui::linear_color_stop(hsla(0., 0., 1., 0.0), 0.45),
            );
            let mut sheen_quad = gpui::fill(inner, sheen);
            sheen_quad.corner_radii = px(inner_r).into();
            window.paint_quad(sheen_quad);
        },
    )
    .absolute()
    .inset_0()
}

pub fn material_surface(fill_hex: u32, radius: f32, stroke: StrokeKind) -> Div {
    div()
        .relative()
        .rounded(px(radius))
        .child(material_canvas(fill_hex, radius, stroke))
}

/// The soft press every button makes, like the main app's interface click.
pub fn press_sound(event: &gpui::MouseDownEvent, _: &mut Window, _: &mut App) {
    if event.button == MouseButton::Left {
        crate::audio::play(crate::audio::UiSound::Click);
    }
}

/// `material_interactive` without the press sound, for controls with a sound of their own.
pub fn material_quiet(
    id: impl Into<gpui::ElementId>,
    group: impl Into<SharedString>,
    fill_hex: u32,
    radius: f32,
    stroke: StrokeKind,
) -> Stateful<Div> {
    let group_name: SharedString = group.into();
    let inner_radius = (radius - 1.25).max(0.);
    div()
        .id(id)
        .group(group_name.clone())
        .relative()
        .rounded(px(radius))
        .cursor_pointer()
        .active(|s| s.opacity(0.88))
        .child(material_canvas(fill_hex, radius, stroke))
        .child(
            div()
                .absolute()
                .inset(px(1.25))
                .rounded(px(inner_radius))
                .group_hover(group_name, |s| s.bg(rgba(0xffffff1f))),
        )
}

pub fn material_interactive(
    id: impl Into<gpui::ElementId>,
    group: impl Into<SharedString>,
    fill_hex: u32,
    radius: f32,
    stroke: StrokeKind,
) -> Stateful<Div> {
    let group_name: SharedString = group.into();
    let inner_radius = (radius - 1.25).max(0.);
    div()
        .id(id)
        .group(group_name.clone())
        .relative()
        .rounded(px(radius))
        .cursor_pointer()
        .active(|s| s.opacity(0.88))
        .capture_any_mouse_down(press_sound)
        .child(material_canvas(fill_hex, radius, stroke))
        .child(
            div()
                .absolute()
                .inset(px(1.25))
                .rounded(px(inner_radius))
                .group_hover(group_name, |s| s.bg(rgba(0xffffff1f))),
        )
}

pub fn section(title: &'static str, caption: &'static str) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(5.))
        .child(div().text_size(px(16.)).text_color(rgb(TEXT)).child(title))
        .child(
            div()
                .text_size(px(12.))
                .text_color(rgb(MUTED))
                .child(caption),
        )
}

/// An avatar's picture and optional decoration overlay. `true` is the bundled sample
/// used by the UI playground.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AvatarImage {
    pub path: Option<PathBuf>,
    pub decoration: Option<PathBuf>,
}
impl From<bool> for AvatarImage {
    fn from(sample: bool) -> Self {
        Self {
            path: sample.then(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/avatar.webp")),
            decoration: None,
        }
    }
}

pub fn avatar(
    initials: impl Into<SharedString>,
    size: f32,
    background: u32,
    foreground: u32,
    image: impl Into<AvatarImage>,
) -> Div {
    let image = image.into();
    // The colour is only for initials; a photo gets a neutral edge so it can't read as status.
    let background = if image.path.is_some() { MATERIAL_DARK } else { background };
    let initials: SharedString = initials.into();
    let radius = size / 2.;
    let stroke = if background == MATERIAL_DARK || background == 0x343438 || background == 0x38383d {
        StrokeKind::Neutral
    } else {
        StrokeKind::Colored
    };
    let mut root = material_surface(background, radius, stroke)
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .size(px(size))
        .p(px(1.25))
        .text_size(px(if size >= 48. {
            16.
        } else if size <= 24. {
            10.
        } else {
            14.
        }))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(rgb(foreground));
    if let Some(path) = image.path {
        root = root.child(
            img(path)
                .size_full()
                .rounded(px((radius - 1.25).max(0.)))
                .object_fit(gpui::ObjectFit::Cover),
        );
    } else {
        root = root.child(initials);
    }
    if let Some(decoration) = image.decoration {
        root = root.child(decoration_layer(decoration, size));
    }
    root
}

/// Same placement as the main app: centred at 118% of the avatar.
fn decoration_layer(path: PathBuf, size: f32) -> gpui::AnyElement {
    let frames = match art_animation(&path) {
        ArtAnimation::Frames(frames) => Some(frames),
        ArtAnimation::Pending => {
            // Repaint soon, so the animation starts once it's decoded.
            EFFECT_ON_SCREEN.store(true, Ordering::Relaxed);
            None
        }
        ArtAnimation::Still => None,
    };
    let Some(frames) = frames else {
        return img(path)
            .absolute()
            .left(px(-size * 0.09))
            .top(px(-size * 0.09))
            .size(px(size * 1.18))
            .object_fit(gpui::ObjectFit::Contain)
            .into_any_element();
    };
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            // The shared 20 fps tick repaints while one of these is on screen.
            EFFECT_ON_SCREEN.store(true, Ordering::Relaxed);
            let frame = art_frame(&frames);
            let _ = window.paint_image(bounds, px(0.).into(), frames, frame, false);
        },
    )
    .absolute()
    .left(px(-size * 0.09))
    .top(px(-size * 0.09))
    .size(px(size * 1.18))
    .into_any_element()
}

pub enum ArtAnimation {
    /// Being decoded; show the first frame for now.
    Pending,
    Still,
    Frames(std::sync::Arc<gpui::RenderImage>),
}

/// The largest size a decoration needs. Sources are 288px, and 60 frames at that size
/// would hold about 20 MB per decoration.
const DECORATION_FRAME_SIZE: u32 = 192;

/// GPUI's image loader only plays GIF and WebP and reduces APNG to one frame, so
/// decorations are decoded here, once per file and off the UI thread.
pub fn art_animation(path: &std::path::Path) -> ArtAnimation {
    type Decoded = Option<std::sync::Arc<gpui::RenderImage>>;
    static DECODED: OnceLock<std::sync::Mutex<std::collections::HashMap<PathBuf, Decoded>>> =
        OnceLock::new();
    static STARTED: OnceLock<std::sync::Mutex<std::collections::HashSet<PathBuf>>> = OnceLock::new();
    let animated = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with("_animated.png"));
    if !animated {
        return ArtAnimation::Still;
    }
    let decoded = DECODED.get_or_init(Default::default);
    if let Some(result) = decoded.lock().unwrap().get(path) {
        return result.clone().map_or(ArtAnimation::Still, ArtAnimation::Frames);
    }
    if STARTED.get_or_init(Default::default).lock().unwrap().insert(path.to_path_buf()) {
        let path = path.to_path_buf();
        std::thread::spawn(move || {
            let frames = decode_apng(&path, DECORATION_FRAME_SIZE).map(std::sync::Arc::new);
            decoded.lock().unwrap().insert(path, frames);
        });
    }
    ArtAnimation::Pending
}

/// True if this art changes over time, or may once it's decoded.
pub fn art_is_animated(art: &AvatarImage) -> bool {
    art.decoration
        .as_ref()
        .is_some_and(|path| !matches!(art_animation(path), ArtAnimation::Still))
}

/// All frames of an APNG in GPUI's BGRA order. `None` if it has fewer than two.
fn decode_apng(path: &std::path::Path, largest: u32) -> Option<gpui::RenderImage> {
    use image::AnimationDecoder;
    let file = std::io::BufReader::new(std::fs::File::open(path).ok()?);
    let decoder = image::codecs::png::PngDecoder::new(file).ok()?;
    if !decoder.is_apng().ok()? {
        return None;
    }
    let mut frames = Vec::new();
    for frame in decoder.apng().ok()?.into_frames() {
        let frame = frame.ok()?;
        let delay = frame.delay();
        let mut pixels = frame.into_buffer();
        if pixels.width() > largest || pixels.height() > largest {
            pixels = image::imageops::resize(
                &pixels,
                largest,
                largest,
                image::imageops::FilterType::Triangle,
            );
        }
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        frames.push(image::Frame::from_parts(pixels, 0, 0, delay));
    }
    (frames.len() > 1).then(|| gpui::RenderImage::new(frames))
}

pub fn avatar_group() -> Div {
    let people = [
        ("AN", 0x324c63, 0xc6e1fa),
        ("JM", 0x534263, 0xe8cff7),
        ("SK", 0x5f4936, 0xffddae),
        ("EL", 0x36584c, 0xc6f4d9),
    ];
    let mut row = div().flex().items_center();
    for (index, (name, bg, fg)) in people.into_iter().enumerate() {
        let mut item = div().rounded_full().child(avatar(name, 40., bg, fg, false));
        if index > 0 {
            item = item.ml(px(-10.));
        }
        row = row.child(item);
    }
    row.child(
        div()
            .ml(px(-10.))
            .rounded_full()
            .child(avatar("+3", 40., MATERIAL_DARK, 0xe4e4e7, false)),
    )
}

// HeroUI v3 uses a 28px medium badge and a 16px small badge, styled with the 3-layer Nitro ring.
pub fn badge(label: impl Into<SharedString>, bg: u32, fg: u32) -> Div {
    let label: SharedString = label.into();
    let stroke = if bg == MATERIAL_DARK || bg == 0x343438 {
        StrokeKind::Neutral
    } else {
        StrokeKind::Colored
    };
    material_surface(bg, 14., stroke)
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .min_w(px(28.))
        .h(px(28.))
        .px(px(if label.len() > 2 { 9. } else { 4. }))
        .text_color(rgb(fg))
        .text_size(px(12.))
        .line_height(px(16.))
        .font_weight(gpui::FontWeight::MEDIUM)
        .child(label)
}

pub fn badge_anchor(
    initials: impl Into<SharedString>,
    value: impl Into<SharedString>,
    bg: u32,
    bottom: bool,
    photo: bool,
) -> Div {
    let initials: SharedString = initials.into();
    let value: SharedString = value.into();
    let badge_width = if value.len() > 1 { 32. } else { 18. };
    let fg = 0xffffff;
    let mut overlay = material_surface(bg, 9., StrokeKind::Colored)
        .absolute()
        .right(px(-badge_width / 4.))
        .w(px(badge_width))
        .h(px(18.))
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgb(fg))
        .text_size(px(10.))
        .line_height(px(13.))
        .font_weight(gpui::FontWeight::MEDIUM);
    overlay = if bottom {
        overlay.bottom(px(-4.5))
    } else {
        overlay.top(px(-4.5))
    };
    if !value.is_empty() {
        overlay = overlay.child(value);
    }
    div()
        .relative()
        .flex_shrink_0()
        .size(px(48.))
        .child(avatar(initials, 48., MATERIAL_DARK, 0xe4e4e7, photo))
        .child(overlay)
}

pub fn alert(
    icon: &'static str,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    color: u32,
) -> Div {
    let color = themed(color);
    let title: SharedString = title.into();
    let description: SharedString = description.into();
    let mut indicator = div()
        .size(px(24.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgb(color));
    if icon == "HeroSpinner" {
        indicator = indicator.child(
            svg()
                .path(icon)
                .size(px(16.))
                .text_color(rgb(color))
                .with_animation(
                    "alert-spinner",
                    Animation::new(Duration::from_millis(900)).repeat(),
                    |icon, progress| {
                        icon.with_transformation(Transformation::rotate(percentage(progress)))
                    },
                ),
        );
    } else {
        indicator = indicator.child(svg().path(icon).size(px(16.)).text_color(rgb(color)));
    }
    let mut content = div().flex_1().min_w_0().flex().flex_col().child(
        div()
            .text_size(px(14.))
            .line_height(px(24.))
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(rgb(color))
            .child(title),
    );
    if !description.is_empty() {
        content = content.child(
            div()
                .text_size(px(14.))
                .line_height(px(20.))
                .text_color(rgb(MUTED))
                .child(description),
        );
    }
    material_surface(MATERIAL_PANEL, 24., StrokeKind::Neutral)
        .flex()
        .items_start()
        .gap(px(16.))
        .w_full()
        .px(px(16.))
        .py(px(12.))
        .child(indicator)
        .child(content)
}

pub fn tab_text(selected: bool) -> Hsla {
    rgb(if selected { 0xffffff } else { 0xa1a1aa }).into()
}

pub fn label(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(12.))
        .text_color(rgb(MUTED))
        .child(text.into())
}

// ---------------------------------------------------------------------------
// Tabs
// ---------------------------------------------------------------------------

const TABS_TRANSITION: Duration = Duration::from_millis(420);
/// The side of the indicator being left behind starts this much later, so it stretches
/// towards the new tab and then catches up, instead of sliding as a rigid block.
const TABS_TRAILING_DELAY: f32 = 0.05;

/// Rises to 1 with a very small overshoot, then settles.
fn tabs_spring(seconds: f32) -> f32 {
    if seconds <= 0. {
        return 0.;
    }
    1. - (-21. * seconds).exp() * (16. * seconds).cos()
}

/// The tabs' spring, for something moving from 0 to 1 over the same time.
pub fn tabs_curve(progress: f32) -> f32 {
    tabs_spring(progress * TABS_TRANSITION.as_secs_f32())
}

/// A value moving with the tabs' spring, stepped frame by frame so that a busy moment pauses
/// it instead of skipping ahead.
#[derive(Clone, Copy, Debug, Default)]
pub struct Tween {
    from: f32,
    to: f32,
    time: f32,
    pub value: f32,
}

impl Tween {
    /// Moves one frame towards `target`. True while it's still moving.
    pub fn step(&mut self, target: f32, seconds: f32) -> bool {
        if (target - self.to).abs() > 0.01 {
            (self.from, self.to, self.time) = (self.value, target, 0.);
        }
        if self.value == target {
            return false;
        }
        self.time += seconds;
        if self.time >= TABS_TRANSITION.as_secs_f32() {
            self.value = target;
            return false;
        }
        self.value = (self.from + (self.to - self.from) * tabs_spring(self.time)).max(0.);
        true
    }
}

/// Mixes two text colours, for a label brightening as the indicator reaches it.
fn mix_colour(from: gpui::Hsla, to: gpui::Hsla, amount: f32) -> gpui::Hsla {
    let amount = amount.clamp(0., 1.);
    gpui::Hsla {
        h: to.h,
        s: from.s + (to.s - from.s) * amount,
        l: from.l + (to.l - from.l) * amount,
        a: from.a + (to.a - from.a) * amount,
    }
}
const TABS_ELASTIC: Duration = Duration::from_millis(420);

/// Selection and indicator animation for `tabs`. Interrupting a transition continues from
/// wherever the indicator currently is.
pub struct TabsState {
    pub selected: usize,
    from: f32,
    started: Instant,
    drag: Rc<Cell<Option<usize>>>,
    bounds: Rc<Cell<Bounds<Pixels>>>,
}

impl TabsState {
    pub fn new(selected: usize) -> Self {
        Self {
            selected,
            from: selected as f32,
            started: Instant::now() - TABS_TRANSITION,
            drag: Rc::new(Cell::new(None)),
            bounds: Rc::new(Cell::new(Bounds::default())),
        }
    }

    /// The indicator's left and right edges, in tabs. They travel separately, which is what
    /// gives it its stretch.
    fn edges(&self, count: usize) -> (f32, f32) {
        let seconds = self.started.elapsed().as_secs_f32();
        if seconds >= TABS_TRANSITION.as_secs_f32() {
            return (self.selected as f32, self.selected as f32 + 1.);
        }
        let distance = self.selected as f32 - self.from;
        let (lead, trail) = (tabs_spring(seconds), tabs_spring(seconds - TABS_TRAILING_DELAY));
        let (left, right) = if distance >= 0. { (trail, lead) } else { (lead, trail) };
        let last = count.max(1) as f32;
        let left = (self.from + distance * left).clamp(0., last - 1.);
        let right = (self.from + 1. + distance * right).clamp(left + 0.5, last);
        (left, right)
    }

    fn position(&self) -> f32 {
        let t = (self.started.elapsed().as_secs_f32() / TABS_TRANSITION.as_secs_f32()).clamp(0., 1.);
        let eased = 1. - (1. - t).powi(3);
        self.from + (self.selected as f32 - self.from) * eased
    }

    pub fn select(&mut self, index: usize) {
        self.from = self.position();
        self.selected = index;
        self.started = Instant::now();
    }
}

/// Segmented tabs with a sliding indicator. Segments have a fixed width, so the bar never
/// resizes.
pub fn tabs(
    id: &'static str,
    labels: Vec<SharedString>,
    state: &TabsState,
    item_width: f32,
    window: &mut Window,
    on_select: Rc<dyn Fn(&usize, &mut Window, &mut App)>,
) -> Div {
    tabs_with(false, id, labels, state, item_width, window, on_select)
}

/// The same tabs, made of glass: the bar, with a lighter slab sliding over it.
pub fn glass_tabs(
    id: &'static str,
    labels: Vec<SharedString>,
    state: &TabsState,
    item_width: f32,
    window: &mut Window,
    on_select: Rc<dyn Fn(&usize, &mut Window, &mut App)>,
) -> Div {
    tabs_with(true, id, labels, state, item_width, window, on_select)
}

fn tabs_with(
    glass: bool,
    id: &'static str,
    labels: Vec<SharedString>,
    state: &TabsState,
    item_width: f32,
    window: &mut Window,
    on_select: Rc<dyn Fn(&usize, &mut Window, &mut App)>,
) -> Div {
    let glass = glass || glass_ui();
    if state.started.elapsed() < TABS_ELASTIC.max(TABS_TRANSITION) {
        window.request_animation_frame();
    }
    let selected = state.selected;
    let position = state.position();
    let (left_edge, right_edge) = state.edges(labels.len());
    let held = state.drag.get().is_some();
    let t = (state.started.elapsed().as_secs_f32() / TABS_ELASTIC.as_secs_f32()).clamp(0., 1.);
    let pulse = (std::f32::consts::PI * t).sin().powi(2);
    let edge = if selected == 0 { -1. } else if selected + 1 == labels.len() { 1. } else { 0. };
    let stretch = edge * 4. * pulse;
    let lift = 0.; // Elasticity is horizontal; the tab hit targets never change height.
    let width = item_width * labels.len() as f32 + 8.;
    let mut bar = div().relative()
        .child(if glass { glass_look(CONTROL_HEIGHT / 2. + lift / 2., GlassLook::WELL) } else { material_surface(MATERIAL_SUBTLE, CONTROL_HEIGHT / 2. + lift / 2., StrokeKind::Neutral) }
            .absolute().left(px(stretch.min(0.))).top(px(-lift / 2.))
            .w(px(width + stretch.abs())).h(px(CONTROL_HEIGHT + lift)))
        .w(px(item_width * labels.len() as f32 + 8.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .p(px(4.))
        .child(
            if glass {
                glass_look(INNER_HEIGHT / 2. + lift / 2., GlassLook::SLAB)
            } else {
                material_surface(0x34343a, INNER_HEIGHT / 2. + lift / 2., StrokeKind::Neutral)
            }
                .absolute()
                // Pressed, it squeezes in slightly, under the finger.
                .top(px(if held { 5. } else { 4. - lift / 2. }))
                .left(px(4. + left_edge * item_width + stretch.min(0.) + if held { 1.5 } else { 0. }))
                .w(px((right_edge - left_edge) * item_width + stretch.abs() - if held { 3. } else { 0. }))
                .h(px(if held { INNER_HEIGHT - 2. } else { INNER_HEIGHT + lift })),
        );
    let count = labels.len();
    for (index, name) in labels.into_iter().enumerate() {
        bar = bar.child(
            div()
                .id((id, index))
                .cursor_pointer()
                .w(px(item_width))
                .flex_shrink_0()
                .h(px(INNER_HEIGHT))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                // Brightens as the indicator arrives, and dims as it leaves.
                .text_color(mix_colour(
                    tab_text(false),
                    tab_text(true),
                    1. - (position - index as f32).abs(),
                ))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_size(px(14.))
                .when(selected != index, |tab| tab.hover(|s| s.bg(rgba(0xffffff0d))))
                .capture_any_mouse_down(press_sound)
                .child(name),
        );
    }
    let bounds = state.bounds.clone();
    let drag = state.drag.clone();
    let down_bounds = state.bounds.clone();
    let down_drag = state.drag.clone();
    let down_select = on_select.clone();
    // On the bar itself rather than window-wide, so anything covering the tabs keeps the click.
    bar = bar.on_mouse_down(MouseButton::Left, move |event, window, cx| {
        if count == 0 { return; }
        let x = f32::from(event.position.x - down_bounds.get().origin.x) - 4.;
        let index = ((x / item_width).floor().max(0.) as usize).min(count - 1);
        down_drag.set(Some(index));
        down_select(&index, window, cx);
    });
    bar.child(canvas(move |rect, _, _| bounds.set(rect), {
        let bounds = state.bounds.clone();
        move |_, _, window, _| {
            let move_drag = drag.clone();
            window.on_mouse_event(move |event: &gpui::MouseMoveEvent, phase, window, cx| {
                if phase != gpui::DispatchPhase::Capture || move_drag.get().is_none() || count == 0 { return; }
                // The release can be missed if it landed while the tabs weren't on screen.
                if event.pressed_button != Some(MouseButton::Left) {
                    move_drag.set(None);
                    return;
                }
                let x = f32::from(event.position.x - bounds.get().origin.x) - 4.;
                let index = ((x / item_width).floor().max(0.) as usize).min(count - 1);
                if move_drag.get() != Some(index) {
                    move_drag.set(Some(index));
                    on_select(&index, window, cx);
                }
            });
            window.on_mouse_event(move |_: &gpui::MouseUpEvent, phase, window, _| {
                if phase == gpui::DispatchPhase::Capture && drag.get().is_some() {
                    drag.set(None);
                    window.refresh();
                }
            });
        }
    }).absolute().size_full())
}

// ---------------------------------------------------------------------------
// Slider
// ---------------------------------------------------------------------------

/// Per-slider state owned by the view: layout bounds and keyboard focus.
#[derive(Clone)]
pub struct SliderHandle {
    pub bounds: Rc<Cell<Bounds<Pixels>>>,
    pub focus: FocusHandle,
    pub motion: Rc<Cell<SliderMotion>>,
}

/// The slider's press and stretch animation.
#[derive(Clone, Copy, Default)]
pub struct SliderMotion {
    pressed: bool,
    /// Current press amount when the direction last changed (quick clicks stay continuous).
    press_from: f32,
    /// When `pressed` last changed.
    changed: Option<Instant>,
    /// How far the track is stretched while dragging past an end. Negative is left.
    stretch: f32,
    /// Where the handle is drawn, gliding towards `target` rather than jumping to it.
    shown: Option<f32>,
    target: f32,
    hovered: bool,
    /// 0 to 1, following `hovered`.
    hover: f32,
    last_frame: Option<Instant>,
}

const SLIDER_PRESS_MS: f32 = 180.;
const SLIDER_SPRING_MS: f32 = 340.;
const SLIDER_MAX_STRETCH: f32 = 7.;
/// How quickly the handle catches up. Tighter under the finger, so a drag never feels loose.
const SLIDER_GLIDE_SECONDS: f32 = 0.055;
const SLIDER_GLIDE_HELD_SECONDS: f32 = 0.018;
const SLIDER_HOVER_SECONDS: f32 = 0.07;

#[cfg(test)]
mod slider_motion_tests {
    use super::*;

    #[test]
    fn the_handle_glides_to_its_value_and_then_rests() {
        let mut motion = SliderMotion::default();
        motion.advance(0.2);
        assert_eq!(motion.shown, Some(0.2), "the first frame starts in place");
        motion.last_frame = Some(Instant::now() - Duration::from_millis(16));
        motion.advance(0.8);
        let part_way = motion.shown.unwrap();
        assert!(part_way > 0.2 && part_way < 0.8, "{part_way}");
        assert!(!motion.settled());
        for _ in 0..200 {
            motion.last_frame = Some(Instant::now() - Duration::from_millis(16));
            motion.advance(0.8);
        }
        assert_eq!(motion.shown, Some(0.8));
        assert!(motion.settled(), "nothing should keep asking for frames at rest");
    }

    #[test]
    fn reversing_a_partial_press_keeps_its_current_scale() {
        let mut motion = SliderMotion {
            pressed: true,
            changed: Some(Instant::now() - Duration::from_millis(35)),
            ..Default::default()
        };
        let held = motion.press();
        assert!(held > 0. && held < 1.);
        motion.press_from = held;
        motion.pressed = false;
        motion.changed = Some(Instant::now());
        assert!((motion.press() - held).abs() < 0.01);
        motion.changed = Some(Instant::now() - Duration::from_millis(200));
        assert_eq!(motion.press(), 0.);
    }

    #[test]
    fn elastic_release_returns_without_crossing_the_endpoint() {
        let mut motion = SliderMotion { stretch: 7., ..Default::default() };
        let mut previous = 7.;
        for ms in [0, 40, 80, 160, 240, 340] {
            motion.changed = Some(Instant::now() - Duration::from_millis(ms));
            let current = motion.current_stretch();
            assert!(current >= 0. && current <= previous);
            previous = current;
        }
        assert_eq!(previous, 0.);
        assert!(rubber_band(1000.) <= SLIDER_MAX_STRETCH);
        assert_eq!(rubber_band(-50.), -rubber_band(50.));
    }
}

impl SliderMotion {
    /// Moves the drawn handle and hover along for this frame.
    fn advance(&mut self, target: f32) {
        let now = Instant::now();
        let dt = self.last_frame.map_or(0., |last| now.duration_since(last).as_secs_f32()).min(0.05);
        self.last_frame = Some(now);
        self.target = target;
        let glide = if self.pressed { SLIDER_GLIDE_HELD_SECONDS } else { SLIDER_GLIDE_SECONDS };
        let shown = self.shown.unwrap_or(target);
        let moved = shown + (target - shown) * (1. - (-dt / glide).exp());
        self.shown = Some(if (target - moved).abs() < 0.0008 { target } else { moved });
        let goal = if self.hovered { 1. } else { 0. };
        let hover = self.hover + (goal - self.hover) * (1. - (-dt / SLIDER_HOVER_SECONDS).exp());
        self.hover = if (goal - hover).abs() < 0.01 { goal } else { hover };
    }

    fn settled(&self) -> bool {
        self.shown == Some(self.target) && self.hover == if self.hovered { 1. } else { 0. }
    }

    fn elapsed_ms(&self) -> f32 {
        self.changed
            .map_or(f32::MAX, |changed| changed.elapsed().as_secs_f32() * 1000.)
    }

    /// 0 at rest, 1 fully pressed.
    fn press(&self) -> f32 {
        let progress = (self.elapsed_ms() / SLIDER_PRESS_MS).clamp(0., 1.);
        let eased = 1. - (1. - progress).powi(3);
        let target = if self.pressed { 1. } else { 0. };
        self.press_from + (target - self.press_from) * eased
    }

    /// The stretch right now: follows the drag, then springs back with a small settle.
    fn current_stretch(&self) -> f32 {
        if self.pressed {
            return self.stretch;
        }
        let time = self.elapsed_ms() / 1000.;
        if time * 1000. >= SLIDER_SPRING_MS {
            return 0.;
        }
        self.stretch * ((1. + time * 24.) * (-time * 24.).exp()).clamp(0., 1.)
    }

    fn animating(&self) -> bool {
        let elapsed = self.elapsed_ms();
        elapsed < SLIDER_PRESS_MS || (!self.pressed && self.stretch != 0. && elapsed < SLIDER_SPRING_MS)
    }
}

/// Dragging past an end gives way less and less, up to a small limit.
fn rubber_band(excess: f32) -> f32 {
    excess.signum() * SLIDER_MAX_STRETCH * (1. - (-excess.abs() / 70.).exp())
}

fn slider_snap(position: f32, bounds: Bounds<Pixels>, max: u16, step: u16) -> Option<u16> {
    let width = f32::from(bounds.size.width);
    if width <= 24. {
        return None;
    }
    let relative = position - f32::from(bounds.origin.x);
    let ratio = ((relative - 12.) / (width - 24.)).clamp(0., 1.);
    let steps = (ratio * max as f32 / step as f32).round() as u16;
    Some((steps * step).min(max))
}

/// The Nitro slider. `step > 1` draws a point for every selectable value.
pub fn slider(
    slot: usize,
    handle: &SliderHandle,
    value: u16,
    max: u16,
    step: u16,
    width: f32,
    on_change: Rc<dyn Fn(&u16, &mut Window, &mut App)>,
) -> Stateful<Div> {
    slider_with(false, slot, handle, value, max, step, width, on_change)
}

/// The same slider, made of glass.
pub fn glass_slider(
    slot: usize,
    handle: &SliderHandle,
    value: u16,
    max: u16,
    step: u16,
    width: f32,
    on_change: Rc<dyn Fn(&u16, &mut Window, &mut App)>,
) -> Stateful<Div> {
    slider_with(true, slot, handle, value, max, step, width, on_change)
}

#[allow(clippy::too_many_arguments)]
fn slider_with(
    glass: bool,
    slot: usize,
    handle: &SliderHandle,
    value: u16,
    max: u16,
    step: u16,
    width: f32,
    on_change: Rc<dyn Fn(&u16, &mut Window, &mut App)>,
) -> Stateful<Div> {
    let glass = glass || glass_ui();
    let inner = width - 24.;
    let target = (value as f32 / max as f32).clamp(0., 1.);
    let mut motion = handle.motion.get();
    motion.advance(target);
    handle.motion.set(motion);
    let ratio = motion.shown.unwrap_or(target);
    let press = motion.press();
    let stretch = motion.current_stretch();
    let stretch_left = (-stretch).max(0.);
    let stretch_right = stretch.max(0.);
    let thumb_width = 28. + 5. * press + 2. * motion.hover;
    let thumb_height = 20. + 4. * press + 1.5 * motion.hover;
    let thumb_radius = thumb_height / 2.;
    let hover_motion = handle.motion.clone();
    let motion_cell = handle.motion.clone();
    let press_motion = handle.motion.clone();
    let move_motion = handle.motion.clone();
    let bounds = handle.bounds.clone();
    let focus = handle.focus.clone();
    let on_press = on_change.clone();
    let press_bounds = bounds.clone();
    let on_move = on_change.clone();
    let move_bounds = bounds.clone();
    let on_key = on_change;
    let mut track = div()
        .id(("slider", slot))
        .track_focus(&handle.focus)
        .relative()
        .w(px(width))
        .max_w_full()
        .h(px(28.))
        .cursor_pointer()
        .on_hover(move |hovered, window, _| {
            let mut current = hover_motion.get();
            current.hovered = *hovered;
            hover_motion.set(current);
            window.refresh();
        })
        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
            cx.stop_propagation();
            window.focus(&focus);
            let previous = press_motion.get();
            press_motion.set(SliderMotion {
                pressed: true,
                press_from: previous.press(),
                changed: Some(Instant::now()),
                stretch: previous.current_stretch(),
                ..previous
            });
            if let Some(next) = slider_snap(f32::from(event.position.x), press_bounds.get(), max, step)
            {
                on_press(&next, window, cx);
            }
        })
        .on_key_down(move |event, window, cx| {
            let next = match event.keystroke.key.as_str() {
                "left" | "down" => value.saturating_sub(step),
                "right" | "up" => value.saturating_add(step).min(max),
                "home" => 0,
                "end" => max,
                _ => return,
            };
            on_key(&next, window, cx);
            cx.stop_propagation();
        })
        .child(
            if glass { glass_look(10., GlassLook::WELL) } else { material_surface(MATERIAL_SUBTLE, 10., StrokeKind::Neutral) }
                .absolute()
                .top(px(4.))
                .left(px(-stretch_left))
                .w(px(width + stretch_left + stretch_right))
                .h(px(20.)),
        )
        .child(
            // On top of the glass, not behind it: a strong, undistorted colour.
            if glass {
                let base: Hsla = rgb(ACCENT).into();
                let vivid = |lightness: f32| Hsla { s: 1., l: lightness, ..base };
                div()
                    .rounded(px(10.))
                    .border_1()
                    .border_color(rgba(0xffffff33))
                    .bg(gpui::linear_gradient(
                        90.,
                        gpui::linear_color_stop(vivid(0.5), 0.),
                        gpui::linear_color_stop(vivid(0.62), 1.),
                    ))
            } else {
                material_surface(ACCENT, 10., StrokeKind::Colored)
            }
                .absolute()
                .top(px(4.))
                .left(px(-stretch_left))
                .w(px(12. + inner * ratio + stretch_left + stretch_right))
                .h(px(20.)),
        );
    if step > 1 {
        let points = (max / step) as usize;
        for n in 0..=points {
            let place = n as f32 / points as f32;
            // A mark swells just as the handle passes it.
            let near = (1. - (place - ratio).abs() * points as f32).clamp(0., 1.);
            let size = 3. + 2. * near;
            track = track.child(
                div()
                    .absolute()
                    .left(px(12. + inner * place - size / 2.))
                    .top(px(14. - size / 2.))
                    .size(px(size))
                    .rounded_full()
                    // Marks for the steps, quiet enough not to compete with the handle.
                    .bg(rgba(if place <= ratio + 0.0001 { 0xffffff59 } else { 0xffffff2e })),
            );
        }
    }
    track
        .child(
            if glass { glass_look(thumb_radius, GlassLook::KNOB) } else { material_surface(ACCENT, thumb_radius, StrokeKind::Colored) }
                .absolute()
                .top(px(14. - thumb_height / 2.))
                .left(px(12. + inner * ratio + stretch - thumb_width / 2.))
                .w(px(thumb_width))
                .h(px(thumb_height))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .w(px((thumb_width - 6.) * (1. - 0.1 * press)))
                        .h(px((thumb_height - 6.) * (1. - 0.1 * press)))
                        .rounded_full()
                        .bg(rgb(0xffffff)),
                ),
        )
        .child(
            canvas(
                move |rect, window, _| {
                    bounds.set(rect);
                    let current = motion_cell.get();
                    if current.animating() || !current.settled() {
                        window.request_animation_frame();
                    }
                },
                {
                    let motion = handle.motion.clone();
                    move |_, _, window, _| {
                        // Track-owned drag payloads stop receiving moves beyond the hitbox.
                        window.on_mouse_event(move |event: &gpui::MouseMoveEvent, phase, window, cx| {
                            let mut current = move_motion.get();
                            if phase != gpui::DispatchPhase::Capture || !current.pressed { return; }
                            // The release can be missed if the slider was closed mid-drag.
                            if event.pressed_button != Some(MouseButton::Left) {
                                current.press_from = current.press();
                                current.pressed = false;
                                current.changed = Some(Instant::now());
                                move_motion.set(current);
                                window.refresh();
                                return;
                            }
                            let area = move_bounds.get();
                            let x = f32::from(event.position.x);
                            let start = f32::from(area.origin.x) + 12.;
                            let end = f32::from(area.origin.x + area.size.width) - 12.;
                            current.stretch = rubber_band(x - x.clamp(start, end));
                            move_motion.set(current);
                            if let Some(next) = slider_snap(x, area, max, step) { on_move(&next, window, cx); }
                            window.refresh();
                        });
                        // Releases can happen far from the slider, so listen window-wide.
                        window.on_mouse_event(move |_: &gpui::MouseUpEvent, phase, window, _| {
                            let mut current = motion.get();
                            if phase == gpui::DispatchPhase::Capture && current.pressed {
                                current.press_from = current.press();
                                current.pressed = false;
                                current.changed = Some(Instant::now());
                                motion.set(current);
                                window.refresh();
                            }
                        });
                    }
                },
            )
            .absolute()
            .size_full(),
        )
}

/// A slider with its label and current value above it.
pub fn slider_field(
    title: impl Into<SharedString>,
    value: impl Into<SharedString>,
    width: f32,
    slider: Stateful<Div>,
) -> Div {
    div()
        .w(px(width))
        .max_w_full()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            div()
                .flex()
                .justify_between()
                .text_size(px(14.))
                .child(title.into())
                .child(value.into()),
        )
        .child(slider)
}

// ---------------------------------------------------------------------------
// Button group
// ---------------------------------------------------------------------------

pub fn button_group(fill: u32, stroke: StrokeKind) -> Div {
    material_surface(fill, 25.01, stroke)
        .flex()
        .items_center()
        .p(px(1.25))
}

/// One segment of a `button_group`. Its hover follows the group's outer corners.
pub fn button_group_segment(
    id: impl Into<gpui::ElementId>,
    index: usize,
    count: usize,
    separator: u32,
) -> Stateful<Div> {
    let mut segment = div()
        .id(id)
        .relative()
        .cursor_pointer()
        .h(px(CONTROL_HEIGHT - 2.5))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .text_size(px(14.))
        .font_weight(gpui::FontWeight::MEDIUM)
        .hover(|s| s.bg(rgba(0xffffff18)))
        .active(|s| s.bg(rgba(0x00000024)))
        .capture_any_mouse_down(press_sound);
    if index == 0 {
        segment = segment.rounded_l(px(18.75));
    }
    if index + 1 == count {
        segment = segment.rounded_r(px(18.75));
    }
    // No separator colour means no separators.
    if index > 0 && separator != 0 {
        segment = segment.child(
            div()
                .absolute()
                .left_0()
                .top(relative(0.5))
                .mt(px(-9.))
                .w(px(1.))
                .h(px(18.))
                .bg(rgb(separator)),
        );
    }
    segment
}

// ---------------------------------------------------------------------------
// Split toggle (call controls)
// ---------------------------------------------------------------------------

/// A toggle with an attached menu chevron. The hover covers the whole pill.
#[allow(clippy::too_many_arguments)]
pub fn split_toggle(
    name: &'static str,
    index: usize,
    icon: &'static str,
    fill: u32,
    stroke: StrokeKind,
    icon_color: u32,
    on_toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    on_menu: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    // The chevron matches the soundboard pill's: 12px, with 11px clear of the edge.
    material_quiet((name, index), format!("{name}-{index}"), fill, 25.01, stroke)
        .w(px(65.))
        .h(px(CONTROL_HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .id(("split-toggle", index))
                .w(px(42.))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(on_toggle)
                .child(svg().path(icon).size(px(20.)).text_color(rgb(icon_color))),
        )
        .child(
            div()
                .id(("split-menu", index))
                .w(px(23.))
                .h_full()
                .flex()
                .items_center()
                .justify_start()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(on_menu)
                .child(
                    svg()
                        .path("CallChevronDown")
                        .size(px(12.))
                        .text_color(rgb(icon_color)),
                ),
        )
}

// ---------------------------------------------------------------------------
// Media grid
// ---------------------------------------------------------------------------

/// One tile of a `media_grid`. `count > 1` means this file was sent that many times.
pub struct MediaTile {
    pub content: Div,
    pub count: usize,
}

const MEDIA_GRID_WIDTH: f32 = 400.;
const MEDIA_GRID_GAP: f32 = 6.;
/// The shape of the cropped thumbnails, so a tile shows one without stretching it.
const MEDIA_TILE_SHAPE: f32 = 555. / 352.;

/// A tile's width: half the grid, or all of it for a last tile that spans the row.
pub fn media_tile_width(count: usize, index: usize) -> f32 {
    if count % 2 == 1 && index + 1 == count { MEDIA_GRID_WIDTH } else { (MEDIA_GRID_WIDTH - MEDIA_GRID_GAP) / 2. }
}

/// A tile's height. With an odd count the last tile spans the row, so it's taller.
pub fn media_tile_height(count: usize, index: usize) -> f32 {
    let half = (MEDIA_GRID_WIDTH - MEDIA_GRID_GAP) / 2.;
    let spans = count % 2 == 1 && index + 1 == count;
    (if spans { MEDIA_GRID_WIDTH } else { half }) / MEDIA_TILE_SHAPE
}

/// A burst of media condensed into one card: up to four tiles, then `+N` on the last.
/// `height_single` is the tile height when only one distinct file is shown.
pub fn media_grid(tiles: Vec<MediaTile>) -> Div {
    let distinct = tiles.len();
    let hidden = distinct.saturating_sub(4);
    let visible: Vec<MediaTile> = tiles.into_iter().take(4).collect();
    let half = (MEDIA_GRID_WIDTH - MEDIA_GRID_GAP) / 2.;
    let visible_count = visible.len();
    let mut grid = div().flex().flex_wrap().gap(px(MEDIA_GRID_GAP)).w(px(MEDIA_GRID_WIDTH));
    for (index, tile) in visible.into_iter().enumerate() {
        // An odd last tile spans the row, so the card always has a clean edge.
        let full = visible_count % 2 == 1 && index + 1 == visible_count;
        let height = media_tile_height(visible_count, index);
        let mut cell = div()
            .relative()
            .w(px(if full { MEDIA_GRID_WIDTH } else { half }))
            .h(px(height))
            .rounded(px(10.75))
            .overflow_hidden()
            .bg(rgb(palette().media))
            .child(tile.content);
        if tile.count > 1 {
            cell = cell.child(
                div()
                    .absolute()
                    .right(px(6.))
                    .bottom(px(6.))
                    .child(badge(format!("×{}", tile.count), MATERIAL_DARK, TEXT)),
            );
        }
        if hidden > 0 && index + 1 == visible_count {
            cell = cell.child(
                div()
                    .absolute()
                    .inset_0()
                    .bg(rgba(0x000000a6))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(22.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(rgb(0xffffff))
                    .child(format!("+{}", hidden + 1)),
            );
        }
        grid = grid.child(cell);
    }
    grid
}

// ---------------------------------------------------------------------------
// Name effects
// ---------------------------------------------------------------------------

/// A display name's animated style, matching the main app's effects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NameEffect {
    Fuzzy,
    Sparkles,
    Resize,
    Bouncy,
    Wavy,
    Gradient,
    Glitch,
    Particle,
}

impl NameEffect {
    pub const ALL: [NameEffect; 8] = [
        NameEffect::Fuzzy,
        NameEffect::Sparkles,
        NameEffect::Resize,
        NameEffect::Bouncy,
        NameEffect::Wavy,
        NameEffect::Gradient,
        NameEffect::Glitch,
        NameEffect::Particle,
    ];

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|effect| effect.label().eq_ignore_ascii_case(value))
    }

    pub fn label(self) -> &'static str {
        match self {
            NameEffect::Fuzzy => "Fuzzy",
            NameEffect::Sparkles => "Sparkles",
            NameEffect::Resize => "Resize",
            NameEffect::Bouncy => "Bouncy",
            NameEffect::Wavy => "Wavy",
            NameEffect::Gradient => "Gradient",
            NameEffect::Glitch => "Glitch",
            NameEffect::Particle => "Particle",
        }
    }
}

static EFFECT_ON_SCREEN: AtomicBool = AtomicBool::new(false);

/// True if an animated name was painted on screen since the last call.
pub fn take_effect_on_screen() -> bool {
    EFFECT_ON_SCREEN.swap(false, Ordering::Relaxed)
}

fn effect_clock() -> f32 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f32()
}

/// Progress through a looping animation, 0 to 1.
fn cycle(period: f32, delay: f32) -> f32 {
    ((effect_clock() - delay) / period).rem_euclid(1.)
}

/// Rises from 0 to 1 by `peak`, and back down to 0 by `end`.
fn pulse(progress: f32, peak: f32, end: f32) -> f32 {
    if progress < peak {
        progress / peak
    } else if progress < end {
        1. - (progress - peak) / (end - peak)
    } else {
        0.
    }
}

fn mix(a: u32, b: u32, amount: f32) -> u32 {
    let channel = |shift: u32| {
        let (x, y) = (((a >> shift) & 0xff) as f32, ((b >> shift) & 0xff) as f32);
        ((x + (y - x) * amount).round() as u32) << shift
    };
    channel(16) | channel(8) | channel(0)
}

fn letters(text: &str) -> Vec<String> {
    text.chars()
        .map(|letter| if letter == ' ' { "\u{a0}".to_string() } else { letter.to_string() })
        .collect()
}

/// Reports when this name is painted inside the window, so it keeps animating only then.
fn effect_marker() -> impl IntoElement {
    canvas(
        |bounds, window, _| {
            let top = f32::from(bounds.origin.y);
            if top < f32::from(window.viewport_size().height) && top + f32::from(bounds.size.height) > 0. {
                EFFECT_ON_SCREEN.store(true, Ordering::Relaxed);
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
}

/// A display name with its effect. Inherits weight and colour, and stays on one line.
pub fn name_text(text: impl Into<SharedString>, effect: Option<NameEffect>, font_size: f32) -> Div {
    let text: SharedString = text.into();
    let line = font_size * 1.3;
    let root = div()
        .relative()
        .flex()
        .flex_shrink_0()
        .whitespace_nowrap()
        .line_height(px(line));
    let Some(effect) = effect else {
        return root.child(text);
    };
    match effect {
        NameEffect::Bouncy | NameEffect::Wavy => {
            let mut row = root.child(effect_marker());
            for (index, letter) in letters(&text).into_iter().enumerate() {
                let lift = if effect == NameEffect::Bouncy {
                    let progress = cycle(1.9, index as f32 * 0.078);
                    pulse(progress, 0.12, 0.24) * 0.24
                } else {
                    let progress = cycle(1.05, index as f32 * 0.09);
                    (0.5 - 0.5 * (progress * std::f32::consts::TAU).cos()) * 0.13
                };
                row = row.child(div().relative().top(px(-lift * font_size)).child(letter));
            }
            row
        }
        NameEffect::Gradient => {
            const STOPS: [u32; 4] = [0xf7d35b, 0xed92ff, 0x78b8ff, 0xf7d35b];
            let all = letters(&text);
            let count = all.len().max(1) as f32;
            let shift = cycle(3.2, 0.);
            let mut row = root.child(effect_marker());
            for (index, letter) in all.into_iter().enumerate() {
                let position = (index as f32 / count / 3. - shift).rem_euclid(1.) * 3.;
                let stop = position.floor() as usize;
                let color = mix(STOPS[stop.min(2)], STOPS[(stop + 1).min(3)], position.fract());
                row = row.child(div().text_color(gpui::rgb(color)).child(letter));
            }
            row
        }
        NameEffect::Glitch => {
            let band = |period: f32, bands: &[(f32, f32, f32)]| {
                let progress = cycle(period, 0.);
                bands
                    .iter()
                    .rev()
                    .find(|(start, _, _)| progress >= *start)
                    .filter(|(start, _, _)| progress < start + 0.05)
                    .map(|(_, top, bottom)| (*top, *bottom))
            };
            let layer = |slice: Option<(f32, f32)>, offset: f32, color: u32, text: SharedString| {
                slice.map(|(top, bottom)| {
                    let start = line * top;
                    div()
                        .absolute()
                        .left(px(offset))
                        .top(px(start))
                        .h(px(line * (1. - top - bottom)))
                        .overflow_hidden()
                        .opacity(0.76)
                        .text_color(gpui::rgb(color))
                        .child(div().relative().top(px(-start)).child(text))
                })
            };
            let first = band(1.65, &[(0.84, 0.14, 0.63), (0.89, 0.60, 0.17), (0.94, 0.33, 0.45)]);
            let second = band(1.2, &[(0.77, 0.49, 0.25), (0.83, 0.08, 0.72), (0.91, 0.68, 0.09)]);
            root.child(effect_marker())
                .child(text.clone())
                .children(layer(first, -1., 0x55dfff, text.clone()))
                .children(layer(second, 1., 0xff78a7, text))
        }
        NameEffect::Fuzzy => {
            // Horizontal strips jittered independently, like the main app's canvas effect.
            let frame = (effect_clock() * 30.) as u32;
            let strips = (line / 2.).ceil() as usize;
            let mut stack = root
                .child(effect_marker())
                .child(div().opacity(0.).child(text.clone()));
            for strip in 0..strips {
                let seed = frame.wrapping_mul(2_654_435_761).wrapping_add(strip as u32 * 40_503);
                let jitter = ((seed >> 13) % 5) as f32 * 0.6 - 1.2;
                let top = strip as f32 * 2.;
                stack = stack.child(
                    div()
                        .absolute()
                        .left(px(jitter))
                        .top(px(top))
                        .h(px(2.))
                        .overflow_hidden()
                        .child(div().relative().top(px(-top)).child(text.clone())),
                );
            }
            stack
        }
        NameEffect::Resize => {
            let handle = || {
                div()
                    .absolute()
                    .size(px(4.))
                    .rounded(px(1.))
                    .border_1()
                    .border_color(gpui::rgb(0x9bc8ff))
                    .bg(gpui::rgb(0x182433))
            };
            root.px(px(3.))
                .border_1()
                .rounded(px(3.))
                .border_color(rgba(0x74b2ffb8))
                .child(text)
                .child(handle().top(px(-3.)).left(px(-3.)))
                .child(handle().top(px(-3.)).right(px(-3.)))
                .child(handle().bottom(px(-3.)).left(px(-3.)))
                .child(handle().bottom(px(-3.)).right(px(-3.)))
        }
        NameEffect::Sparkles => {
            let mut stack = root.child(effect_marker()).child(text);
            for index in 0..6 {
                let progress = cycle(1.85, index as f32 * 0.37);
                let strength = pulse(progress, 0.45, 1.);
                let size = (5. + (index % 3) as f32 * 1.5) * (0.25 + 0.75 * strength);
                stack = stack.child(
                    div()
                        .absolute()
                        .left(relative((8 + (index * 31) % 82) as f32 / 100.))
                        .top(relative((4 + (index * 47) % 91) as f32 / 100.))
                        .ml(px(-size / 2.))
                        .mt(px(-size / 2.))
                        .opacity(strength)
                        .child(
                            svg()
                                .path("Sparkle")
                                .size(px(size))
                                .text_color(gpui::rgb(0xd9c5ff))
                                .with_transformation(Transformation::rotate(percentage(
                                    strength * 0.25,
                                ))),
                        ),
                );
            }
            stack
        }
        NameEffect::Particle => {
            let mut stack = root.child(effect_marker()).child(text);
            for index in 0..9i32 {
                let progress = cycle(1.45, index as f32 * 0.19);
                let strength = pulse(progress, 0.5, 1.);
                stack = stack.child(
                    div()
                        .absolute()
                        .left(relative(((index * 37) % 110 - 5) as f32 / 100.))
                        .top(relative(((index * 53) % 120 - 10) as f32 / 100.))
                        .ml(px(3. * strength))
                        .mt(px(-5. * strength))
                        .size(px(1. + strength))
                        .rounded_full()
                        .opacity(0.9 * strength)
                        .bg(gpui::rgb(0xa78bfa)),
                );
            }
            stack
        }
    }
}

// ---------------------------------------------------------------------------
// System marker
// ---------------------------------------------------------------------------

/// A centred divider for things the app says rather than a person: dates and call events.
/// Who's typing, above the composer. The dots animate on the shared 20 fps tick.
pub fn typing_indicator(
    name: impl Into<SharedString>,
    initials: impl Into<SharedString>,
    color: u32,
    art: impl Into<AvatarImage>,
) -> Div {
    EFFECT_ON_SCREEN.store(true, Ordering::Relaxed);
    let clock = effect_clock();
    let mut dots = div().flex().items_center().gap(px(2.)).ml(px(1.));
    for index in 0..3 {
        // Each dot brightens in turn, a third of a 1.2 second cycle apart.
        let phase = (clock / 1.2 - index as f32 / 3.).rem_euclid(1.);
        let brightness = (phase * std::f32::consts::TAU).sin().max(0.);
        dots = dots.child(
            div()
                .size(px(3.))
                .rounded_full()
                .bg(rgb(0xa1a1aa))
                .opacity(0.35 + 0.65 * brightness),
        );
    }
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .h(px(24.))
        .child(avatar(initials, 20., color, 0xffffff, art))
        .child(
            div()
                .text_size(px(12.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(rgb(0xf2f2f3))
                .child(name.into()),
        )
        .child(div().text_size(px(12.)).text_color(rgb(0xa1a1aa)).child("is typing"))
        .child(dots)
}

pub fn system_marker(text: impl Into<SharedString>) -> Div {
    let text = text.into();
    div()
        .w_full()
        .my(px(10.))
        .flex()
        .items_center()
        .gap(px(12.))
        .child(div().flex_1().h(px(1.)).bg(rgba(0xffffff12)))
        .child(
            material_surface(MATERIAL_SUBTLE, 999., StrokeKind::Neutral)
                .px(px(12.))
                .py(px(4.))
                .text_size(px(11.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(rgb(0xa1a1aa))
                .child(text),
        )
        .child(div().flex_1().h(px(1.)).bg(rgba(0xffffff12)))
}

// ---------------------------------------------------------------------------
// Call pill
// ---------------------------------------------------------------------------

/// Call length as m:ss, or h:mm:ss from an hour on.
pub fn call_duration(seconds: u64) -> String {
    if seconds >= 3600 {
        format!("{}:{:02}:{:02}", seconds / 3600, seconds / 60 % 60, seconds % 60)
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}

/// Text whose characters sit in fixed cells, so changing digits never shift the layout.
fn tabular_text(text: &str, size: f32) -> Div {
    let mut row = div().flex().items_center().text_size(px(size));
    for character in text.chars() {
        let width = if character.is_ascii_digit() { size * 0.66 } else { size * 0.36 };
        row = row.child(
            div()
                .w(px(width))
                .flex()
                .justify_center()
                .child(character.to_string()),
        );
    }
    row
}

/// People in a call, overlapping slightly.
pub fn avatar_stack(people: Vec<(SharedString, u32, AvatarImage)>) -> Div {
    let mut stack = div().flex().flex_shrink_0().items_center();
    for (index, (initials, color, image)) in people.into_iter().enumerate() {
        stack = stack.child(
            div()
                .when(index > 0, |d| d.ml(px(-6.)))
                .child(avatar(initials, 22., color, 0xffffff, image)),
        );
    }
    stack
}

/// The call button outside a call: "Voice" to start one, or who's in and "Join".
pub fn voice_button(
    id: &'static str,
    label: &'static str,
    people: Vec<(SharedString, u32, AvatarImage)>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    // Someone's already in there, so it's an invitation.
    let joinable = !people.is_empty();
    let ink = if joinable { 0xffffff } else { 0xd4d4d8 };
    material_quiet(
        id,
        id,
        if joinable { MATERIAL_GREEN } else { MATERIAL_SUBTLE },
        25.01,
        if joinable { StrokeKind::Colored } else { StrokeKind::Neutral },
    )
        .h(px(CONTROL_HEIGHT))
        .px(px(13.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(px(7.))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(on_click)
        .map(|button| {
            if people.is_empty() {
                button.child(svg().path("CallPhone").size(px(18.)).text_color(rgb(ink)))
            } else {
                button.child(avatar_stack(people))
            }
        })
        .child(
            div()
                .text_size(px(14.))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(rgb(ink))
                .child(label),
        )
}

/// The in-call pill: people pinned left and the time centred in the rest. The pill never
/// changes size. Hovering turns it red and offers Leave in the same slot.
pub fn call_pill(
    id: &'static str,
    people: Vec<(SharedString, u32, AvatarImage)>,
    seconds: u64,
    hovered: bool,
    on_hover: impl Fn(&bool, &mut Window, &mut App) + 'static,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let stack = avatar_stack(people).pl(px(8.));
    material_surface(
        if hovered { MATERIAL_RED } else { MATERIAL_GREEN },
        25.01,
        StrokeKind::Colored,
    )
    .id(id)
    .cursor_pointer()
    .w(px(136.))
    .h(px(CONTROL_HEIGHT))
    .flex_shrink_0()
    .flex()
    .items_center()
    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
    .on_hover(on_hover)
    .on_click(on_click)
    .child(stack)
    .child(
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(rgb(0xffffff))
            .map(|slot| {
                if hovered {
                    slot.text_size(px(14.)).child("Leave")
                } else {
                    slot.child(tabular_text(&call_duration(seconds), 14.))
                }
            }),
    )
}

// ---------------------------------------------------------------------------
// Top banner
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BannerSeverity {
    Neutral,
    Warning,
    Danger,
}

impl BannerSeverity {
    pub fn parse(value: &str) -> Self {
        match value {
            "warning" => BannerSeverity::Warning,
            "danger" => BannerSeverity::Danger,
            _ => BannerSeverity::Neutral,
        }
    }
}

pub const TOP_BANNER_HEIGHT: f32 = 60.;

/// The app-wide banner across the top of the window, for notices and errors. `progress`
/// (0 to 1) fills behind the content, as during an update download.
const BANNER_WARNING: u32 = 0x8a6200;
const BANNER_DANGER: u32 = 0xc4262e;

pub fn top_banner(
    severity: BannerSeverity,
    message: impl Into<SharedString>,
    progress: Option<f32>,
    actions: Vec<Stateful<Div>>,
) -> Div {
    let surface = if crate::state::active_is_default() { 0x191919 } else { palette().panel };
    // Warnings and errors should stop you on any theme, so they don't mix with its colour.
    let (fill, accent, icon) = match severity {
        BannerSeverity::Neutral => (mix(surface, 0xffffff, 0.05), palette().muted, "HeroInfoIcon"),
        BannerSeverity::Warning => (BANNER_WARNING, 0xffdf7a, "HeroWarningIcon"),
        BannerSeverity::Danger => (BANNER_DANGER, 0xffffff, "HeroDangerIcon"),
    };
    div()
        .relative()
        .w_full()
        .h(px(TOP_BANNER_HEIGHT))
        .px(px(30.))
        .flex()
        .items_center()
        .gap(px(14.))
        .bg(gpui::rgb(fill))
        .when_some(progress, |banner, progress| {
            banner.child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .bottom_0()
                    .w(relative(progress.clamp(0., 1.)))
                    .bg(rgba(0xffffff14)),
            )
        })
        .child(
            svg()
                .path(icon)
                .flex_shrink_0()
                .size(px(18.))
                .text_color(gpui::rgb(accent)),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(13.5))
                .text_color(rgb(0xf2f2f3))
                .child(message.into()),
        )
        .children(actions)
}

// ---------------------------------------------------------------------------
// Call overlay
// ---------------------------------------------------------------------------

/// A live screen in the overlay, under whoever it belongs to.
#[derive(Clone)]
pub struct OverlayStream {
    pub owner: String,
    pub frame: std::sync::Arc<gpui::RenderImage>,
}

/// Paints a video frame whole and centred, on black, as a chat panel.
pub fn video_frame(frame: std::sync::Arc<gpui::RenderImage>, radius: f32) -> Div {
    painted_frame(frame, radius, false)
}

fn painted_frame(frame: std::sync::Arc<gpui::RenderImage>, radius: f32, fill: bool) -> Div {
    div()
        .size_full()
        .rounded(px(radius))
        .overflow_hidden()
        .bg(gpui::rgb(0x000000))
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    let size = frame.size(0);
                    let (across, down) = (
                        f32::from(bounds.size.width) / size.width.0 as f32,
                        f32::from(bounds.size.height) / size.height.0 as f32,
                    );
                    let scale = if fill { across.max(down) } else { across.min(down) };
                    let shown = gpui::size(px(size.width.0 as f32 * scale), px(size.height.0 as f32 * scale));
                    let fitted = Bounds::new(
                        gpui::point(
                            bounds.origin.x + (bounds.size.width - shown.width) / 2.,
                            bounds.origin.y + (bounds.size.height - shown.height) / 2.,
                        ),
                        shown,
                    );
                    let _ = window.paint_image(fitted, px(radius).into(), frame, 0, false);
                },
            )
            .size_full(),
        )
}

#[derive(Clone, PartialEq)]
pub struct OverlayPerson {
    pub id: String,
    pub name: SharedString,
    pub initials: SharedString,
    pub color: u32,
    pub art: AvatarImage,
    pub muted: bool,
    /// Voice level, 0 to 1. Drives the speaking ring.
    pub level: f32,
}

// GPUI's shadow shader extends to 3 * blur_radius. Reserve the maximum, plus
// speaking growth and native pixel rounding/insets at every overlay scale.
pub const OVERLAY_PADDING: f32 = 56.;
const OVERLAY_STREAM_WIDTH: f32 = 320.;
const OVERLAY_STREAM_HEIGHT: f32 = 180.;

/// The stream tile in pixels: always 16:9, at most the full tile, and never larger than the
/// picture, so a small window stays small instead of being stretched.
pub fn overlay_stream_size(source: (u32, u32), scale: f32) -> (f32, f32) {
    let scale = scale.clamp(0.5, 1.5);
    let width = (source.0 as f32).min(OVERLAY_STREAM_WIDTH * scale);
    let height = (source.1 as f32).min(OVERLAY_STREAM_HEIGHT * scale);
    if width * 9. > height * 16. { (height * 16. / 9., height) } else { (width, width * 9. / 16.) }
}

pub fn overlay_size(count: usize, scale: f32, show_names: bool, stream: Option<(u32, u32)>) -> gpui::Size<Pixels> {
    let tile = stream.map(|source| overlay_stream_size(source, scale));
    let scale = scale.clamp(0.5, 1.5);
    let count = count.min(2).max(1) as f32;
    let row = (40. + if show_names { 150. } else { 0. }) * scale;
    let people = (count * 40. + (count - 1.) * 12.) * scale;
    let padding = 2. * OVERLAY_PADDING * scale;
    match tile {
        Some((width, height)) => gpui::size(px(padding + row.max(width)), px(padding + people + 10. * scale + height)),
        None => gpui::size(px(padding + row), px(padding + people)),
    }
}

/// The people in a call as a compact column, with rings that follow their voices.
pub fn call_overlay(
    people: &[OverlayPerson],
    scale: f32,
    show_names: bool,
    stream: Option<&OverlayStream>,
) -> Div {
    let unit = |value: f32| px(value * scale.clamp(0.5, 1.5));
    let mut column = div().flex().flex_col().gap(unit(12.)).p(unit(OVERLAY_PADDING));
    for person in people.iter().take(2) {
        let level = if person.muted { 0. } else { person.level.clamp(0., 1.) };
        let fade = (level / 0.08).clamp(0., 1.);
        let activity = fade * fade * (3. - 2. * fade);
        let ring: Hsla = if person.muted {
            gpui::rgb(0xff3b3b).into()
        } else {
            let mut green: Hsla = gpui::rgb(0x00d084).into();
            green.a = activity * (0.22 + level * 0.5);
            green
        };
        let mut glow: Hsla = gpui::rgb(0x00d084).into();
        glow.a = activity * (0.1 + level * 0.25);
        let size = 40. * scale.clamp(0.5, 1.5);
        let animated_size = size * (1. + level * 0.04);
        let blur = unit(8. + 4. * level);
        let outset = unit(1. + 0.5 * level);
        let art = person.art.clone();
        let mut fallback = avatar(
            person.initials.clone(), animated_size, person.color, 0xffffff, AvatarImage::default(),
        ).into_any_element();
        column = column.child(
            div()
                .id(SharedString::from(format!("overlay-person-{}", person.id)))
                .flex()
                .items_center()
                .gap(unit(10.))
                .child(
                    canvas(move |slot, window, cx| {
                        let dpi = window.scale_factor();
                        let base_pixels = (size * dpi).round().max(1.);
                        let centre = gpui::point(
                            px(((f32::from(slot.origin.x + slot.size.width / 2.) * dpi - base_pixels / 2.).round() + base_pixels / 2.) / dpi),
                            px(((f32::from(slot.origin.y + slot.size.height / 2.) * dpi - base_pixels / 2.).round() + base_pixels / 2.) / dpi),
                        );
                        let desired = animated_size * dpi;
                        let low_pixels = base_pixels + ((desired - base_pixels).max(0.) / 2.).floor() * 2.;
                        let blend = ((desired - low_pixels) / 2.).clamp(0., 1.);
                        let bounds = Bounds::new(gpui::point(centre.x - px(animated_size / 2.), centre.y - px(animated_size / 2.)),
                            gpui::size(px(animated_size), px(animated_size)));
                        let load = |path: &PathBuf, window: &mut Window, cx: &mut App| {
                            window.use_asset::<gpui::ImgResourceLoader>(&path.clone().into(), cx).and_then(Result::ok)
                        };
                        let photo = art.path.as_ref().and_then(|path| load(path, window, cx));
                        let decoration = art.decoration.as_ref().and_then(|path| match art_animation(path) {
                            ArtAnimation::Frames(frames) => Some(frames),
                            _ => load(path, window, cx),
                        });
                        let mut layers = Vec::new();
                        if let Some(photo) = photo {
                            let frame = art_frame(&photo);
                            let deco_frame = decoration.as_ref().map(|image| art_frame(image));
                            // Decorations repaint from `sync_overlay`'s tick.
                            if photo.frame_count() > 1 {
                                window.request_animation_frame();
                            }
                            for (pixels, opacity) in [(low_pixels, 1.), (low_pixels + 2., blend)] {
                                let mut layer = overlay_raster_layer(size, centre, pixels, dpi, opacity,
                                    photo.clone(), frame, decoration.clone(), deco_frame);
                                layer.layout_as_root(gpui::size(gpui::AvailableSpace::Definite(px(size)), gpui::AvailableSpace::Definite(px(size))), window, cx);
                                layer.prepaint_at(slot.origin, window, cx);
                                layers.push(layer);
                            }
                        } else {
                            let measured = fallback.layout_as_root(gpui::size(gpui::AvailableSpace::Definite(px(size)), gpui::AvailableSpace::Definite(px(size))), window, cx);
                            fallback.prepaint_at(gpui::point(centre.x - measured.width / 2., centre.y - measured.height / 2.), window, cx);
                            layers.push(fallback);
                        }
                        (bounds, layers)
                    }, move |_, (bounds, layers), window, cx| {
                        window.paint_shadows(bounds, (bounds.size.width / 2.).into(), &[
                            gpui::BoxShadow { color: glow, offset: gpui::point(px(0.), px(0.)), blur_radius: blur, spread_radius: px(0.) }
                        ]);
                        let ring_bounds = bounds.dilate(outset);
                        let mut circle = gpui::fill(ring_bounds, ring);
                        circle.corner_radii = (ring_bounds.size.width / 2.).into();
                        window.paint_quad(circle);
                        for mut layer in layers { layer.paint(window, cx); }
                    }).size(px(size)).flex_shrink_0(),
                )
                .when(show_names, |row| {
                    row.child(
                        div()
                            .max_w(unit(140.))
                            .overflow_hidden()
                            .text_size(unit(14.))
                            .text_color(gpui::rgb(0xffffff))
                            .child(person.name.clone()),
                    )
                }),
        );
        if let Some(stream) = stream.filter(|stream| stream.owner == person.id) {
            let source = stream.frame.size(0);
            let (width, height) = overlay_stream_size(
                (source.width.0.max(1) as u32, source.height.0.max(1) as u32),
                scale,
            );
            column = column.child(
                div()
                    .mt(unit(-2.))
                    .w(px(width))
                    .h(px(height))
                    .flex_shrink_0()
                    .child(painted_frame(stream.frame.clone(), 8. * scale.clamp(0.5, 1.5), true)),
            );
        }
    }
    column
}

/// The frame to show now, following the file's own delays.
fn art_frame(image: &gpui::RenderImage) -> usize {
    static START: OnceLock<Instant> = OnceLock::new();
    let elapsed = START.get_or_init(Instant::now).elapsed().as_millis();
    // As browsers do, treat near-zero delays as 100 ms.
    let duration = |index| {
        let delay = Duration::from(image.delay(index)).as_millis();
        if delay < 10 { 100 } else { delay }
    };
    let total: u128 = (0..image.frame_count()).map(duration).sum();
    if total == 0 { return 0; }
    let mut offset = elapsed % total;
    for index in 0..image.frame_count() {
        let delay = duration(index);
        if offset < delay { return index; }
        offset -= delay;
    }
    0
}

/// GPUI paint_image floors origins and ceils sizes. Rasterise two even-step sizes
/// about ONE physical-pixel centre, then blend, instead of moving a fractional
/// image origin through that asymmetric snap. Both sizes have the same parity.
fn overlay_raster_layer(
    slot_size: f32, centre: gpui::Point<Pixels>, pixels: f32, dpi: f32, opacity: f32,
    photo: std::sync::Arc<gpui::RenderImage>, photo_frame: usize,
    decoration: Option<std::sync::Arc<gpui::RenderImage>>, deco_frame: Option<usize>,
) -> gpui::AnyElement {
    div().size(px(slot_size)).opacity(opacity).child(canvas(|_, _, _| (), move |_, _, window, _| {
        let bounds = overlay_raster_bounds(centre, pixels, dpi);
        let _ = window.paint_image(bounds, px(pixels / dpi / 2.).into(), photo, photo_frame, false);
        if let (Some(deco), Some(frame)) = (decoration, deco_frame) {
            // Preserve pixel parity, so the decoration uses the exact same centre.
            let parity = pixels % 2.;
            let deco_pixels = ((pixels * 1.18 - parity) / 2.).round() * 2. + parity;
            let _ = window.paint_image(overlay_raster_bounds(centre, deco_pixels, dpi), px(0.).into(), deco, frame, false);
        }
    }).size_full()).into_any_element()
}

fn overlay_raster_bounds(centre: gpui::Point<Pixels>, diameter: f32, dpi: f32) -> Bounds<Pixels> {
    // Bias just inside an integer rectangle to survive image floor/ceil at Windows DPI.
    let x = (f32::from(centre.x) * dpi - diameter / 2.).round();
    let y = (f32::from(centre.y) * dpi - diameter / 2.).round();
    Bounds::new(gpui::point(px((x + 0.0001) / dpi), px((y + 0.0001) / dpi)),
        gpui::size(px((diameter - 0.0001) / dpi), px((diameter - 0.0001) / dpi)))
}

/// A surface of real glass: it bends and diffuses whatever is painted behind it. Children go on
/// top. `tint` is a colour and how strongly it shows, for a coloured button.
pub fn glass_surface(radius: f32, tint: Option<(u32, f32)>) -> Div {
    glass_look(radius, GlassLook { tint, ..GlassLook::STANDARD })
}

pub fn glass_look(radius: f32, look: GlassLook) -> Div {
    div().relative().rounded(px(radius)).child(
        canvas(|_, _, _| (), move |bounds, _, window, _| paint_look(bounds, radius, look, window))
            .absolute()
            // Pinned to the corner, or the surface's own padding would push the glass over.
            .top_0()
            .left_0()
            .size_full(),
    )
}

fn glass_pill(id: &'static str, height: f32, tint: Option<(u32, f32)>) -> Stateful<Div> {
    glass_surface(height / 2., tint)
        .id(id)
        .h(px(height))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.))
        .cursor_pointer()
        .text_size(px(14.))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(gpui::rgb(0xffffff))
        .hover(|style| style.opacity(0.86))
}

fn glass_samples() -> [Arc<gpui::Image>; 3] {
    static SAMPLES: OnceLock<[Arc<gpui::Image>; 3]> = OnceLock::new();
    SAMPLES
        .get_or_init(|| {
            let image = |bytes: &[u8]| Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Jpeg, bytes.to_vec()));
            [
                image(include_bytes!("../assets/glass-samples/sunset.jpg")),
                image(include_bytes!("../assets/glass-samples/lights.jpg")),
                image(include_bytes!("../assets/glass-samples/detail.jpg")),
            ]
        })
        .clone()
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Snowflake {
    x: f32,
    y: f32,
    size: f32,
    alpha: f32,
}

/// Enough for a light fall at any window size.
fn snowflake_count(width: f32, height: f32) -> u32 {
    ((width * height / 9000.) as u32).clamp(30, 140)
}

/// Where one flake is. Worked out from the time alone, so there's nothing to store or update.
fn snowflake(index: u32, seconds: f64, width: f32, height: f32) -> Snowflake {
    const MARGIN: f64 = 12.;
    let random = |salt: u32| {
        let mut hash = index.wrapping_mul(2654435761) ^ salt.wrapping_mul(2246822519);
        hash ^= hash >> 15;
        hash = hash.wrapping_mul(3266489917);
        hash ^= hash >> 13;
        f64::from(hash) / f64::from(u32::MAX)
    };
    // Nearer flakes are bigger, brighter and fall faster.
    let depth = random(1);
    let speed = 22. + depth * 46.;
    let travel = f64::from(height) + MARGIN * 2.;
    let y = (random(2) * travel + seconds * speed).rem_euclid(travel) - MARGIN;
    let sway = (6. + random(3) * 16.) * (seconds * (0.25 + random(4) * 0.5) + random(5) * std::f64::consts::TAU).sin();
    let x = random(6) * f64::from(width) + sway;
    Snowflake { x: x as f32, y: y as f32, size: 1.6 + depth as f32 * 2.8, alpha: 0.28 + depth as f32 * 0.55 }
}

/// Snow falling over the whole window. It can't be clicked.
pub fn snow_overlay(started: Instant) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let (width, height) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
            let seconds = started.elapsed().as_secs_f64();
            for index in 0..snowflake_count(width, height) {
                let flake = snowflake(index, seconds, width, height);
                let origin = bounds.origin + gpui::point(px(flake.x), px(flake.y));
                window.paint_quad(
                    gpui::fill(Bounds::new(origin, gpui::size(px(flake.size), px(flake.size))), hsla(0., 0., 1., flake.alpha))
                        .corner_radii(px(flake.size / 2.)),
                );
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// The bar behind the tabs, for anything else built from them.
pub fn tab_well(radius: f32) -> Div {
    if glass_ui() { glass_look(radius, GlassLook::WELL) } else { material_surface(MATERIAL_SUBTLE, radius, StrokeKind::Neutral) }
}

/// A hover highlight for something with its own surface, which would cover a plain background.
/// Goes after the surface and before the contents, on an element in the same group.
pub fn hover_sheen(group: &'static str, radius: f32) -> Div {
    div().absolute().top_0().left_0().size_full().rounded(px(radius)).group_hover(group, |s| s.bg(rgba(0xffffff1c)))
}

/// The raised piece that marks the selected tab.
pub fn tab_slab(radius: f32) -> Div {
    if glass_ui() { glass_look(radius, GlassLook::SLAB) } else { material_surface(0x34343a, radius, StrokeKind::Neutral) }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CallTile {
    Idle,
    Selected,
    Alarm,
    Live,
}

/// Runs past the end a little and settles back, for things that should feel springy.
pub fn back_out(t: f32) -> f32 {
    const OVERSHOOT: f32 = 1.9;
    let t = t - 1.;
    t * t * ((OVERSHOOT + 1.) * t + OVERSHOOT) + 1.
}

/// One segment of a call module button group, lit the way a selected tab is.
pub fn call_segment(id: &'static str, index: usize, count: usize, icon: &'static str, width: f32, state: CallTile) -> Stateful<Div> {
    const RADIUS: f32 = 14.;
    let backing = match state {
        CallTile::Idle => None,
        CallTile::Selected => Some(tab_slab(RADIUS)),
        CallTile::Alarm => Some(material_surface(MATERIAL_RED, RADIUS, StrokeKind::Colored)),
        CallTile::Live => Some(material_surface(ACCENT, RADIUS, StrokeKind::Colored)),
    };
    let size = if width < 36. { 12. } else { 18. };
    button_group_segment(id, index, count, 0)
        .h(px(INNER_HEIGHT - 2.5))
        .w(px(width))
        .flex_shrink_0()
        // Pops into place whenever it lights up or changes colour.
        .children(backing.map(|backing| {
            backing.absolute().inset(px(2.)).with_animation(
                (id, state as usize),
                Animation::new(Duration::from_millis(260)),
                |backing, delta| backing.inset(px(2. + (1. - back_out(delta)) * 8.)),
            )
        }))
        .child(svg().relative().path(icon).size(px(size)).text_color(rgb(if state == CallTile::Idle { 0xd4d4d8 } else { 0xffffff })))
}

/// What keeps the window buttons readable with content passing under them: the background colour,
/// solid along the top and gone by `FADE_HEIGHT`, and easing in over `soft` from its left edge.
pub fn window_buttons_fade(colour: u32, width: f32, soft: f32) -> impl IntoElement {
    const FADE_HEIGHT: f32 = 44.;
    const SOLID: f32 = 18.;
    const STRIP: f32 = 4.;
    let base: Hsla = gpui::rgb(colour).into();
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let mut column = |x: f32, column_width: f32, alpha: f32| {
                let colour = Hsla { a: alpha, ..base };
                let origin = bounds.origin + gpui::point(px(x), px(0.));
                window.paint_quad(gpui::fill(
                    Bounds::new(origin, gpui::size(px(column_width), px(SOLID))),
                    colour,
                ));
                window.paint_quad(gpui::fill(
                    Bounds::new(origin + gpui::point(px(0.), px(SOLID)), gpui::size(px(column_width), px(FADE_HEIGHT - SOLID))),
                    gpui::linear_gradient(
                        180.,
                        gpui::linear_color_stop(colour, 0.),
                        gpui::linear_color_stop(Hsla { a: 0., ..base }, 1.),
                    ),
                ));
            };
            let mut x = 0.;
            while x < soft {
                let along = (x + STRIP / 2.) / soft;
                column(x, STRIP, along * along * (3. - 2. * along));
                x += STRIP;
            }
            column(x, width - x, 1.);
        },
    )
    .absolute()
    .top_0()
    .right_0()
    .w(px(width))
    .h(px(FADE_HEIGHT))
}

/// The same set of the app's components, for looking at one material beside the other.
pub fn compare_column(title: &'static str, tabs: Div, volume: Stateful<Div>, stepped: Stateful<Div>) -> Div {
    let button = |id: &'static str, fill: u32, stroke: StrokeKind, text: &'static str| {
        material_interactive(id, id, fill, 19., stroke)
            .h(px(38.))
            .px(px(16.))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(13.5))
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(gpui::rgb(0xffffff))
            .child(text)
    };
    let round = |id: &'static str, icon: &'static str| {
        material_interactive(id, id, MATERIAL_DARK, 19., StrokeKind::Neutral)
            .size(px(38.))
            .flex()
            .items_center()
            .justify_center()
            .child(svg().path(icon).size(px(16.)).text_color(gpui::rgb(0xffffff)))
    };
    div()
        .w(px(392.))
        .flex()
        .flex_col()
        .gap(px(14.))
        .text_color(gpui::rgb(0xffffff))
        .child(div().text_size(px(13.)).font_weight(gpui::FontWeight::SEMIBOLD).child(title))
        .child(
            div().flex().gap(px(8.))
                .child(
                    material_interactive("cmp-partner", "cmp-partner", MATERIAL_DARK, 19., StrokeKind::Neutral)
                        .h(px(38.)).px(px(6.)).pr(px(12.)).flex().items_center().gap(px(8.)).text_size(px(13.5))
                        .child(avatar("N".to_string(), 26., 0x3d4a58, 0xffffff, AvatarImage::from(true)))
                        .child("nyxnie")
                        .child(svg().path("ChevronDown").size(px(12.)).text_color(gpui::rgb(0xffffff))),
                )
                .child(
                    material_interactive("cmp-voice", "cmp-voice", MATERIAL_DARK, 19., StrokeKind::Neutral)
                        .h(px(38.)).px(px(14.)).flex().items_center().gap(px(8.)).text_size(px(13.5))
                        .child(svg().path("CallPhone").size(px(15.)).text_color(gpui::rgb(0xffffff)))
                        .child("Voice"),
                ),
        )
        .child(
            div().flex().gap(px(8.))
                .child(button("cmp-primary", ACCENT, StrokeKind::Colored, "Primary"))
                .child(button("cmp-secondary", MATERIAL_DARK, StrokeKind::Neutral, "Secondary"))
                .child(button("cmp-danger", MATERIAL_RED, StrokeKind::Colored, "Danger")),
        )
        .child(
            div().flex().items_center().gap(px(8.))
                .child(round("cmp-play", "Play"))
                .child(round("cmp-sound", "Volume"))
                .child(round("cmp-react", "SmilePlus"))
                .child(badge("3", ACCENT, 0xffffff))
                .child(badge("New", MATERIAL_DARK, 0xffffff)),
        )
        .child(tabs)
        .child(volume)
        .child(stepped)
        .child(
            material_surface(MATERIAL_DARK, 23., StrokeKind::Neutral)
                .h(px(46.)).px(px(7.)).flex().items_center().gap(px(8.))
                .child(round("cmp-attach", "Paperclip"))
                .child(div().flex_1().text_size(px(14.)).child("Message @nyxnie…"))
                .child(button("cmp-send", ACCENT, StrokeKind::Colored, "Send")),
        )
        .child(
            material_surface(MATERIAL_PANEL, 14., StrokeKind::Neutral)
                .h(px(130.)).p(px(14.)).flex().flex_col().gap(px(4.))
                .child(div().text_size(px(14.)).font_weight(gpui::FontWeight::SEMIBOLD).child("A card or menu"))
                .child(div().text_size(px(12.5)).child("Link previews, menus and media cards are this surface.")),
        )
}

/// A comparison panel: two columns, on the plain background or over a picture.
pub fn compare_panel(picture: bool, left: Stateful<Div>, right: Stateful<Div>) -> Div {
    div()
        .relative()
        .w_full()
        .rounded(px(18.))
        .overflow_hidden()
        .p(px(24.))
        .flex()
        .justify_between()
        .when(picture, |panel| {
            panel.child(
                gpui::img(glass_samples()[0].clone())
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .object_fit(gpui::ObjectFit::Cover),
            )
        })
        .child(left)
        .child(right)
}

/// The composer as glass, with a reply and an attachment above it, as they appear in chat.
fn glass_composer() -> Div {
    let icon = |name: &'static str, size: f32| svg().path(name).size(px(size)).text_color(gpui::rgb(0xffffff));
    div()
        .w(px(600.))
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            glass_surface(14., None).h(px(56.)).p(px(6.)).flex().items_center().gap(px(10.))
                .child(
                    div().size(px(44.)).flex_shrink_0().rounded(px(10.)).overflow_hidden()
                        .child(gpui::img(glass_samples()[0].clone()).size_full().object_fit(gpui::ObjectFit::Cover)),
                )
                .child(
                    div().flex_1().flex().flex_col().text_color(gpui::rgb(0xffffff))
                        .child(div().text_size(px(12.5)).font_weight(gpui::FontWeight::SEMIBOLD).child("sunset.mp4"))
                        .child(div().text_size(px(12.)).child("Video · 0:12 · 8.4 MB")),
                )
                .child(div().px(px(10.)).text_color(gpui::rgb(0xffffff)).child("×")),
        )
        .child(
            glass_surface(23., None).h(px(46.)).px(px(7.)).flex().items_center().gap(px(8.))
                .child(glass_pill("glass-composer-attach", 32., Some((0xffffff, 0.1))).w(px(32.)).child(icon("Paperclip", 16.)))
                .child(div().flex_1().text_size(px(14.5)).text_color(gpui::rgb(0xffffff)).child("Message @nyxnie…"))
                .child(
                    glass_pill("glass-composer-send", 34., Some((ACCENT, 0.5))).px(px(14.)).text_size(px(12.5))
                        .child(icon("Send", 13.))
                        .child("Send"),
                ),
        )
}

/// A test bench for glass: header, buttons, a card, tabs, sliders, a composer and a dock over
/// pictures that scroll. The tabs and sliders come in working.
pub fn glass_showcase(tabs: Div, volume: Div, partner_volume: Div) -> Div {
    let icon = |name: &'static str, size: f32| svg().path(name).size(px(size)).text_color(gpui::rgb(0xffffff));
    let pictures = glass_samples().into_iter().fold(div().flex().flex_col(), |column, picture| {
        column.child(gpui::img(picture).w_full().h(px(354.)).flex_shrink_0().object_fit(gpui::ObjectFit::Cover))
    });
    let circle = |id: &'static str, name: &'static str| {
        glass_pill(id, 44., None).w(px(44.)).child(icon(name, 18.))
    };
    div()
        .relative()
        .w_full()
        .h(px(700.))
        .rounded(px(18.))
        .overflow_hidden()
        .child(div().id("glass-pictures").size_full().overflow_y_scroll().child(pictures))
        // The header's pair, as glass.
        .child(
            div().absolute().top(px(18.)).left_0().right_0().flex().justify_center().gap(px(10.))
                .child(
                    glass_pill("glass-partner", 40., None).px(px(6.)).pr(px(14.))
                        .child(avatar("N".to_string(), 28., 0x3d4a58, 0xffffff, AvatarImage::from(true)))
                        .child("nyxnie")
                        .child(icon("ChevronDown", 12.)),
                )
                .child(glass_pill("glass-voice", 40., None).px(px(16.)).child(icon("CallPhone", 16.)).child("Voice")),
        )
        // Buttons, plain and coloured, and round ones.
        .child(
            div().absolute().top(px(90.)).left(px(28.)).flex().flex_col().gap(px(14.))
                .child(
                    div().flex().gap(px(10.))
                        .child(glass_pill("glass-primary", 40., Some((ACCENT, 0.42))).px(px(20.)).child("Primary"))
                        .child(glass_pill("glass-secondary", 40., None).px(px(20.)).child("Secondary"))
                        .child(glass_pill("glass-danger", 40., Some((0xff3b3b, 0.42))).px(px(20.)).child("Danger")),
                )
                .child(
                    div().flex().gap(px(10.))
                        .child(circle("glass-play", "Play"))
                        .child(circle("glass-sound", "Volume"))
                        .child(circle("glass-react", "SmilePlus"))
                        .child(circle("glass-copy", "Copy")),
                ),
        )
        // A larger panel, where the flat middle shows.
        .child(
            glass_surface(24., None).absolute().top(px(90.)).right(px(28.)).w(px(250.)).h(px(170.)).p(px(20.))
                .flex().flex_col().gap(px(6.)).text_color(gpui::rgb(0xffffff))
                .child(div().text_size(px(16.)).font_weight(gpui::FontWeight::SEMIBOLD).child("A glass card"))
                .child(div().text_size(px(13.)).child("Scroll the pictures to see them bend at the edges and blur through the middle.")),
        )
        // Tabs and volume sliders, which respond.
        .child(div().absolute().top(px(290.)).left(px(28.)).child(tabs))
        .child(
            div().absolute().top(px(280.)).right(px(28.)).w(px(330.)).flex().flex_col().gap(px(14.))
                .text_color(gpui::rgb(0xffffff))
                .child(volume)
                .child(partner_volume),
        )
        .child(div().absolute().bottom(px(92.)).left_0().right_0().flex().justify_center().child(glass_composer()))
        // The dock, at the size the settings were tuned for.
        .child(
            div().absolute().bottom(px(24.)).left_0().right_0().flex().justify_center().child(
                glass_surface(17., None).w(px(760.)).h(px(48.)).flex().items_center().justify_center().gap(px(26.))
                    .child(icon("Pencil", 18.))
                    .child(icon("Reply", 18.))
                    .child(icon("CallPhone", 18.))
                    .child(icon("Volume", 18.))
                    .child(icon("Copy", 18.))
                    .child(icon("Picture", 18.)),
            ),
        )
}

pub const LINK_CARD_WIDTH: f32 = 420.;

pub struct LinkCardView {
    pub site: SharedString,
    pub title: SharedString,
    pub description: SharedString,
    pub image: Option<PathBuf>,
    /// Width over height. A squarish picture goes beside the text instead of above it.
    pub aspect: Option<f32>,
    pub playable: bool,
}

/// A link's preview. The caller makes it clickable.
pub fn link_card(view: LinkCardView) -> Div {
    let wide = view.aspect.is_some_and(|aspect| aspect >= 1.3);
    let picture = |width: f32, height: f32, radius: f32, playable: bool, file: PathBuf| {
        div()
            .relative()
            .flex_shrink_0()
            .w(px(width))
            .h(px(height))
            .overflow_hidden()
            .child(gpui::img(file).size_full().rounded(px(radius)).object_fit(gpui::ObjectFit::Cover))
            .when(playable, |picture| {
                picture.child(
                    div().absolute().inset_0().flex().items_center().justify_center().child(
                        material_surface(MATERIAL_DARK, 22., StrokeKind::Neutral)
                            .size(px(44.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(svg().path("Play").size(px(18.)).text_color(gpui::rgb(0xffffff))),
                    ),
                )
            })
    };
    let words = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(3.))
        .child(div().text_size(px(11.5)).line_height(px(15.)).text_color(gpui::rgb(0x9e9ea8)).truncate().child(view.site))
        .child(
            div()
                .text_size(px(14.))
                .line_height(px(19.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(gpui::rgb(0xf2f2f3))
                .line_clamp(2)
                .child(view.title),
        )
        .when(!view.description.is_empty(), |words| {
            words.child(
                div()
                    .text_size(px(12.5))
                    .line_height(px(17.))
                    .text_color(gpui::rgb(0x9e9ea8))
                    .line_clamp(3)
                    .child(view.description),
            )
        });
    let card = material_surface(MATERIAL_PANEL, 14., StrokeKind::Neutral).w(px(LINK_CARD_WIDTH)).p(px(12.));
    match view.image {
        Some(file) if wide => {
            let width = LINK_CARD_WIDTH - 24.;
            let height = (width / view.aspect.unwrap_or(16. / 9.)).clamp(120., 224.);
            card.flex().flex_col().gap(px(10.)).child(words).child(picture(width, height, 10., view.playable, file))
        }
        Some(file) => card
            .flex()
            .items_start()
            .gap(px(12.))
            .child(words)
            .child(picture(76., 76., 10., view.playable, file)),
        None => card.child(words),
    }
}

#[cfg(test)]
mod tab_motion_tests {
    use super::*;

    #[test]
    fn the_spring_settles_on_target_with_only_a_small_overshoot() {
        assert_eq!(tabs_spring(0.), 0.);
        let most = (0..1000).map(|ms| tabs_spring(ms as f32 / 1000.)).fold(0., f32::max);
        assert!(most > 1. && most < 1.05, "{most}");
        assert!((tabs_spring(TABS_TRANSITION.as_secs_f32()) - 1.).abs() < 0.01);
    }

    #[test]
    fn the_indicator_stays_in_the_bar_and_rests_exactly_on_its_tab() {
        for (from, to) in [(0, 2), (2, 0), (0, 1), (1, 2)] {
            let mut state = TabsState::new(from);
            state.select(to);
            for ms in (0..500).step_by(5) {
                state.started = Instant::now() - Duration::from_millis(ms);
                let (left, right) = state.edges(3);
                assert!(left >= 0. && right <= 3. && right > left, "{from}->{to} at {ms}: {left}..{right}");
            }
            state.started = Instant::now() - TABS_TRANSITION;
            assert_eq!(state.edges(3), (to as f32, to as f32 + 1.));
        }
    }

    #[test]
    fn colours_mix_within_their_range() {
        let (dim, bright) = (tab_text(false), tab_text(true));
        assert_eq!(mix_colour(dim, bright, 1.).l, bright.l);
        assert_eq!(mix_colour(dim, bright, -3.).l, dim.l);
    }
}

#[cfg(test)]
mod overlay_tile_tests {
    use super::overlay_stream_size;

    #[test]
    fn the_tile_is_widescreen_and_only_shrinks_for_small_pictures() {
        assert_eq!(overlay_stream_size((1920, 1080), 1.), (320., 180.));
        assert_eq!(overlay_stream_size((800, 600), 1.), (320., 180.));
        assert_eq!(overlay_stream_size((1920, 1080), 1.5), (480., 270.));
        let (width, height) = overlay_stream_size((200, 100), 1.);
        assert!((width - 177.78).abs() < 0.01 && height == 100.);
        assert_eq!(overlay_stream_size((160, 300), 1.), (160., 90.));
    }
}

#[cfg(test)]
mod theme_coverage_tests {
    use super::*;

    /// A neutral grey that isn't mapped stays the same grey in every theme, which is how
    /// components get left out. Status colours and people's own colours aren't neutral.
    #[test]
    fn white_text_is_readable_on_vivid_banners() {
        for fill in [BANNER_WARNING, BANNER_DANGER] {
            assert!(1.05 / (relative_luminance(fill) + 0.05) >= 4.5, "{fill:06x}");
        }
    }

    #[test]
    fn red_text_stays_readable_on_every_theme() {
        for theme in crate::state::AppTheme::ALL {
            let p = theme.palette();
            let red = relative_luminance(themed_with(p, 0xff6467));
            let surface = relative_luminance(p.surface);
            assert!((red + 0.05) / (surface + 0.05) >= 3., "{}", theme.label());
        }
    }

    #[test]
    fn every_neutral_colour_in_the_source_follows_the_theme() {
        let palette = crate::state::AppTheme::Amethyst.palette();
        let mut missed = vec![];
        for (file, source) in [
            ("auth_views.rs", include_str!("auth_views.rs")),
            ("chat.rs", include_str!("chat.rs")),
            ("components.rs", include_str!("components.rs")),
            ("main.rs", include_str!("main.rs")),
        ] {
            for (number, line) in source.lines().enumerate() {
                for part in line.split("0x").skip(1) {
                    let digits: String = part.chars().take_while(char::is_ascii_hexdigit).collect();
                    let Ok(hex) = u32::from_str_radix(&digits, 16) else { continue };
                    if digits.len() != 6 || hex == 0xffffff || hex == 0 {
                        continue;
                    }
                    let channels = [(hex >> 16) & 255, (hex >> 8) & 255, hex & 255];
                    let spread = channels.iter().max().unwrap() - channels.iter().min().unwrap();
                    // Dark ink on white, a person's name colour, and an icon on green.
                    if [0x111113, 0xcbd5e1, 0xf5fff9].contains(&hex) {
                        continue;
                    }
                    if spread <= 24 && themed_with(palette, hex) == hex {
                        missed.push(format!("{file}:{} {digits}", number + 1));
                    }
                }
            }
        }
        assert!(missed.is_empty(), "Unthemed neutrals: {missed:#?}");
    }
}

#[cfg(test)]
mod decoration_tests {
    use super::*;

    fn write_apng(path: &std::path::Path, frames: u32) {
        let file = std::fs::File::create(path).unwrap();
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), 288, 288);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_animated(frames, 0).unwrap();
        encoder.set_frame_delay(1, 12).unwrap();
        let mut writer = encoder.write_header().unwrap();
        for frame in 0..frames {
            // The source is RGBA, red first.
            let pixel = [(frame * 40) as u8, 7, 200, 255];
            writer.write_image_data(&pixel.repeat(288 * 288)).unwrap();
        }
        writer.finish().unwrap();
    }

    #[test]
    fn apng_keeps_every_frame_in_bgra_at_the_reduced_size() {
        let path = std::env::temp_dir().join(format!("hero-apng-{}.png", uuid::Uuid::new_v4()));
        write_apng(&path, 3);
        let art = decode_apng(&path, DECORATION_FRAME_SIZE).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(art.frame_count(), 3);
        assert_eq!(art.size(0).width.0, DECORATION_FRAME_SIZE as i32);
        assert_eq!(Duration::from(art.delay(0)).as_millis(), 83);
        assert_eq!(art.as_bytes(1).unwrap()[..4], [200, 7, 40, 255]);
    }

    #[test]
    fn only_the_animated_cache_files_are_decoded() {
        assert!(matches!(art_animation(std::path::Path::new("544.png")), ArtAnimation::Still));
    }
}

#[cfg(test)]
mod overlay_raster_tests {
    use super::*;
    #[test]
    fn gpui_image_floor_ceil_keeps_both_sizes_on_one_centre_at_fractional_dpi() {
        for dpi in [1f32, 1.25, 1.5, 1.75, 2., 2.5, 3.] {
            for scale in [0.5, 0.7, 1., 1.5] {
                let base = (40. * scale * dpi).round();
                let parity = base % 2.;
                let expected_x = 200. + parity / 2.;
                let expected_y = 300. + parity / 2.;
                let centre = gpui::point(px(expected_x / dpi), px(expected_y / dpi));
                for diameter in [base, base + 2., base + 4.] {
                    let bounds = overlay_raster_bounds(centre, diameter, dpi);
                    // Mirror the verified snapping contract of Window::paint_image.
                    let width = (f32::from(bounds.size.width) * dpi).ceil();
                    let height = (f32::from(bounds.size.height) * dpi).ceil();
                    let actual_x = (f32::from(bounds.origin.x) * dpi).floor() + width / 2.;
                    let actual_y = (f32::from(bounds.origin.y) * dpi).floor() + height / 2.;
                    assert_eq!((width, height), (diameter, diameter));
                    assert_eq!((actual_x, actual_y), (expected_x, expected_y), "dpi={dpi} scale={scale}");
                }
            }
        }
    }
}

#[cfg(test)]
mod snow_tests {
    use super::{snowflake, snowflake_count};

    #[test]
    fn flakes_fall_inside_the_window_and_keep_moving() {
        for index in 0..140 {
            for seconds in [0., 1.5, 90., 86_400., 2_592_000.] {
                let flake = snowflake(index, seconds, 1200., 800.);
                assert!((-12. ..=812.).contains(&flake.y), "{flake:?}");
                assert!((-22. ..=1222.).contains(&flake.x), "{flake:?}");
                assert!(flake.alpha <= 0.83 && flake.size <= 4.4);
            }
            assert_ne!(snowflake(index, 1., 1200., 800.), snowflake(index, 1.5, 1200., 800.));
        }
    }

    #[test]
    fn tweens_reach_their_target_and_can_turn_around() {
        use super::Tween;
        let mut tween = Tween::default();
        for _ in 0..4 {
            assert!(tween.step(1., 1. / 60.));
        }
        assert!(tween.value > 0. && tween.value < 1.2);
        let turned_at = tween.value;
        assert!(tween.step(0., 1. / 60.));
        assert!(tween.value < turned_at + 0.2);
        while tween.step(0., 1. / 60.) {}
        assert_eq!(tween.value, 0.);
        assert!(!tween.step(0., 1. / 60.));
    }

    #[test]
    fn the_amount_of_snow_is_bounded() {
        assert_eq!(snowflake_count(100., 100.), 30);
        assert_eq!(snowflake_count(1200., 800.), 106);
        assert_eq!(snowflake_count(5000., 3000.), 140);
    }
}
