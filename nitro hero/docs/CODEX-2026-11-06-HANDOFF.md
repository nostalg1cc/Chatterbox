# Codex continuation, November 6, 2026 (living handoff)

**Current authoritative snapshot: `CURRENT-HANDOFF.md`.** Earlier entries below are
historical and include REJECTED fixes. In particular the fallback-only zero-drift
reports and raw fractional-image renderer were insufficient and are superseded.

## Verified GPU image snap root cause and final raster-centred implementation

Reproduced the user's exact offset using a red512 image fixture through the real
image path: at level.05 photo centroid~[59.976,52.976], exposed green~[62.935,56.013].
gpui0.2.2 Window::paint_image FLOORS physical origin and CEILS physical size while
quads/shadows retain fractional geometry. Shared fractional bounds therefore still
shift the photo top-left and expose a bottom-right crescent. Source verified locally.

Final components.rs uses direct ImgResourceLoader-cached photo/APNG data; one fixed
physical-pixel centre, adjacent same-parity raster diameters two pixels apart, lower
photo layer opaque +upper Div-opacity blend. Integer bounds biased inward .0001 to
survive fractional DPI; decoration parity/centre matched, same APNG frame per layer.
Ring/glow paint behind both image layers. Geometry correction does not change audio,
accepted gain/tuning/placement, backend or theme palette. Shared avatar() elsewhere
unchanged; loading fallback still uses it. Intermediate blends may soften image edges.

Helpers: overlay_raster_layer, overlay_raster_bounds, overlay_art_frame. New unit test
asserts exact centre under GPUI floor/ceil at7 DPIs x4 UI scales x3 raster diameters.
New scripts/verify_overlay_alignment.py refuses calls, uses an IMAGE (not initials),
captures30 frames over3 sizes, compares independent weighted red/green centroids,
rejects fallback/missing photo and writes repeatable measurements before reuse of
temporary paths. Result: max photo drift0.0px and max ring separation0.0px.13 app
tests pass, debug build warning-free. Real photo/deco final series also inspected.

Current running exe SHA256:
AF2CFE5991BDD40580A143C11D5AE404994A6395D6DE5294356D475C903240B5
Process39260 verified at workspace target/debug/nitro-hero.exe after final rebuild.
Voice worker unchanged (DC6773...507F4). Human live-motion acceptance still pending;
do not equate these objective render checks with user acceptance. Full source map,
artifact paths, current feature limits and rollback instructions are in CURRENT-HANDOFF.md.

## Latest correction: ring behind artwork; fixed measured portrait centre

User rejected foreground ring over the decoration and still saw avatar drift toward
top-left. The previous foreground-outline approach BELOW IS SUPERSEDED.

- components::call_overlay no longer resizes a flex-centred child. Its constant-size
  Canvas owns one AnyElement produced by the SAME shared avatar() function (no
  copied image/deco markup). During canvas prepaint, layout_as_root measures that
  element at animated size; prepaint_at places its MEASURED box symmetrically around
  the fixed canvas centre. This avoids rounding an animated flex layout separately.
- Canvas paint order is glow, filled circular status ring, portrait.paint LAST.
  All artwork and animated decorations are above the green/red ring. Ring bounds
  dilate measured portrait bounds by1.25px+scaled(1+.5*level), keeping the exposed
  status rim thin behind the existing contact rim. Removed foreground outline.
- Each participant row now has a stable id derived from person.id, isolating image
  animation state when two people use the same decoration asset.
- Low-level smooth fade, meter curve, max4% growth, padding56, placement and
  75/180ms smoothing are unchanged. Audio worker not modified this turn.
- Public GPUI APIs verified in local element.rs: AnyElement::layout_as_root,
  prepaint_at, paint. Canvas prepaint owns the element until paint, same frame.

Verified in NEW running build (not merely source): user explicitly authorized ending
the active call/restarting. Parent stopped, debug build completed, app relaunched.
12 tests pass. MCP ten-level fallback portrait sweep 0,.05,.1,.2,.4,.6,.8,1,.1,0
at70% measured EXACTLY [60.5,53.5] centre for EVERY frame, not the prior +/-0.5px.
Also inspected actual profile+decoration at 0,.1,.3,.7,1,0: ring remains behind art.
Audits: overlay-centered-measurements.json, overlay-centered-review.png,
overlay-behind-photo-review.png. These are sampled rendering checks; live subjective
motion acceptance still belongs to user. App left open with verified new renderer.

QA caveat: custom probe frame filenames reuse frame indices. A later photo sweep
overwrites fallback raw files. Compute measurements/copy frames before the next
probe. Composite review PNGs and JSON measurements above are independent snapshots.

Rollback is entirely components::call_overlay rendering/row ids. Reverting to
animated flex positioning can restore the reproduced centre variance; reintroducing
the foreground outline obscures decorations. No production or audio-worker changes.

## Latest: attached presence ring and uninterrupted smoothing

User reported the green effect felt detached and jittery after the reduced tuning.
This supersedes the separate filled-ring disk and hard speaking-gate descriptions.

- components::call_overlay now puts BOTH paint canvases INSIDE the animated avatar
  wrapper. Glow uses that wrapper's ACTUAL laid-out bounds, not an independently
  calculated rectangle derived from the fixed slot. The shared avatar child occupies
  those same bounds, including Taffy/device-pixel rounding.
- Foreground ring is an outline, not a filled disk behind the avatar. Its bounds
  dilate the avatar by1.25px; stroke width2.5px + scaled .5*level covers the shared
  avatar's black 1.25px contact rim instead of showing a separate green disk beyond
  it. Circular radii follow actual bounds. This intentionally changes only overlay
  chrome; global shared avatar layout/material/image/decoration rendering remains.
- Removed hard `level > .02` visibility/scale switch. Activity fade is smoothstep
  of clamp(level/.08,0,1); multiply existing ring/glow alpha by this fade. Size is
  continuously `40*scale*(1+.04*level)` even near silence. Previous sensitivity,
  padding56, placement, 75/180ms attack/release and microphone curve stay intact.
- hud::Hud tracks `animating`. Target updates reset last_frame ONLY when idle;
  while animating, preserve the timestamp so incoming 50ms meter updates do not
  continually restart the integration clock. Render aggregates animation requests.
- Debug MCP nitro_test_overlay now optionally accepts levels (max16) and photo=true,
  captures each target, returns level metadata and restores by closing the test HUD.
  Defaults still produce quiet/loud captures. Refuses during real calls; no mic,
  calls or backend writes. Photo=true uses only the existing profile art locally.

Verified: 12 app tests pass; debug build passes without warnings. Live MCP inspected
the user's actual photo+animated decoration at levels0,.01,.02,.04,.1,.3,.7,1,.2,0.
Ring visually follows the photo edge through the series and fades through low levels.
Artifacts: audits/overlay-attached-frames.json and overlay-attached-review.png (private
local QA; exclude from releases). This is a sampled rendering check, not a frame-time
benchmark or confirmed human acceptance of final live speech motion.

Rollback: canvas nesting/outline/fade are local to components::call_overlay; smoothing
timestamp handling is in hud.rs. Reverting canvases to independent slot math can
restore rounding mismatch; reverting hard gate can restore popping. Worker/audio
meter NOT changed this turn; latest installed worker hash remains DC6773...507F4.
App restarted only after MCP confirmed no active call; left running with the update.

## Final overlay clipping, centering and quieter tuning

User confirmed the microphone-response fix works, then reported clipped glow,
off-centre scaling, excessive intensity/sensitivity and too much left-edge spacing.
This section SUPERSEDES earlier overlay dimensions, gains and animation constants.

- Shared components::overlay_size now controls native window size and renderer:
  56px padding on each edge, fixed 40px avatar slots, 12px row gap, at most two
  people, optional 150px label allowance (140px label +10px gap), all scaled.
  Window x is display left edge, not previous +24/+8 offsets. Visible avatar is
  about 30px farther left at 70% than the temporary 88px-padding fix.
- GPUI Windows shadow shader uses a THREE-blur-radius support region. Old 14px
  padding could not contain it. Final glow blur is only 8+4*level (12px max);
  padding56 also covers scale growth and native pixel insets at minimum scale.
- components::call_overlay has an immutable centred 40px slot; canvas computes
  avatar bounds about that slot centre, paints glow and an explicit circular ring,
  then flex-centres the avatar at exactly the same animated size. Replaces absolute
  offsets plus a spread BoxShadow (which reused pre-spread corner radii).
- Final max scale is 1+0.04*level (was .08), ring spread 2+level (was 4+2*level),
  ring alpha .22+.5*level, glow alpha .1+.25*level, muted ring spread2. HUD smoothing
  is 75ms attack /180ms release (was 35/120). Layout slots don't change with speech.
- hud::sync applies make_passive/position AFTER open_window returns; doing it
  inside creation allowed GPUI's final setup to reintroduce nonclient insets on
  the first frame. Quiet/loud native window sizes now match from the first capture.
- Final local_meter::display_level is `clamp((peak-.005)/(.14-.005),0,1)^1.25`.
  Removed x4 visual multiplier. Keeps the same effective noise floor as x4 version
  but raises full-scale raw peak from .035 to .14. Recorded speech peak .02535 now
  maps to ~.094, not .60. Actual microphone signal/gain/routing remains unchanged.
- Latest installed voice worker SHA256 (supersedes hashes below):
  DC6773F31445A335A7956F90BD173A60C7F5F74F423D16EB613009A84AA507F4

Verification and MCP additions:
- New debug-only nitro_test_overlay tool creates a temporary synthetic quiet/loud
  HUD, waits for smoothing, captures both windows and always closes it. Refuses
  during real calls. No microphone/audio/back-end access. Optional scale .5..1.5.
- screenshot helper generalized to save_screenshot(hwnd,name); normal main-window
  screenshot behavior preserved. Synthetic filenames contain scale and state.
- Real native capture measurements: at .5 => 76x76, .7 =>106x106, 1.5 =>228x228,
  identical quiet/loud dimensions; avatar centre drift 0 / .5 /0 pixels; no green
  excess on ANY window edge in any capture. Local report audits/overlay-geometry.json.
- Inspected final default-size full-intensity image audits/overlay-soft-final.png:
  complete soft halo, thin ring, circle centred. No physical microphone tests in
  this geometry/tuning turn. User acceptance of this final feel is still pending.
- App tests12 pass, worker tests2 pass, debug build passes; latest source cleanup
  only removes an unused import after that build. Hero left open with final tuning.

Rollback: geometry changes are in components.rs call_overlay/overlay_size and
hud.rs sync, with deterministic MCP captures available to compare. Sensitivity
is independent in local_meter.rs; preserve working endpoint metering if changing
the curve. No production changes, call joins or microphone probes for this turn.

## Latest voice-overlay investigation: local microphone levels

Final outcome: fixed both missing local samples and under-sensitive display gain.
The user confirmed MicrophoneFX is the intended input and coordinated a speech
test. Peak was .0253499, which mapped to only .02049 under the initial curve.
Final display_level uses `(clamp((raw_peak * 4 - .02) / .12, 0, 1))^1.25`, giving
about .60 for that measured speech. This gain affects VISUALS ONLY; it does not
change transmitted audio gain, microphone routing, suppression or voice volume.
Below raw peak .005 remains gated out; quiet .008 is a small response; .035 is full.
Unit regression explicitly checks the measured MicrophoneFX speech exceeds .5.

User reported no overlay movement while speaking. Direct authorized five-second
probe reproduced a concrete fault: native prepare succeeded but the local device
track NativeAudioStream sink emitted ZERO local level samples. Remote decoded
track metering was separate and retained. Animation code alone was not sufficient.

Changes:
- Added tools/voice-engine/src/local_meter.rs. Dedicated Windows COM apartment
  activates IAudioMeterInformation on the EXACT selected capture endpoint. Polls
  GetPeakValue every 50ms, emits normalized `level` events with side=local/empty
  epoch. It reads volume only, does not copy/store PCM, open a second capture stream,
  enable speaker output or join a room. Drop sets stop and joins its thread.
- Engine replaces the non-producing device-track sink with this LocalMeter after
  recording starts, drops/restarts it on route changes, and drops it before stop.
  Default route resolves to factory.recording_device_guid(0), matching the existing
  strict routes::input(SystemDefault)->index0 rule. Explicit GUIDs remain exact;
  there is no silent fallback to another input. Native remote-track monitor unchanged.
- Curve derives from src/lib/voice-activity.ts threshold .02, ceiling .14,
  exponent 1.25, with final visual-only input multiplier 4; clamp bad/out-of-range inputs. This is visual gain, NOT microphone
  audio attenuation or actual in-call input-volume processing.
- Endpoint metering is before WebRTC's per-track processing and can reflect other
  applications using that same endpoint. It does not prove the peer receives audio.
  Muted/deafened local overlay is suppressed; remote uses decoded PCM as before.
- Worker Cargo.toml/Cargo.lock add pinned windows 0.61.3 with Audio/Endpoints/COM/
  StructuredStorage/Variant features. Verified generated API signatures locally;
  Activate needs StructuredStorage+Variant features, not just Audio+COM.
- voice::State gains local_meter_status; supervisor consumes local-meter status
  during preparation and normal event loop. MCP voice.local_meter_status reports
  ready / endpoint-unavailable / com-unavailable without exposing device IDs/audio.
- scripts/mic_level_probe.py requires --allow-microphone and 1..10 second duration;
  caller must have actual user authorization. Uses selected saved input, closes
  stdin and waits for EOF shutdown. No saved audio, playback or room membership.

Verification:
- Before: prepared=true, local levelSamples=0 over five seconds.
- After initial meter replacement: 98 samples, 97 above .002, peak .00859 using
  temporary peak*2 scaling. This proved the source now emits; that temporary scaling
  was then REPLACED with the original Nitro curve described above.
- Final curve probe: 96 samples, meter=ready, rawPeak=.000825, normalized peak=0,
  no errors. User said they were speaking, prompting selected-input verification.
- Selected input readback: Wave Link MicrophoneFX, NOT Mic In or Monitor/Stream.
  User confirmed this was intentional. No device preference was changed.
- A temporary shared WASAPI sample-capture comparison was implemented and tested:
  coordinated speech produced 94 samples, PCM peak=.0253499001 and endpoint peak
  EXACTLY the same. This ruled out the endpoint peak API as the discrepancy. Removed
  the temporary IAudioCaptureClient/format parser and its test before final build:
  the shipping fix uses only IAudioMeterInformation, no extra capture client.
- No PCM was saved or played; measurement probes had no voice membership/credentials.
- Worker unit tests: response at silence/quiet/normal/loud and strict routing pass
  (2 total). App tests 12 pass. Synthetic two-way PCM/mute/reconnect passes (128ms).
  Coordinated physical speech measurement is confirmed; actual visual movement in
  a fresh real-call HUD remains for the user's acceptance, not claimed from unit tests.

Rollback: revert local_meter module + engine field/lifecycle and worker dependency
changes together; restoring old local track sink restores the reproduced zero-data
bug. Status/probe additions are independent. Keep the earlier physical-playout gate;
this change must not re-enable early speaker startup. Latest installed worker SHA256:
882D0960447F6991E0FE18E6D6E7DC9BD202CE75340A7C1E295B2DF8635AA501
supersedes every earlier worker hash below. Final worker tests: 2 pass; full app:
12 pass; synthetic two-peer transport: pass (128ms). Worker installed while no call
was holding it; Hero GUI was rebuilt/restarted with meter diagnostics earlier in
this investigation. A fresh join starts the new worker. No production changes.

## Latest correction: custom accents and control contrast

User corrected the earlier fixed-accent interpretation. The following SUPERSEDES
the fixed-blue and flattened-control descriptions further below:

- Only Default uses #0A84FF. Every other theme derives its own accent hue from
  Nitro's THEME_OPTIONS accent swatch, darkening in 2% RGB steps until white text
  reaches 4.6:1. The exact source background/material colours and IDs stay intact.
- Native control surfaces are intentionally distinct: track is source background
  mixed 42% toward black; selected/raised is material mixed 12% toward white, capped
  to preserve muted-text contrast. Default uses #18181A track / #414147 selected.
- Soft accent is lightened as needed for 4.6:1 on raised; indicator accent for
  3.2:1 on stage. Source swatch values are therefore hue references rather than
  falsely advertised exact final button colours. Generated in sync_themes.py.
- Contrast tests enforce body/muted text >=4.5, non-default white/accent >=4.5,
  indicator-line/stage >=3, soft/raised >=4.5 and selected-tab/track >=1.6, across
  all 29. Only Default retains its existing white/blue 3.65:1 exception. Also rejects
  accidental default-blue accents in non-default themes.
- components::themed maps the gallery's blue soft New badge surface to selected
  theme surface outside Default; personal avatar/name art and status colours retain
  their meaning. Tabs and all raised/track consumers use shared palette roles.
- nitro hero/AGENTS.md now records the user's Default-only blue requirement so a
  future edit does not repeat the fixed-blue regression.
- Live MCP captures visually checked Default, Amethyst and Gold in BOTH gallery
  and chat: tabs, sliders, primary/secondary buttons, sidebar and composer. Script
  scripts/theme_ui_audit.py restores initial theme/view and never writes backend.
  Captures are private local audits/theme-ui files; keep out of releases.
- cargo test --locked: 12 passed. Generator --check still protects exact source
  IDs/bg/material AND derived native palettes. Reverse this correction by reverting
  generator changes and regenerating themes.rs plus the badge/test changes, but
  that would restore the unwanted fixed accents and low-contrast tabs.

## Scope and safety

Work is in `nitro hero/`. Main Nitro, other native experiments, stable updater,
production schema/functions and releases have NOT been modified by this continuation.
The workspace was already heavily dirty/untracked, including the whole Hero folder.
Do not stage the whole repo. Source changes below are local and unreleased.
User explicitly requested Claude-readable change and reversal notes.

## User decisions and current outstanding requests

- Approved screen limits: 500 GB/month, one viewer/share, ten-minute shares.
  See SCREEN-SHARE-GUARDS.md. Actual server enforcement and native capture are
  NOT implemented; the Hero share button fails closed. Do not call this shipping.
- User approved leaving the solo call and restarting Hero to replace its worker.
  This was done; parent termination closed stdin and the worker exited via EOF.
- Implemented follow-up: volume-reactive overlay scale with smooth attack/release;
  horizontally elastic draggable tabs; device persistence/recovery tests; all 29
  Nitro theme IDs/palettes; signed in-place migration skeleton (not an installer).
- Never join a real microphone/call for tests without explicit authorization.
  The MCP was run with --dev-mcp only, no --dev-mcp-write-chat allowance.

## Implemented changes and reversal map

### Media cards

- `src/main.rs`: MediaFile::Ready is now a struct variant with `original` and
  optional `sidebar` PathBufs. `src/sync.rs::fetch_image` produces both off-thread.
- `src/backend.rs::sidebar_thumbnail`: image reader guesses format because cache
  filenames can lack extensions. Creates a 555x352 centre-cover PNG with Triangle
  resampling beside original, appending `.sidebar-v1.png` to the full filename;
  temp write then rename, cached on later visits. No new delivery link/download.
- `src/chat.rs::render_chat_image` delegates to render_media_image(Contain).
  Cover selects the pre-cropped thumbnail and uses Fill, so GPUI rounds the actual
  visible bitmap rather than the enlarged original. Chat keeps original/Contain.
- Sidebar outer material card is 178.5px tall with 1.25px rim inset and a 176px
  image. Inner radius 10.75px, outer 12px. Labels/reply are absolutely overlaid at
  bottom with a transparent-to-black fade; no separate footer space. Nonshrinking
  cards, 12px gaps; existing lazy viewport requests and caching retained.
- Why: GPUI img.rs paints Cover into expanded image bounds; parent overflow clips
  rectangles, not rounded child corners. Rounding a Cover img alone did not fix it.
- Reverse together: revert Ready shape + sync producer + chat reader, then remove
  thumbnail function/test. Thumbnail files can simply remain unused in cache.

### Slider

- `src/components.rs::SliderMotion`: stores press_from, preserving continuity on
  early release or re-press. 180ms cubic press; handle grows 28x20 -> 33x24.
  Radius always height/2. White inset shrinks by 10% under press (HeroUI reference).
- Stretch uses bounded exponential rubber band (7px max), critically damped
  monotone return (340ms), normalized factor clamped [0,1] to avoid FP overshoot.
- Removed SliderDrag/Empty drag payload. Window-wide MouseMove + MouseUp capture
  handlers keep dragging alive beyond track hitbox and release anywhere. Value
  snapping still uses 12px end insets; keyboard behavior preserved.
- MCP gained nitro_drag: posts 12 moves with held button, captures held state,
  releases, waits 400ms. Backend writes blocked throughout. No credentials exposed.
- Reverse pointer handling and animation independently if needed, but preserve
  dynamic radius; restoring fixed radius is what made the pressed handle boxy.

### Tabs (final follow-up)

- TabsState.select retriggers on a repeated selection and continues indicator
  position from its current interpolation. Edge selection uses a 420ms sin² pulse,
  max 4px sideways stretch. Vertical lift is now ZERO; bar is always 40px high,
  indicator 32px. TabsState owns Rc<Cell> drag index/bounds. Canvas registers
  window-wide Down/Move/Up capture handlers: start only within bounds, select the
  segment under the pointer as it crosses, clamp outside ends, release anywhere.
  Removed per-tab click selection to avoid duplicate commits. Existing press sound
  stays on each segment. Live MCP drag Overview->Settings changed panel correctly.
  Reverse drag handlers + bounds/drag fields together; remove pulse separately.

### Decorations and profile copy

- `backend::cached_decoration` downloads full original Cloudinary APNG only for
  equipped profile art into `{id}-animated.png` (safe_name maps hyphen to underscore).
  Keeps catalogue decoration_file as 96px stills. Falls back to still on failure;
  existing per-session failed-download suppression applies.
- `components::avatar` gives decoration Img a stable path-derived ID, which GPUI
  needs to retain ImgState/frame index. GPUI 0.2.2 already decodes APNG frames.
- Real cache inspection: 049 and 544 are each 60 frames; combined new downloads
  1,198,709 bytes. No pixel-motion benchmark or background CPU acceptance yet.
- Production read-only check: tlgf and nrohde (display name waatq) ALREADY both
  have avatar_decoration=544, name_decoration=resize. No DB update was necessary.
- Reverse APNG selection + stable ID if required; still catalogue cache is intact.

### Voice startup/static investigation

- `tools/voice-engine/src/engine.rs`: set_adm_playout_enabled(false) BEFORE acquiring
  platform ADM and during preparation. Merely removing explicit StartPlayout did
  not prevent libwebrtc automatically enabling the hardware path.
- Added update_playout and deafened state. Platform output only opens after a
  generation-matched remote-ready command; clear_peer closes it and resets readiness.
- `src/voice.rs`: on generation-filtered first-decoded(remote), send remote-ready
  carrying epoch. Track discovery alone no longer marks decoded readiness.
- Important library detail verified in webrtc-sys 0.3.48/src/adm_proxy.cpp and
  synthetic_audio_device.cpp: disabling PLATFORM playout keeps a synthetic pump
  requesting decoded PCM. update_playout MUST NOT stop that synthetic pump while
  awaiting the first frame, or readiness can deadlock. Synthetic test mode without
  an acquired ADM is left untouched by update_playout.
- Installed rebuilt worker SHA256:
  7E26E04609328C1D7A5B2FF2AFA649CEA101052F552DA997AB5AE830C5285E48
  (workspace target/debug and TEMP/nh/debug matched after user-approved restart).
- This is a supported startup-path fix, NOT proof the user's static is solved.
  Real two-person audible output and device-specific Wave Link testing remain.
- Reverse engine/supervisor protocol together. Never restore platform output
  enabled at prepare solely because a synthetic transport test passes.

### Overlay native frame

- `src/hud.rs::make_passive`: remove caption/thickframe/border style bits, disable
  DWM nonclient rendering (attribute 2, value 1), set no border colour (attribute 34,
  0xfffffffe), apply SWP_FRAMECHANGED. Existing transparent/topmost/clickthrough/
  noactivate flags retained. Avatar speaking glow is intentional, separate from
  the unwanted rectangular Windows frame/shadow. Visual desktop acceptance pending.
- Reverse just the new style/DWM calls to isolate any transparency regression.

### Volume-reactive overlay follow-up

- `hud.rs::Hud` retains normalized levels per participant and smooths toward new
  targets on animation frames: 35ms exponential attack, 120ms release, clamp dt to
  100ms, settle at 0.002 error. Drops departed participant state. Updating targets
  resets last_frame so an idle overlay does not jump straight to a loud new level.
- `components::call_overlay`: stable 40px layout slot; avatar/ring expands around
  its centre by `1 + level * 0.08` while speaking (>0.02); ring spread 4+2*level,
  glow blur 14+8*level, glow alpha .5+.4*level, matching original voice-hud.css.
  Muted ring stays red and never scales. No rectangular shadow is added.
- Targets come from the existing engine's measured RMS*4 levels, not a timed pulse.
  Real whisper/loudness response and native desktop appearance still need human
  acceptance. Reverse smoothing and geometry independently if isolating an issue.

### Exact Nitro themes and persistence

- `scripts/sync_themes.py` reads src/lib/themes.ts for all 29 IDs/labels and actual
  src/features/v3-shell/styles.css for --app-bg/--material-color. HSL is converted
  to nearest 8-bit RGB. `--check` verifies generated source has not drifted.
- Generated `src/themes.rs` replaces the placeholder AppTheme enum/palettes formerly
  in state.rs; state reexports it, main declares mod themes. Same canonical camelCase
  IDs serialize as main Nitro, old PascalCase names deserialize as aliases. Retired
  Ocean->teal, Moss->lime, Ember->sunset; DefaultDark->default. All 29 round-trip.
- All native surface roles now use the original material colour; canvas/stage/media
  use original app background; foreground #fff and muted #d9d9d9 match live CSS.
  Functional iOS blue remains shared (source picker swatch accents are NOT live CSS
  accent overrides). `components::themed` now also maps Default; main input colours
  apply for Default too. Accent-white contrast is the preexisting 3.65:1 exception
  for ALL themes, explicitly reported instead of mutating source palettes to pass.
- MCP nitro_set_theme accepts canonical ID or label; removed obsolete 12-name enum.
- Device dropdowns already saved input_device_id/output_device_id and routed live.
  Preferences now use serde(default), preserving saved devices if newer fields are
  absent. save_state writes/syncs .json.tmp then renames over primary, preserving a
  valid previous .json.bak; load falls back to backup if primary is corrupt. Errors
  go to stderr; a visible save-failure notification is not implemented.
- Tests verify native device IDs+theme survive disk save/replace/reload and corrupt
  primary recovery, and partial older preferences retain IDs/default missing fields.
  This does NOT imply browser WebRTC device IDs will map automatically on migration.
- Reverse generated themes/state reexport/main module + mapping edits together.
  Do not rewrite preferences to old aliases without preserving backup. Reverting
  file safety is independent of themes; backup remains ordinary JSON (no session).

### Stable upgrade skeleton

- `release/upgrade-contract.json` and docs/STABLE-UPGRADE-SKELETON.md describe stable
  identity, signed NSIS replacement, key boundaries and preferences mapping. Disabled,
  design-only: no executable signing, installer build, registry edits or key access.
- Preserve io.github.nostalg1cc.dislight, stable nitro.exe/current-user NSIS identity,
  existing Tauri public key and endpoint; use a version above 0.1.72; future Hero
  must implement its own compatible updater. Signature verification is mandatory.
- Tauri localStorage export/import remains unimplemented. Preserve original profile
  files; map camelCase fields to native fields and canonical theme IDs; reconcile
  browser/native device IDs; never export refresh tokens in plain preferences.
- Warning documented: old installer-hooks.nsh broadly removes LOCALAPPDATA/Dislight;
  do not reuse that cleanup blindly for a native migration. No source changes there.

## Verification so far

- cargo build --locked: pass; cargo test --locked: all 12 pass after themes/devices.
- python scripts/sync_themes.py --check: all 29 canonical IDs/palettes match sources.
- powershell -File scripts/build-voice.ps1: pass and installed after solo call exit.
- python scripts/voice_transport_smoke.py: two-way synthetic native PCM, simultaneous
  offers, mute/unmute, restart all pass; latest measured connection+decode 119ms.
  This test never opens physical audio devices and does not prove audible quality.
- Live MCP: signed-in restore, media cards inspected with full image/overlay label,
  slider held at endpoint reaches 100 and remains capsule-shaped. PrintWindow held
  screenshots may omit text while composition is active; do not treat as a full
  animation/performance benchmark.
- Screenshots named *check.png and *final.png and hero-*.log are local test outputs
  in Hero folder; contain chat, keep out of releases. Temp screenshot path is reused;
  copy it into workspace before inspecting to avoid stale file/path confusion.

## Next maintainer checklist

Verify final app restart and worker hash; get human acceptance for audio/HUD/motion.
Do not equate approved budget limits or a disabled share button
with deployed server quota enforcement. No production write/release has occurred.

Final local handoff state: Hero was rebuilt after the last smoothing adjustment,
restarted with --dev-mcp while out of a call, and left open. The approved worker
hash above remains installed. No real call was opened by Codex for acceptance.
