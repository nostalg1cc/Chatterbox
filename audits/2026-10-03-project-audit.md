# Nitro project audit — 3 October 2026

Reviewed local v0.1.69 at `0f8db56`, deployed Supabase project `lapjrxdgcbdseskmyfru`, Vercel production deployment metadata, and Cloudinary usage/asset metadata. No production data, settings, credentials, or app behavior were modified. Audit emphasis: voice room membership, network/IP changes, disconnect/reconnect, screen sharing, and backend authorization.

No critical unauthenticated account/room takeover was established. There are high-priority correctness and authorization defects. This is a source/configuration audit with isolated reproductions, not a completed two-user network impairment test.

## What is good

- Voice uses direct WebRTC with authenticated Cloudflare TURN recovery. Cloudflare SFU carries screen sharing on a separate peer connection. Supabase owns durable membership and private signaling. This is a reasonable architecture for one-to-one calls.
- Room generations and voice session IDs isolate many stale events; intentional leave clears local media without waiting for server cleanup.
- The September hardening is present: 5-second client TURN timeout, 4-second provider timeout, nonblocking credential loading, usable TLS TURN URLs, bounded signaling outbox, server acknowledgements, deduplication, and relay retries/backoff.
- Deployed voice join/leave cleanup uses a shared transaction advisory lock (join acquires it through cleanup). Unique membership prevents a user occupying several voice rooms. Authenticated heartbeat/leave operations scope updates to the user's session ID.
- All public database tables have RLS. Voice tables are read-only to clients; controlled functions handle membership. Cloudflare session operations bind session ownership and conversation. Soundboard access uses private storage and short-lived signed URLs.
- Production TypeScript/Vite build passes. Remaining build notices concern bundle size and ineffective updater code splitting, not compilation failure.
- Cloudinary usage is currently low: 1.23/25 credits (4.92%), 876,984,865 storage bytes and 439,942,670 bandwidth bytes, with usage last updated 2 October. This describes current usage, not a spending guarantee.

## High-priority findings

### H1 — Canceling a pending join does not cancel its server response

Source: `src/stores/voice.ts:396–514`, especially the RPC at 429 and the state installation after it.

`joinAttempt` is checked after microphone acquisition, but not after `join_voice_room`. If Leave, account cleanup, or another join happens while that RPC waits, its old success can restore the canceled session. Old errors/conflict paths can also clear shared microphone/UI state belonging to newer work. The microphone is assigned to a module global before cancellation is checked, compounding overlapping join races.

**Evidence:** the audit harness executes the extracted production join, cancels its attempt during the mocked RPC, then resolves the old RPC. The old room/session becomes active again.

**Fix:** own microphone/channel resources per operation; validate operation ID, account, room and session after every await. Compensate for a canceled successful server join by leaving only its returned session. Superseded callbacks must never stop newer resources.

### H2 — An old heartbeat can disconnect a new session

Source: `src/stores/voice.ts:1666–1683`.

`sendHeartbeat` captures a session, awaits its RPC, and calls `disconnectLocal(false)` on `not_found` without checking that the current session is still the one queried. Fast leave/rejoin or takeover can turn a legitimate old-session failure into teardown of the new call.

**Evidence:** reproduced with the actual extracted function: an old heartbeat resolved after switching state to a new session tears down the new session. `verifyActiveLease` already demonstrates the safer identity-check pattern elsewhere.

**Fix:** compare captured session, conversation and account against current state after await. Avoid overlapping heartbeat requests and bound hung requests.

### H3 — Signaling presence loss immediately destroys healthy media

Source: `src/stores/voice.ts:1137–1165`.

An empty remote presence snapshot closes the voice peer immediately, even if its connection is still connected. A partner's Supabase socket interruption can therefore interrupt otherwise working direct/TURN audio and be shown as a real leave. A channel resubscription can also temporarily lack partner presence while reconciliation completes.

**Evidence:** extracted presence-sync function, mocked empty snapshot and connected media peer: peer closed and status set to solo.

**Fix:** distinguish temporary signaling absence from explicit leave/confirmed expired lease. Keep healthy media through a bounded grace period; reconcile durable membership before destroying it. Preserve prompt behavior for an intentional leave.

### H4 — Longer outages lose the room lease; recovery never reacquires it

Source: `src/stores/voice.ts:1067–1080, 1110–1130, 1618–1683`; deployed `private.cleanup_stale_voice_rooms` and `private.heartbeat_voice_room` (mirrored in `supabase/migrations/20260815120000_purge_short_voice_calls.sql`).

Heartbeats occur every 45 seconds, but start only after signaling subscribes and presence tracking completes. Database cleanup removes seats older than 120 seconds, running once per minute and on joins. A prolonged initial signaling failure, outage, or system sleep can lose membership while the client still intends to stay in voice. Returning heartbeat `not_found` ends the session; it does not rejoin. Signaling recovery keeps the old room generation, which cannot work once the empty room has been finalized/recreated.

**Fix:** start lease maintenance immediately after successful join. On resume/reconnect, verify and renew membership before media negotiation. Handle expired room generations explicitly. Track whether the user still wants voice, and do not automatically steal a seat from another active device.

**Timing:** 120 seconds is measured from the last successful heartbeat, not from the start of an outage. Cleanup timing depends on cron phase and other joins. The empty-room grace is 20 seconds, but finalization is evaluated by cleanup; it is not an exact 20-second timer.

### H5 — Screen sharing has ownership, cleanup and recovery defects

Source: `src/stores/voice.ts:550–612, 1301–1313, 1731–1753, 1977–2101`; `src/lib/cloudflare-realtime.ts`.

- One global `cloudflareScreenConnection` holds either the local publisher or remote subscriber. Receiving a partner's published screen closes the local publisher. Two simultaneous shares cannot coexist reliably.
- `stopLocalScreen` returns immediately if no local video track exists. A viewer-only leave therefore does not close its subscriber; neither voice-peer teardown nor local disconnect otherwise closes that connection. Media can continue receiving after the UI clears it.
- Screen peers lack post-connection failure/restart monitoring. Voice ICE recovery does not repair them after an IP change.
- Publisher session/track IDs are broadcast once and not retained/reannounced to late joiners or reconnecting partners. A partner joining an already-running share has no reliable subscription path.
- Publisher/subscriber factories have no failure cleanup around the allocated peer, and their API calls lack an explicit timeout. Awaited capture/publication/subscription results have no current-session guard; stale completions can attach media after leaving or switching rooms.

**Evidence:** viewer-only stop early return reproduced in isolation; other cases are direct source traces, not live dual-share tests.

**Fix:** separate publisher and subscriber ownership; close both on leave; close factory peers on errors; add operation cancellation and timeouts. Persist/reannounce current screen metadata and independently recover screen transport.

### H6 — Web production lacks the current voice fixes

Vercel's production deployment `dpl_3SGoyBhUymFoe9Nw3Pej5aSXDEk1` is READY and owns `dislight.vercel.app`, but its source commit is `805dd21` (release v0.1.65). Local/desktop source is v0.1.69. This is verified deployment metadata, not an assumption from the handoff.

Browser users therefore do not receive the later client voice hardening. Desktop-to-browser tests can be testing mismatched recovery behavior.

**Fix:** align production web and desktop releases and show client version in diagnostics. Do not deploy as part of this audit.

### H7 — Friendship acceptance allows participant identity changes

Source: `supabase/migrations/20260709164800_rls_policies.sql:57`; live policy, column privileges and trigger definitions checked.

The update policy checks that the OLD row belongs to the pending request's addressee, but its new-row check only requires accepted/blocked status. Authenticated users have UPDATE privileges on requester/addressee columns. No installed trigger makes those IDs immutable. The acceptance trigger creates a conversation from the NEW IDs.

A recipient can retain themselves as addressee, change requester to another profile, and accept, causing an unsolicited conversation/fabricated relationship with that profile. This is an authorization defect; it does not establish access to the victim's existing conversations. No live exploit/update was performed.

**Fix:** prohibit changing participant IDs at the database boundary; restrict updates to allowed columns and validate status transitions. Add a transactional policy regression test using disposable rows in a development database.

### H8 — Link-preview IPv6 filtering is bypassed

Source: `supabase/functions/link-preview/index.ts:58–72`; same guard confirmed in deployed version 8.

WHATWG URL hostnames retain IPv6 brackets (`[::1]`), but the guard compares to `::1` and prefixes `fc`, `fd`, `fe80`. It consequently accepts loopback/private IPv6 addresses. Hostname checks also do not establish that a DNS name resolves to a public address.

**Evidence:** executing the actual guard accepts `http://[::1]/`, `http://[fd00::1]/`, and `http://[fe80::1]/`. No request was made to those addresses. Whether the hosting network permits reaching them was not tested.

**Fix:** normalize and classify IP literals correctly; enforce public resolved destinations and safe redirects, with DNS-rebinding protection or a restricted outbound fetch service. Current redirect and response-size limits are useful but insufficient for destination authorization.

### H9 — DM attachment delivery is public by URL

Source: `src/lib/media.ts:15–22`; signed upload fields in `supabase/functions/purge-chat-media/index.ts:183–184`. Live Cloudinary chat assets exist under delivery type `upload` without returned access controls.

Attachments use ordinary Cloudinary upload URLs; possession of the URL enables delivery without Supabase conversation authorization. Random IDs reduce guessing but do not revoke a copied/shared URL. Soft-deleting a message also does not immediately destroy its Cloudinary asset; the current cleanup does not select on message `deleted_at` and attachments receive a 100-year expiry.

**Severity:** high if private/revocable DM media is the requirement. This is an access-model gap, not evidence that someone has accessed another user's attachments.

**Fix:** authenticated delivery/signed expiring URLs or an authorized proxy, plus explicit attachment deletion/invalidation policy.

## Medium-priority findings

### M1 — Realtime reconnection does not reconcile database snapshots

`subscribeToVoiceDiscovery` ends with `.subscribe()` without a resubscription handler; chat/friend subscriptions behave similarly (`src/stores/chat.ts:516–603`, `src/stores/friends.ts:121–165`). There is no application catch-up query after missed events. Cached `loadMessages` returns immediately for an already-loaded conversation. A restored socket can leave stale room/participant lists, messages, reactions or unread counts until reload.

**Fix:** resubscription reconciliation for voice and incremental/refetched chat/friend state; merge snapshots carefully with events and current-session guards.

### M2 — TURN lifetime and service abuse controls are incomplete

Credentials last 12 hours, but no proactive refresh is scheduled for a healthy relayed call. Refresh is driven by joining/recovery. Cloudflare documents disconnection after credential expiry, so long calls can suffer an avoidable disruption. See [Cloudflare TURN FAQ](https://developers.cloudflare.com/realtime/turn/faq/).

The broker verifies conversation participation and existence of a voice seat, but not request session identity or seat freshness, and has no application rate limiter/issuance quota. Similar limits are missing from screen session creation; the public tweet proxy also lacks app-level abuse controls. Provider/account controls were not inspectable.

**Fix:** renew before expiry with a safe ICE transition, bind sensitive operations to the active lease, reuse/cache credentials deliberately, and add appropriate user/service quotas and alerts.

### M3 — Takeover signaling trusts client-declared session identity

Deployed `can_access_voice_topic` authorizes conversation participants and room generation, not the active voice seat. Clients accept remote presence `userId/sessionId` and signal `fromSessionId` without matching them against authoritative current membership. Thus another signed-in device on the same account can enter that channel without holding the current seat and interfere with session selection. This is not arbitrary outside-account room access.

RLS alone cannot immediately revoke an already-authorized socket: [Supabase documents channel authorization caching](https://supabase.com/docs/guides/realtime/authorization). Tightening join-time policies should be paired with authoritative session validation/event handling.

**Fix:** authorize current seat at channel join, reconcile sender session against durable membership, and design explicit revocation/takeover behavior.

### M4 — Audio/device state and failure diagnostics need improvement

The voice preferences subscription recreates the microphone on device-ID changes, but not on noise-suppression changes. There is no raw microphone `ended`/device-removal recovery. Speaker users always have echo cancellation disabled. The final recovery error says the direct connection needs TURN even after TURN was already attempted. `/ping` is useful, but no joined timeline explains lease, signaling, ICE and screen failures together.

**Fix:** apply changed capture constraints deliberately, recover removed devices, offer suitable echo control, and record a local diagnostic timeline with selected route, receive progress, ICE transitions, retries and lease results. Never include credentials, message contents or raw SDP in routine telemetry.

## Expected behavior during network changes

| Scenario | Current behavior / concern | Required verification |
|---|---|---|
| Brief media-path failure/IP change | ICE disconnected state waits 6 seconds, then restart; failed state restarts promptly. TURN/rebuild recovery exists. | Both directions resume on Wi-Fi/Ethernet and VPN changes; no stale signals damage the rebuilt peer. |
| Signaling socket fails, media remains healthy | Presence removal can close working media (H3). | Preserve audio during signaling-only interruption; reconcile confirmed leave separately. |
| VPN/IP changes without offline/online browser event | Recovery relies on ICE state; `online` is only a supplementary trigger. | Terminal failed state can recover when reachability returns without a new `online` event. |
| Outage/sleep expires membership | Seat is removed after its 120-second freshness window; current client ends rather than rejoining (H4). | Resume intent survives appropriately; another device's takeover is respected. |
| Leave while join/heartbeat is waiting | Stale result can resurrect/terminate sessions (H1/H2). | Fast cancel, switch and account sign-out cannot affect replacement sessions. |
| Partner joins an existing screen share | Publisher metadata is not reliably reannounced (H5). | Screen appears without restarting capture. |
| IP change during screen share | Voice recovery does not repair the independent screen connection (H5). | Voice and screen recover separately and show accurate statuses. |
| Viewer leaves / both users share screens | Subscriber cleanup missing / publisher and subscriber share one variable (H5). | Zero remaining peers/media after leave; simultaneous shares do not close each other. |

## Validation and limits

- `npm run build`: passed outside the sandbox after sandbox process restrictions blocked the initial attempt; no dependency changes needed.
- `node audits/2026-10-03-voice-repro.mjs`: five source-extracted reproductions confirmed (four voice lifecycle cases and IPv6 URL guard bypass). Dependencies mocked; these are deterministic defect demonstrations, not media performance measurements.
- Supabase checked read-only: deployed voice functions, policies, triggers, authenticated friendship column privileges, cleanup cron, edge function source and security advisors. No public tables lacked RLS. Advisor warnings: leaked-password protection disabled and intentional username availability SECURITY DEFINER exposure; private tables without policies are intentionally inaccessible to clients, not automatically security holes.
- Last-24-hour function/realtime log text search found no messages containing error/failed. This narrow result cannot prove service health or explain historical call failures.
- Vercel production deployment inspected successfully; project-details tool had a schema mismatch, so settings/environment details were not verified.
- Cloudflare has no direct account connector exposed in this session. Deployed TURN/SFU integration code and official docs were checked; account analytics, live allocations, billing controls and provider network health were not checked.
- No two-user audio test, real IP switch, sleep/resume, packet-loss injection, restrictive-firewall test or live security exploit was performed. The existing handoff's two-client test requirement remains open.

## Recommended order

1. Fix session ownership/cancellation (H1/H2) and friendship/preview authorization (H7/H8).
2. Preserve healthy media through signaling interruptions, reconcile leases and snapshots, then recover expired membership/generation (H3/H4/M1).
3. Repair independent screen lifecycle/recovery (H5).
4. Align web and desktop production versions (H6), and decide private attachment delivery/deletion behavior (H9).
5. Add diagnostic timelines and run the two-client matrix above before tuning recovery thresholds or replacing the one-to-one media architecture.

## Remediation — Nitro v0.1.70

The historical findings above describe v0.1.69. The authorized fixes below are implemented in v0.1.70:

| Finding | Remediation |
|---|---|
| H1/H2 | Join/microphone ownership is tied to attempt, account, desired room and lease. Membership mutations serialize; canceled accepted seats are left explicitly. Pending captures stop on leave. Heartbeat/recovery responses and peer rebuilds verify current ownership. |
| H3/H4 | Lease heartbeat begins immediately after successful join. Healthy media survives missing signaling presence while durable membership is checked. Resume/reconnect renews an expired lease with the same session, respecting another device's takeover and changing room generations. |
| H5 | Independent publisher/subscriber peers and abort controllers; failed factories close their peers; leave aborts pending factories immediately. Screen transport retries, late-join metadata replay, relay ICE servers, and 105-minute provider-session renewal are implemented. |
| H6 | Desktop and Vercel production are aligned to v0.1.70. Production startup smoke check passes. |
| H7 | Friendship identity trigger plus status-only authenticated UPDATE grant. Mutation and role-permission checks pass. |
| H8 | IPv4/IPv6/mapped/reserved URL tests pass. DNS results must all be public; requests pin the checked numeric socket while preserving hostname TLS verification; redirects repeat validation. Body size, header size, ports and total request time are bounded. Local protected public fetch returns HTTP 200. |
| H9 | All 13 legacy Cloudinary chat assets moved to authenticated delivery with invalidation. Broker verifies conversation/message access and undeleted state before issuing five-minute URLs. Signed image/video requests return 206; expired/forged signatures return 401. Delete requests invalidate assets immediately with hourly retry. Legacy Storage reads also require an undeleted attachment message. |
| M1 | Voice discovery, friends and cached chat windows reconcile after Realtime resubscription; missed messages paginate and edits/deletes/reactions reload. Snapshot writes check account/session identity; newer received events win over older message snapshots. |
| M2 | Service-only atomic quotas cover relay, screen operations, uploads/downloads, previews and the public Twitter video proxy. Relay credentials renew before expiry; screen sessions renew before their two-hour authorization deadline. Proxy transfers have byte/time limits and disallow redirects. |
| M3 | Secret room topics are returned only through the authenticated current-seat join implementation. Departure/takeover rotates generation and secret; clients validate sender session against authoritative membership. Public RPC stays an unprivileged wrapper. |
| M4 | Removed microphones recover; capture preference changes apply live; optional echo cancellation defaults on. Bounded local metadata diagnostics include lease/signaling/ICE/screen events and route/quality samples, without credentials, messages or SDP. |

Also secured the scheduled cleanup bypass: the public application JWT alone now returns 403; cron supplies a private Vault-backed cleanup header. Deployment inspection caught signing backups and recovery files in CLI inputs; explicit ignore rules and an isolated tracked-source build now exclude them.

Verification: eight source-extracted voice/screen regression cases pass; address-classification regression passes; TypeScript, five Edge Function checks, Vite and signed NSIS builds pass. SQL rollback tests verified friendship immutability, quota behavior, secured topic issuance, takeover revocation, leave revocation and lease reacquisition; production privileges and cleanup denial were verified. Installer signature is verified cryptographically against the configured updater public key.

Remaining operational limits: real two-device direct/TURN/network-switch/signaling-outage/sleep/packet-loss tests have not been performed. Cloudflare account-level analytics and billing controls remain uninspected because no direct account connector is available. Supabase leaked-password protection remains disabled and requires account/dashboard configuration; no management credential for changing Auth settings is available here. Username availability is intentionally public for signup; private tables intentionally have no client policies. Already issued attachment links can remain usable for up to five minutes, CDN invalidation can take time, and already downloaded/local cached copies cannot be recalled.
