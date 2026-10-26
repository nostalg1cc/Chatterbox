# Nitro streaming investigation — October 26, 2026

Scope: normal Tauri/React Nitro. GPUI/native beta directories are experimental
and excluded unless the user explicitly mentions them (recorded in AGENTS.md).

## Current published status

Normal Nitro desktop and web are now published as v0.1.72 from stable-base
commit 141a5413e8060f601c2f3e496c7183c9bc1a3b66. The desktop installer,
signature and updater manifest are on the public GitHub release; uploaded
asset SHA-256/size match the verified local files. The public latest updater
serves 0.1.72 with the matching 416-character signature and no BOM; installer
delivery is HTTP 200. Vercel production deployment
dpl_43X8BkNQQg188JCJ7Pz5iKP4mTcA is READY at dislight.vercel.app with that
commit; public bundle verification confirms version/search/web header code.

The second live retest found PGRST106 on ownership persistence: PostgREST does
not expose private, including to service_role. Migration
20261026074414 adds narrowly granted SECURITY INVOKER read/save RPCs; private
stays unexposed, anon/authenticated cannot execute them, and a rollback-only
service-role fixture passed create, track save, readback and stale-lease
rejection. Advisors remain unchanged. Broker v12 uses these RPCs.
The user then reported sharing appears to stay active. Full partner reception
and adverse-network acceptance remain unverified.

The final bubbles use opaque #171717, 15px/22px text, and no stroke. Web-only
Settings/media-sidebar controls use existing app actions; browser checks
confirm both behaviors and absence of duplicate controls in native mode.
The normal dev app was rebuilt/restarted on 0.1.72. GPUI remains excluded.

## Follow-up: immediate failure, wide previews, search and message bubbles

The user's live test exposed a further broker defect in v10. October 26 logs
at 07:18–07:20 UTC show create failing with `decoding_error` / “Body JSON
validation error: sessionDescription.” The broker sent `{}` with a JSON
content type for an SDP-less create. Cloudflare's bodyless-create route needs
an actual absent body; `{}` selects its offer-validation schema. The first
mock accepted `{}` and did not exercise that distinction.

Corrected the outbound request to omit both body and Content-Type for a
bodyless create; offer creates and track/answer operations retain JSON.
Deployed `cloudflare-realtime` v11, ACTIVE, with JWT verification retained;
source readback confirms the corrected wire shape. Regression now constructs
a real Request and asserts zero body bytes and no JSON content type.

Wide (>16:9) main-app and overlay previews now fill via centered cropping,
preserving proportions rather than stretching. Other aspect ratios retain
contain behavior. Production soundboard menu now has an autofocus search
field, trimmed case-insensitive sound-name matching, clear action and no-match
feedback, preserving pin order and playback. Its viewport height allowance
accounts for the new field.

Added latest-message bubbles below each call participant, before any stream.
Only live chat INSERT events emit them; history fetch/open does not. One
bubble per sender replaces its predecessor for ten seconds. Replayed IDs do
not extend expiry, edits preserve the deadline, deletion removes the bubble,
and account/call/conversation changes isolate and clear state. Text is bounded,
rendered without embeds, and never logged or persisted by the HUD. Attachment
messages without text use a short image/video label. Window sizing expands
for bubbles and shrinks on expiry. Main chat and HUD import the exact same
Framer Motion body entrance values (left slide/overshoot, 215ms delay, 360ms
duration); HUD respects reduced motion.

Follow-up validation: frontend build and broker type check pass; eight existing
and eleven streaming/broker/message regressions pass. Headless production-
component checks verify actual soundboard search/play/clear/empty states and
the absence of black bars at the top/bottom of a relayed ultrawide capture.
The existing synthetic WebRTC/HUD browser test additionally verifies latest-
message replacement, stable DOM across roster replay and ten-second expiry.
Normal Nitro dev was restarted as a detached launcher (debug executable PID
24824 at verification, local server 5174). Live Cloudflare acceptance remains
pending the user's retest; no real call or screen capture was initiated by
these tests.

## Confirmed cause

Production `cloudflare-realtime` logs from October 25 at 14:32:23 and
14:32:29 UTC report upstream HTTP 400, `invalid_session_description`:
“SDP contains no ice-ufrag. Please ensure the SDP is valid.” The broker returned
502 to the client. Deployed v9 matched the local source.

Both factories in `src/lib/cloudflare-realtime.ts` created an SDP offer before
adding any transceivers, then sent it to `/sessions/new`. That offer has no
media sections or media ICE credentials. Creation fails before any screen
tracks can reach the partner. Retrying the same offer cannot repair it.
There is currently no fallback transport; screen sharing depends entirely on
this Cloudflare connection. Microphone voice uses a separate peer connection.

The overlay had a separate functional gap: its update payload contained only
participants, and its renderer only displayed avatars/names. A MediaStream
cannot be sent directly through serialized Tauri events to another webview.

## Applied fixes

- Follow Cloudflare's current documented flow: create an SDP-less session,
  add the publisher's screen video/audio, submit its media offer via
  `tracks/new`, and apply the answer. The subscriber creates an SDP-less
  session, requests remote tracks, answers the SFU offer and gathers ICE
  before submitting the answer via `renegotiate`. Connections use max-bundle.
- Broker accepts the new create shape and retains support for valid legacy
  offer creates, existing authentication, participant/session/conversation
  authorization, quotas, and server-only credentials. Partial provider track
  errors and failed track-metadata persistence fail explicitly.
- Cancellation interrupts ICE/connection waits and closes pending transports.
  Failed subscriptions remove any media exposed by `ontrack` before a later
  negotiation failure. Stop presence clears pending as well as connected
  viewers. Incoming video track end clears viewer state.
- Optional monotonic screen revisions reject reordered publish/stop messages.
  Old clients without revisions remain accepted until a revised sender is
  observed for that voice session. Duplicate in-flight publications do not
  repeatedly cancel their own subscription. Outgoing obsolete screen signals
  are not replayed from the signaling queue.
- Starting state prevents concurrent display pickers. Stop/capture-end/leave
  clears the capture, boosted audio graph, publishing connection and UI flags.
  A share-state change behind an older heartbeat issues a follow-up heartbeat
  rather than leaving the old flag in the database until the next interval.
- An app-level HUD bridge follows the active voice call, independent of the
  selected conversation or settings view. It relays only existing video
  tracks locally as 320×180 JPEG previews, at most 8 frames/second per source,
  with one event in flight per source and no overlay audio playback.
- Each preview appears beneath its owner. Window bounds expand/recenter for
  one/two streams and shrink after stop. A remote publication keeps a waiting
  slot during setup/recovery; stale frames expire, stream IDs reject obsolete
  frames, and stopping removes the slot. A roster watchdog clears content if
  the owning page stops updating. Relay teardown detaches video without
  stopping tracks owned by the call.

## Verification

- `npm run build` passes; existing bundle-size/updater-import warnings remain.
- `deno check --config supabase/functions/cloudflare-realtime/deno.json
  supabase/functions/cloudflare-realtime/index.ts` passes.
- Six existing voice ownership/recovery regressions and two screen
  cancellation regressions pass.
- Ten streaming/broker regressions pass in `audits/streaming-regressions.mjs`.
- `audits/streaming-browser-smoke.mjs` passes with isolated headless Chromium:
  real WebRTC publisher/subscriber factories receive synthetic canvas video
  and oscillator audio through a local mock SFU. Measured framesDecoded > 2,
  nonzero inbound audio packets, and video dimensions 320×180 (normal adaptive
  initial resolution). The built HUD renders two owner-matched previews,
  expires stale frames and removes stopped/call-ended UI with no page errors.
  Its local screenshot is `audits/streaming-overlay-smoke.png`.
- Deployed only `cloudflare-realtime` to `lapjrxdgcbdseskmyfru`, now ACTIVE v10
  with JWT verification enabled. Readback confirms the expected source;
  anonymous POST returns 401. No migrations, secrets or release changes.

## Remaining validation and alternatives

Both users need the corrected frontend to test both sending and receiving.
The installed v0.1.71 client still sends empty create offers; the server change
alone cannot repair that client's WebRTC negotiation. The corrected v0.1.72
desktop update and matching production web version are now published.

The headless tests use synthetic media and a local mock provider, not a real
two-account Cloudflare call. Verify that live flow in the dev app: each user
starts/stops/restarts sharing, ends it through the OS picker, leaves/rejoins,
switches chats, and minimizes the main window while watching the HUD. Check
captured audio only for sources that actually offer it. WebView2 background
timer/decode throttling and real Tauri cross-window transport remain runtime
acceptance items; headless DOM tests do not prove them.

The recommended first fix is the verified negotiation repair above. If live
testing subsequently shows provider/network outages, an optional resilience
improvement is a separate P2P screen connection, with independent screen
audio routing and signaling. Do not casually put screen tracks back onto the
microphone connection: it undoes the app's intentional voice isolation.
That fallback is a larger addition and is not implemented here.

References: [Cloudflare connection patterns](https://developers.cloudflare.com/realtime/sfu/get-started/connection-patterns/),
[official Cloudflare example](https://github.com/cloudflare/realtime-examples/blob/main/echo/index.html),
[Supabase broadcast acknowledgement](https://supabase.com/docs/guides/realtime/broadcast).
