# Nitro Hero

## November 6 maintainer update

Theme rule: only Default uses normal iOS blue. Other themes have their own accent
family, darker tracks and brighter selected controls, with contrast checks across
all 29 palettes. The original Nitro background/material colours and IDs are retained.

Read `docs/CODEX-2026-11-06-HANDOFF.md` for the current change inventory,
verification and rollback notes; older preview descriptions below are historical.
Equipped avatar decorations now use cached full APNGs with GPUI frame playback.
Voice uses the native helper and screen sharing is explicitly unavailable until
native capture and server usage enforcement exist. Approved usage limits and
the remaining server work are in `docs/SCREEN-SHARE-GUARDS.md`.

The shared slider handles window-wide pointer movement and keeps a capsule
radius throughout press/release; edge tabs perform an automatic elastic pulse.
Sidebar images use local cover thumbnails inside the material rim, with labels
and reply controls over the image. No gallery viewer or video playback is implied.

A separate native GPUI component preview for Nitro. It displays a native gallery of the requested Avatar, Avatar Group, Badge, Alert, and Tabs examples on a black canvas. The Tauri app and existing GPUI experiment are independent.

## Run

From this directory, with the Rust MSVC toolchain and Windows SDK installed:

```powershell
cargo run --locked
```

The native window uses GPUI and no webview. Close the window to exit.

## HeroUI reference

`reference/` pins `@heroui/react` as a source and behavior reference. Restore it with `npm ci` in that directory. It is not loaded by the GPUI application. Native components will be independent implementations, with visual and behavioral fidelity verified individually against HeroUI; parity is not assumed.

## Design guidance

`vendor/unslop` contains Unslop by Damir Sokolovsky, licensed CC BY 4.0. Its guidance can inform future design work; faithful component recreations should be compared to their requested references.

## Backend

Log in, sign up, conversations, messages, replies, reactions, friend requests and onboarding completion use the production Supabase project, with `VITE_SUPABASE_URL` and `VITE_SUPABASE_ANON_KEY` from the repo's `.env`. Every request carries the user's own access token, so Row Level Security applies. Failed logins never sign in.

Only the refresh token is saved, encrypted for the current Windows user with DPAPI, at `%LOCALAPPDATA%\Nitro Hero\session.bin`. Preferences are in `preferences.json` beside it, and avatars are cached there too. Chat is never written to disk.

New messages are polled (every 3 seconds when focused, 15 otherwise), not pushed by Realtime yet. History loads 50 messages at a time as you scroll up. Images download only when near the viewport, at most three at once, and are cached on disk. Avatar decorations show their still frame, and name colours, weights and all 8 name effects are applied. Name effects repaint at 20 fps, only while one is on screen and the window is focused. The soundboard lists the sounds this account can see; the server only returns the shared list to members of a live voice room. The call button reads the real voice room: Voice when empty, Join with who's in, and the shared Call Pill locally after joining. Sounds play on this device only; other people hear them once voice exists. Pushed notices from `app_notices` appear as the top banner, checked once a minute. The push secret belongs only in the command sending a notice, never in this app. Not connected yet: video playback, presence, animated decorations, name fonts, choosing a name effect in Hero, and settings/profile edits beyond onboarding. In-call controls appear only after Join voice, which is still a local preview.

## Dev MCP

Debug builds can expose the running app to tests, so they don't need desktop automation:

```powershell
cargo run -- --dev-mcp
python scripts/mcp_call.py nitro_get_state
python scripts/mcp_call.py nitro_screenshot
```

It listens on 127.0.0.1 only, on a random port, with a random bearer token, both written to `%LOCALAPPDATA%\Nitro Hero\dev-mcp.json`. `nitro-hero.exe --mcp-bridge` is a stdio bridge for MCP clients and is registered in the repo's `.mcp.json`. Release builds compile all of it out.

State never includes credentials, tokens or message bodies, the password field can't be set, and backend writes are refused during MCP input, except sends and reactions in the one conversation named by `--dev-mcp-write-chat=<username>`. `nitro_screenshot` captures this window as it appears, including chat. Add tools in `src/dev_mcp.rs` as tests need them.

## Calls

Calls use the same room membership, private signaling and Cloudflare TURN credentials as main Nitro, so the two can call each other. The audio engine is a separate process, `tools/voice-engine`, built with its own runtime flags. Run this once, and again if the engine changes:

```powershell
powershell -File scripts/build-voice.ps1
python scripts/voice_transport_smoke.py
```

Cloudflare bills only egress, with a shared 1,000 GB free each month across TURN and the SFU. Direct connections and STUN are free. Voice can't reach the limit: even an always-on relayed two-person call at 64 kbps is about 41 GB a month. Screen sharing can (about 1.1 GB per viewer-hour at 2.5 Mbps), so it's guarded by a monthly budget and leases on the server. See `docs/SCREEN-SHARE-GUARDS.md`.

## Interface sounds

`src/audio.rs` ports Nitro's interface sounds: its synthesized click and menu tones, and its recorded mute, message, reaction and voice WAVs in `assets/sounds`. Soundboard clips are WebM/Opus, decoded with `opus-decoder` and `matroska-demuxer`. Output uses the system default device, not the selected one.

## Components

The UI playground is the source of truth. Tabs, Slider, Button Group, Media Grid, Call Pill, Voice Button, Top Banner, System Marker, Name Effects and the call Split Toggle are shared functions in `src/components.rs` used by both the playground and the app. Button, Toggle Group, Dropdown and Input examples are still written inline in the playground and need the same extraction.

## Message bursts

Consecutive captionless media from one sender, each within 2 minutes of the last, render as one Media Grid card. Files with the same kind, byte size and dimensions show as one tile with a count, and appear once in the sidebar. The database stores no content hash, so this is a size-based match; Cloudinary etags confirmed it for the existing duplicates. Blocking duplicate uploads waits until Hero has uploads.

## Themes

Each of the 12 themes in `src/state.rs` is a palette of colour roles for surfaces, accent, text, selection and separators. Hero's `components::rgb` maps every colour through the active palette, so hard-coded greys follow the theme too, and input placeholders, carets and selection follow it as well. Default Dark keeps Nitro's normal colours, and status colours stay fixed. Use `components::rgb` for new UI, and `gpui::rgb` only for a colour that must not be themed. `cargo test contrast -- --nocapture` prints every pair and enforces WCAG AA for body and muted text on all surfaces, and for white on the accent on non-default themes. Default Dark's white-on-accent is 3.65:1 and intentionally unchanged.

## Current preview coverage

Avatar sizes, image and initial fallbacks; a stacked group with an overflow count; badge counts and anchored badges; alert status examples and dismiss/restore; a primary tab bar with animated selection. These are initial demonstrations, not complete HeroUI parity. Keyboard navigation, accessibility semantics, animated transitions, all variants, and systematic pixel comparison remain to be completed.



The official HeroUI v3 source and documentation demos are available in `reference/heroui-source/`; `reference/node_modules` contains the pinned React and Gravity UI icon packages. Extracted Gravity UI example icons live in `assets/icons/`. The split-group segments are preview actions; dropdown menus are not implemented yet.
