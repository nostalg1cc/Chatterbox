//! Links in message text: finding them, styling them, and their preview cards.
use gpui::{HighlightStyle, Hsla, UnderlineStyle, px};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    hash::{DefaultHasher, Hash, Hasher},
    ops::Range,
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};

/// Cards shown under one message, so a list of links doesn't fill the screen.
pub const MAX_CARDS: usize = 2;
/// The preview function allows 30 calls a minute. Staying under it means a chat full of links
/// fills in gradually, and never gets refused.
const CALL_SPACING: Duration = Duration::from_millis(2500);
/// How long a failed lookup rests. It's only tried again if the link is still on screen.
const RETRY_AFTER: Duration = Duration::from_secs(120);
/// A picture is decoded only to crop it, so one that claims to be enormous isn't worth it.
const LARGEST_PICTURE_PIXELS: u64 = 40_000_000;
const CACHE_LIMIT_BYTES: u64 = 200 * 1024 * 1024;

/// The web addresses in a piece of text. Only http and https count, since these get opened.
pub fn find(text: &str) -> Vec<Range<usize>> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(offset) = text[from..].find("http") {
        let start = from + offset;
        let rest = &text[start..];
        let scheme = if rest.starts_with("https://") {
            8
        } else if rest.starts_with("http://") {
            7
        } else {
            from = start + 4;
            continue;
        };
        // "shttp://" is part of another word, not a link.
        if text[..start].chars().next_back().is_some_and(char::is_alphanumeric) {
            from = start + scheme;
            continue;
        }
        let token = rest
            .find(|c: char| c.is_whitespace() || c.is_control() || matches!(c, '<' | '>' | '"'))
            .unwrap_or(rest.len());
        let length = trim_trailing(&rest[..token]);
        if length > scheme {
            found.push(start..start + length);
        }
        from = start + token.max(scheme);
    }
    found
}

/// Sentence punctuation after a link isn't part of it, but a bracket it opened is.
fn trim_trailing(link: &str) -> usize {
    let mut end = link.len();
    while let Some(last) = link[..end].chars().next_back() {
        let unmatched = match last {
            ')' => link[..end].matches(')').count() > link[..end].matches('(').count(),
            ']' => link[..end].matches(']').count() > link[..end].matches('[').count(),
            '.' | ',' | ';' | ':' | '!' | '?' | '\'' | '*' | '_' | '~' => true,
            _ => false,
        };
        if !unmatched {
            break;
        }
        end -= last.len_utf8();
    }
    end
}

/// Text styling for links and a selection, split so that no two ranges overlap.
pub fn highlights(
    links: &[Range<usize>],
    selection: Option<Range<usize>>,
    link: Hsla,
    selected: Hsla,
) -> Vec<(Range<usize>, HighlightStyle)> {
    let selection = selection.filter(|range| !range.is_empty());
    let mut cuts: Vec<usize> = links.iter().flat_map(|range| [range.start, range.end]).collect();
    cuts.extend(selection.iter().flat_map(|range| [range.start, range.end]));
    cuts.sort_unstable();
    cuts.dedup();
    let mut styled = Vec::new();
    for pair in cuts.windows(2) {
        let (start, end) = (pair[0], pair[1]);
        let is_link = links.iter().any(|range| range.start <= start && end <= range.end);
        let is_selected = selection.as_ref().is_some_and(|range| range.start <= start && end <= range.end);
        if !is_link && !is_selected {
            continue;
        }
        styled.push((
            start..end,
            HighlightStyle {
                color: is_link.then_some(link),
                underline: is_link.then_some(UnderlineStyle {
                    thickness: px(1.),
                    color: Some(link),
                    wavy: false,
                }),
                background_color: is_selected.then_some(selected),
                ..Default::default()
            },
        ));
    }
    styled
}

/// Whether a link from a saved or received preview is safe to hand to the system to open.
pub fn can_open(url: &str) -> bool {
    (url.starts_with("https://") || url.starts_with("http://"))
        && !url.chars().any(|c| c.is_whitespace() || c.is_control() || matches!(c, '<' | '>' | '"'))
}

/// A position moved back to the start of its character, so slicing there can't panic.
pub fn on_character(text: &str, mut index: usize) -> usize {
    index = index.min(text.len());
    while !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

/// The word around a position, for a double click.
pub fn word_at(text: &str, index: usize) -> Range<usize> {
    let index = on_character(text, index);
    let is_word = |c: char| !c.is_whitespace();
    let start = text[..index]
        .char_indices()
        .rev()
        .find(|(_, c)| !is_word(*c))
        .map_or(0, |(at, c)| at + c.len_utf8());
    let end = text[index..].find(|c: char| !is_word(c)).map_or(text.len(), |at| index + at);
    start..end
}

/// What's selected in message text. Selections stay inside one message.
#[derive(Clone, Debug, PartialEq)]
pub struct TextSelection {
    pub message: usize,
    pub text: std::rc::Rc<String>,
    pub anchor: usize,
    pub head: usize,
    pub dragging: bool,
}

impl TextSelection {
    pub fn range(&self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }

    /// The selected words, if any.
    pub fn words(&self) -> Option<&str> {
        self.text.get(self.range()).filter(|words| !words.is_empty())
    }
}

/// A preview, whichever kind of page it came from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LinkCard {
    pub url: String,
    pub site: String,
    pub title: String,
    pub description: String,
    pub image: Option<String>,
    pub playable: bool,
}

/// One link's preview once it has been looked up.
#[derive(Clone, Debug, PartialEq)]
pub struct LoadedCard {
    pub card: LinkCard,
    pub image: Option<PathBuf>,
    /// Width over height, when the picture could be read.
    pub aspect: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LinkState {
    Loading,
    /// The page has no preview.
    Nothing,
    /// Not looked up yet, or the lookup failed. It can be tried again after this time.
    Later(Instant),
    Ready(LoadedCard),
}

fn host(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    host.strip_prefix("www.").unwrap_or(host).to_string()
}

/// Reads the link-preview function's three kinds of answer into one card.
pub fn card_from(requested: &str, preview: &Value) -> Option<LinkCard> {
    let text = |key: &str| preview[key].as_str().unwrap_or("").trim().to_string();
    let url = Some(text("url")).filter(|url| can_open(url)).unwrap_or_else(|| requested.to_string());
    let card = match preview["kind"].as_str()? {
        "youtube" => LinkCard {
            site: "YouTube".into(),
            title: text("title"),
            description: text("authorName"),
            image: preview["thumbnail"].as_str().map(String::from),
            playable: true,
            url,
        },
        "tweet" => {
            let author = &preview["author"];
            let first = preview["media"].as_array().and_then(|media| media.first());
            LinkCard {
                site: format!("@{}", author["handle"].as_str().unwrap_or("")),
                title: author["name"].as_str().unwrap_or("").to_string(),
                description: text("text"),
                image: first
                    .and_then(|media| {
                        if media["type"] == "photo" { media["url"].as_str() } else { media["thumbnailUrl"].as_str() }
                    })
                    .map(String::from),
                playable: first.is_some_and(|media| media["type"] != "photo"),
                url,
            }
        }
        "site" => LinkCard {
            site: Some(text("siteName")).filter(|site| !site.is_empty()).unwrap_or_else(|| host(&url)),
            title: text("title"),
            description: text("description"),
            image: preview["image"].as_str().map(String::from),
            playable: false,
            url,
        },
        _ => return None,
    };
    (!card.title.is_empty()).then_some(card)
}

fn cache_name(url: &str) -> String {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// What's kept of a picture to reach a shape: always inside it, and at least one pixel.
fn crop_to_shape(width: u32, height: u32, shape: f32) -> (u32, u32) {
    let kept = if (width as f32) / (height.max(1) as f32) > shape {
        ((height as f32 * shape) as u32, height)
    } else {
        (width, (width as f32 / shape) as u32)
    };
    (kept.0.clamp(1, width.max(1)), kept.1.clamp(1, height.max(1)))
}

/// The picture cut to the card's shape once, so it can keep its rounded corners, which a crop
/// at paint time would lose. Video thumbnails are 4:3 with black bands that this removes.
fn card_picture(original: &std::path::Path, file: &std::path::Path, playable: bool) -> Option<(PathBuf, f32)> {
    let shape_of = |width: u32, height: u32| width as f32 / height.max(1) as f32;
    if file.exists() {
        let (width, height) = image::image_dimensions(file).ok()?;
        return Some((file.to_path_buf(), shape_of(width, height)));
    }
    let reader = || image::ImageReader::open(original).ok()?.with_guessed_format().ok();
    let (width, height) = reader()?.into_dimensions().ok()?;
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > LARGEST_PICTURE_PIXELS {
        return None;
    }
    let picture = reader()?.decode().ok()?;
    let shape = shape_of(width, height);
    let wanted = if playable {
        16. / 9.
    } else if shape >= 1.3 {
        shape.clamp(1.77, 3.3)
    } else {
        1.
    };
    let (kept_width, kept_height) = crop_to_shape(width, height, wanted);
    let cropped = picture.crop_imm((width - kept_width) / 2, (height - kept_height) / 2, kept_width, kept_height);
    // Twice the size it's shown at, for high-resolution screens.
    let largest = if wanted >= 1.3 { 792 } else { 152 };
    let sized = if kept_width > largest {
        cropped.resize(largest, u32::MAX, image::imageops::FilterType::Triangle)
    } else {
        cropped
    };
    // JPEG has no transparency, and a transparent logo mustn't turn black.
    let mut flat = sized.to_rgba8();
    for pixel in flat.pixels_mut() {
        let alpha = u32::from(pixel[3]);
        for (channel, behind) in pixel.0.iter_mut().zip([0x2b_u32, 0x2f, 0x38]) {
            *channel = ((u32::from(*channel) * alpha + behind * (255 - alpha)) / 255) as u8;
        }
    }
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgba8(flat)
        .to_rgb8()
        .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Jpeg)
        .ok()?;
    crate::backend::write_whole(file, &bytes).ok()?;
    let _ = std::fs::remove_file(original);
    Some((file.to_path_buf(), wanted))
}

/// Whether a server call may go now. Asking is what uses up the turn.
fn take_call_turn() -> bool {
    static NEXT: Mutex<Option<Instant>> = Mutex::new(None);
    let Ok(mut next) = NEXT.lock() else { return true };
    let now = Instant::now();
    if next.is_some_and(|next| now < next) {
        return false;
    }
    *next = Some(now + CALL_SPACING);
    true
}

/// A link's card, from disk when it has been seen before, so each costs one server call and
/// one picture download ever. "No preview" isn't saved: the function also answers that when a
/// site was briefly down, so it's worth one more try next session.
pub fn load(url: &str) -> LinkState {
    let name = cache_name(url);
    let saved = crate::backend::cache_file("link-previews", &format!("{name}.json"));
    let remembered = std::fs::read(&saved).ok().and_then(|bytes| serde_json::from_slice::<LinkCard>(&bytes).ok());
    let card = match remembered {
        Some(card) => card,
        None => {
            if !take_call_turn() {
                return LinkState::Later(Instant::now() + CALL_SPACING);
            }
            let Ok(reply) = crate::backend::fetch_link_preview(url) else {
                return LinkState::Later(Instant::now() + RETRY_AFTER);
            };
            let Some(card) = card_from(url, &reply["preview"]) else {
                return LinkState::Nothing;
            };
            if let Ok(bytes) = serde_json::to_vec(&card) {
                let _ = crate::backend::write_whole(&saved, &bytes);
            }
            card
        }
    };
    let picture = card.image.as_deref().filter(|image| image.starts_with("https://")).and_then(|image| {
        let shaped = crate::backend::cache_file("link-previews", &format!("{name}.card.jpg"));
        let original = crate::backend::cache_file("link-previews", &format!("{name}.image"));
        if !shaped.exists() && !original.exists() {
            crate::backend::download_preview_image(image, &original)?;
            crate::backend::trim_cache("link-previews", CACHE_LIMIT_BYTES);
        }
        card_picture(&original, &shaped, card.playable)
    });
    let (image, aspect) = picture.map_or((None, None), |(file, aspect)| (Some(file), Some(aspect)));
    LinkState::Ready(LoadedCard { card, image, aspect })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn links(text: &str) -> Vec<&str> {
        find(text).into_iter().map(|range| &text[range]).collect()
    }

    #[test]
    fn finds_addresses_and_leaves_the_sentence_out() {
        assert_eq!(links("https://open.spotify.com/track/6wAOzF7"), ["https://open.spotify.com/track/6wAOzF7"]);
        assert_eq!(links("see https://a.com/x, or http://b.org."), ["https://a.com/x", "http://b.org"]);
        assert_eq!(links("(https://a.com/x)"), ["https://a.com/x"]);
        assert_eq!(links("https://en.wikipedia.org/wiki/Go_(game)"), ["https://en.wikipedia.org/wiki/Go_(game)"]);
        assert_eq!(links("<https://a.com/é?q=1>"), ["https://a.com/é?q=1"]);
    }

    #[test]
    fn other_text_and_schemes_are_not_links() {
        assert!(links("http is a protocol, https:// alone isn't a link").is_empty());
        assert!(links("javascript:alert(1) file:///c:/x ftp://a.com shttp://a.com").is_empty());
    }

    #[test]
    fn highlights_never_overlap() {
        let (link, selected) = (gpui::black(), gpui::white());
        let styled = highlights(&[4..10], Some(7..14), link, selected);
        let ranges: Vec<_> = styled.iter().map(|(range, _)| range.clone()).collect();
        assert_eq!(ranges, [4..7, 7..10, 10..14]);
        assert!(styled[0].1.color.is_some() && styled[0].1.background_color.is_none());
        assert!(styled[1].1.color.is_some() && styled[1].1.background_color.is_some());
        assert!(styled[2].1.color.is_none() && styled[2].1.background_color.is_some());
    }

    #[test]
    fn only_plain_web_links_can_be_opened() {
        assert!(can_open("https://a.com/x?y=1"));
        assert!(!can_open("httpx://a.com"));
        assert!(!can_open("file:///c:/windows"));
        assert!(!can_open("https://a.com/\" onload"));
    }

    #[test]
    fn crops_stay_inside_the_picture_at_every_shape() {
        for (width, height) in [(1, 1), (3, 1), (1, 3), (480, 360), (1200, 630), (7, 4), (4096, 17)] {
            for shape in [1., 16. / 9., 3.3] {
                let (kept_width, kept_height) = crop_to_shape(width, height, shape);
                assert!((1..=width).contains(&kept_width) && (1..=height).contains(&kept_height));
            }
        }
        assert_eq!(crop_to_shape(480, 360, 16. / 9.), (480, 270));
    }

    #[test]
    fn positions_inside_a_character_move_to_its_start() {
        assert_eq!(on_character("aé", 2), 1);
        assert_eq!(on_character("aé", 9), 3);
        assert_eq!(word_at("one twö", 6), 4..8);
    }

    #[test]
    fn double_click_takes_a_whole_word() {
        assert_eq!(word_at("one twö three", 5), 4..8);
        assert_eq!(word_at("one", 3), 0..3);
    }

    #[test]
    fn every_preview_kind_becomes_a_card() {
        let site = json!({"kind":"site","url":"https://www.a.com/x","title":"T"});
        let card = card_from("https://www.a.com/x", &site).unwrap();
        assert_eq!((card.site.as_str(), card.playable), ("a.com", false));
        let video = json!({"kind":"youtube","url":"https://y.be/1","title":"V","thumbnail":"https://i/1.jpg","authorName":"A"});
        assert!(card_from("https://y.be/1", &video).unwrap().playable);
        let tweet = json!({"kind":"tweet","url":"https://x.com/a/status/1","author":{"name":"N","handle":"a"},"text":"hi",
            "media":[{"type":"video","url":"https://v","thumbnailUrl":"https://t"}]});
        let card = card_from("https://x.com/a/status/1", &tweet).unwrap();
        assert_eq!((card.site.as_str(), card.image.as_deref(), card.playable), ("@a", Some("https://t"), true));
        assert!(card_from("https://a.com", &Value::Null).is_none());
    }
}
