# Screen sharing: enablement gate and proposed production policy

## Current status (2026-11-06, latest)

Deployed at the user's request: migration `screen_share_guards`, function `screen-share`, and the
`reap-screen-shares` schedule. Hero shares for real.

Checked against production: a solo share reached Cloudflare and went live, the lease was recorded
with nothing reserved, and the schedule closed it afterwards. That test found a real bug, now
fixed: closing a share whose connection had already gone returned 410 and counted as a failed close,
which blocked new shares until fixed. The migration also passes 29 checks in an in-memory Postgres.

**Not yet checked:** watching through Cloudflare, which needs a second person. The viewer path has
only run between two local engines. Main Nitro's budget gap below still applies.

Client: `tools/voice-engine/src/screen.rs`, `src/screen_share.rs`, `src/screen_view.rs`, picker and
pinned panel in `src/chat.rs`, overlay tile in `components::call_overlay`. Your own share shows in the
panel and overlay from a local preview, which never leaves the machine.

Quality settings, from measurements between two engines on this machine:

| Source and preference | Result |
| --- | --- |
| Video, balanced (WebRTC's default) | about 10 fps at 320-480 wide, matching the lag in main Nitro |
| Video, favour frame rate | steady rate, but starts 320 wide and takes about 10 seconds to sharpen |
| Video, hold both (used) | full size and full rate from the first second |

The send cap stays 2.5 Mbit/s, because raising it costs budget. Detailed, fast-moving content will
soften at that rate. Viewers get up to 1600 wide at 30 fps.

Rollback: `select cron.unschedule('reap-screen-shares')`, and set `limit_bytes` to 0 to stop new
shares while anything closes.

Known limits:
- Main Nitro uses `cloudflare-realtime`, so its shares and views bypass the ledger entirely. A main
  client watching a Hero share isn't reserved, but is cut off when the share's 10 minutes end.
- The budget is an estimate from reservations. Actual Cloudflare usage isn't read, which would need
  an API token. It isn't a hard dollar ceiling against a modified client.
- No screen audio yet.
- The picker and panel reuse existing row and card styles, but weren't designed with the user.

## Earlier status

The text below is the earlier proposal, kept for its reasoning and acceptance checks.

## Limits approved by the user on November 6

The user approved the 500 GB monthly budget, one-viewer limit and ten-minute
maximum in this chat. Implementation and deployment acceptance checks below
remain required. Approval of limits is not evidence of deployed enforcement.

- Separate screen PeerConnection from voice, preserving voice latency and isolation.
- Video sender maximum 2,500,000 bits/s; optional screen audio 128,000 bits/s.
- One publisher per voice session; one viewer per publication; no hidden subscriptions.
- Viewer subscribes only after explicit Watch; closing/hiding the preview unsubscribes.
- Ten-minute publication maximum, requiring a fresh deliberate start afterward.
- Viewer heartbeat every 15 seconds; 45-second lease; server reaper every 15 seconds.
- Reserve a full ten-minute viewer allowance atomically before issuing a subscription.
  At 2.628 Mbps plus 25% accounting headroom this is 246,375,000 bytes per viewer.
- Proposed global screen allowance: 500,000,000,000 bytes per UTC calendar month.
  This is an application budget proposal, NOT a verified Cloudflare account allowance.
  TURN and other applications need their own accounting/reserve outside this amount.
- Reject all new/renewed leases on budget, membership, heartbeat, DB or provider failure.
- Do not refund a reservation until every associated provider track is confirmed closed.
- Provider cleanup failure blocks new allocations and leaves its reservation outstanding.

## Server work required BEFORE changing Hero's enablement gate

1. Inspect the live `cloudflare-realtime` function and the private ownership RPCs.
   Do not deploy the workspace copy over a newer production version.
2. Add private, RLS-protected monthly ledger and viewer lease tables. RPC access is
   service-role-only; admission uses a transaction/row lock so parallel requests
   cannot overspend. Client timestamps, claimed usage and bitrate are not trusted.
3. Apply membership/ownership checks to create, publish, subscribe, renewal and close.
   Reserve before the provider call, store every allocated provider session and MID,
   serialize operations per session and compensate late/partial allocations.
4. A scheduled server reaper force-closes both viewer video/audio MIDs at expiry,
   on membership loss and when a publisher ends. Check each provider result, retain
   failures for retry, and expose unresolved cleanup as an operational error.
5. App heartbeats are not revocation. Cloudflare media can continue after a lease
   expires; the reaper must call `PUT /sessions/{id}/tracks/close` with `force:true`.
6. Reconcile against provider/account usage and stop admissions conservatively on
   missing telemetry. A client sender bitrate cap is not a provider-enforced cap;
   malicious or modified clients, delayed telemetry and cleanup outages mean the
   reservation model alone cannot promise a hard dollar ceiling. Confirm the
   provider's available enforcement mechanisms before advertising such a guarantee.
7. Main Nitro currently uses the same broker without these lease operations. Plan
   a compatible client migration or an explicitly Hero-only guarded entry point;
   silently changing the shared contract can break installed v0.1.72 clients.
8. Implement native capture/encoder/viewer, negotiated bitrate verification, local
   stop/watch lifecycle and cancellation. Only enable after the deployed server
   confirms the guard protocol version and real two-client tests pass.

## Required acceptance checks

Concurrent last-budget admission; duplicate retries; wrong owner/conversation;
left/stale voice membership; stale and forged heartbeat; expired lease while media
continues; partial publish/subscribe; lost create response; stop during negotiation;
crashed publisher/viewer; failed force-close; month rollover; provider/API outage;
no viewers and hidden viewers; restart/rejoin; mixed Hero/main-Nitro clients.
Measure actual provider egress alongside reservations and verify voice stays healthy.

## Rollback

Disable admissions first, force-close all outstanding guarded tracks, wait for
confirmed cleanup, then roll back code/schema. Never delete the ledger or cleanup
records while a provider resource may still forward media. Hero stays disabled.

## Provider references verified November 6

- https://developers.cloudflare.com/realtime/sfu/platform/limits/
- https://developers.cloudflare.com/realtime/sfu/get-started/connection-patterns/
- https://developers.cloudflare.com/realtime/sfu/concepts/negotiation/

This file is a reviewable proposal, not evidence that production enforcement exists.
