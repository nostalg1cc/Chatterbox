# Dislight ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â Agent Handoff Document

> **Purpose:** This file is the single source of truth for cross-agent handoff (Claude Code ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬Â Codex).
> Whoever works on this project: **update the checklist below as you complete work**, and add notes
> in "Decisions & gotchas" when you make a non-obvious choice. Keep it current after every phase.

## What is this project?

**Dislight** ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â a Windows desktop 1-on-1 chat app (Discord-inspired, but no servers/guilds; strictly DM-focused).
Sleek, minimal, **dark-only** UI: Mica by default (Acrylic optional) for the transparent sidebar/window frame, plus a bordered near-black chat surface.

## Stack

- **Shell:** Tauri 2.x Ã¢â‚¬â€ `decorations: false`, `transparent: false`, custom overlaid titlebar, solid black native window background
- **Frontend:** React 19 + TypeScript + Vite, Tailwind CSS v4, shadcn/ui, dark-only, bundled Google Sans Flex
- **State:** Zustand stores (`src/stores/`) + Supabase Realtime subscriptions pushing into stores. No react-query.
- **Backend:** Supabase (auth + Postgres/RLS + realtime). No custom server. Rust surface is minimal (window effects only).
- **Package manager:** npm (pnpm not installed on this machine)
- **Routing:** none ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â top-level state machine in `App.tsx`: `booting ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢ auth ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢ main`

## Supabase

- **Org:** "Nate's projects" (`vercel_icfg_oxyNEBfLgvm7JSw54EpiKrX9`)
- **Project:** `dislight` ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â ref `lapjrxdgcbdseskmyfru`, region `eu-north-1`, free tier ($0/month)
- **DO NOT TOUCH** the other project `supabase-red-village` (`wqxtjrkvdhitcduztcju`)
- Client env vars in `.env` (gitignored): `VITE_SUPABASE_URL`, `VITE_SUPABASE_ANON_KEY` (see `.env.example`)
- Migrations are applied via Supabase MCP `apply_migration` AND mirrored in `supabase/migrations/*.sql` (repo copy is the reference; keep both in sync)
- **Email confirmation:** new projects default to ON. App handles both states. User can disable in Dashboard ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢ Authentication ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢ Sign In / Up ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢ Email ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢ "Confirm email" for instant signup in dev.

### Schema (all tables RLS-enabled, in `public`)

| Table | Purpose | Key points |
|---|---|---|
| profiles | user profile | auth user id; username unique; live avatar metadata + constrained name color |
| `friendships` | requests + friends | `requester_id`, `addressee_id`, `status` enum pending/accepted/blocked; unique on ordered pair |
| `conversations` | 1:1 threads | `user1_id < user2_id` ordered pair, unique; auto-created by trigger when friendship ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢ accepted; `last_message_at` bumped by message trigger |
| messages | chat messages | soft delete/edit; optional WebP/WebM attachment metadata; server media expires after 3 days |
| `reactions` | emoji reactions | unique (message_id, user_id, emoji); emoji ÃƒÂ¢Ã¢â‚¬Â°Ã‚Â¤ 8 chars |
| `conversation_reads` | unread tracking | PK (conversation_id, user_id), `last_read_at` |

- Helper: `public.is_participant(conv_id uuid)` ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â SECURITY DEFINER, used by policies to avoid recursion
- Realtime publication includes messages, reactions, friendships, conversations, and profiles
- Presence/typing: Supabase Realtime channels (presence channel `online`, broadcast `typing:{conversation_id}`), no tables

## Layout / design language

- Grid: `[titlebar 32px] / [sidebar 280px | chat pane]`
- Mica shows through **titlebar + sidebar** (transparent layers); **chat pane is solid** `bg-background` (this is native Win11 style ÃƒÆ’Ã‚Â  la File Explorer ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â NOT glassmorphism; do not add blur/gradients)
- Near-black zinc tokens, raised hairline borders, bundled Google Sans Flex with Segoe UI Variable fallback, lucide icons
- Destructive red is the only accent color

## Checklist (keep updated!)

### Phase 0 ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â Handoff
- [x] AGENTS-HANDOFF.md + AGENTS.md created

### Phase 1 ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â Supabase backend
- [x] Project created (`lapjrxdgcbdseskmyfru`)
- [x] Migration: schema (tables, enum, triggers, indexes) ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â `initial_schema`
- [x] Migration: RLS policies + helpers ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â `rls_policies`
- [x] Migration: realtime publication + replica identity full on reactions/friendships ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â `realtime_publication`
- [x] Migration: helpers moved to `private` schema, trigger fn EXECUTE revoked ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â `lock_down_function_exposure`
- [x] Advisors: security = 0 findings; performance = only "unused index" INFO (expected, fresh DB)
- [x] `.env` + `.env.example` written (URL + `sb_publishable_...` key in `VITE_SUPABASE_ANON_KEY`)
- [x] Migrations mirrored to `supabase/migrations/`

### Phase 2 ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â Scaffold
- [x] Vite + React + TS scaffold (hand-written, in repo root; create-vite refused non-empty dir)
- [x] Tailwind v4 + shadcn init (radix base, **nova preset**, Geist font bundled via @fontsource) + 19 components in `src/components/ui/`
- [x] Tauri init: undecorated/transparent/shadow/dark window, strict prod CSP (devCsp null for HMR), window-control capabilities
- [x] Rust: window-vibrancy `apply_mica` in setup + `is_mica_supported` command (cargo check passes)
- [x] git init + .gitignore + initial commit

### Phase 3 ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â Window shell
- [x] `components/titlebar.tsx` (drag region, min/max/close, hides outside Tauri)
- [x] App layout grid, no window scrollbars

### Phase 4 ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â Auth + boot
- [x] `lib/supabase.ts` client
- [x] Boot screen + session restore + onAuthStateChange routing
- [x] Login form (email+password)
- [x] Signup form (username availability check, display name, verify-email state + resend)
- [x] `stores/auth.ts`

### Phase 5 ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â Main app
- [x] `stores/friends.ts`, `stores/chat.ts`, `stores/presence.ts`
- [x] Sidebar: conversation list (unread badge, online dot, last message, context menu), user footer
- [x] Add friend dialog (by username, distinct error toasts)
- [x] Friends view (tabs: All / Pending; accept/decline/cancel; realtime + toast on incoming)
- [x] Chat view: header (status + typing), message list (grouping, day separators, pagination, autoscroll)
- [x] Message item: hover actions, edit, soft delete, reactions (quick-8 popover, toggle chips)
- [x] Composer (Enter send / Shift+Enter newline, optimistic send)
- [x] Realtime wiring: messages, reactions, friendships, conversations, presence, typing broadcast
- [x] Unread tracking (conversation_reads upsert on view/focus)
- [x] Settings dialog (edit display name, logout)

### Phase 6 ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â Polish + verify
- [x] Empty states, skeletons, error toasts
- [x] `npx tsc --noEmit` + `npm run build` + `cargo check` clean
- [ ] E2E: two accounts (app + browser on Vite URL), friend request ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢ accept ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢ chat ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢ edit/delete ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢ reactions ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢ typing ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢ unread
- [ ] RLS negative test (third account can't read others' messages)
- [ ] Final advisors pass

### Phase 7 - Media storage + UI refinement (July 10)
- [x] Live migration applied: private chat-media bucket + public avatars bucket
- [x] Chat bucket restricted to WebP/WebM, 50 MiB/object, 3-day retention metadata
- [x] Strict 512 MiB chat-media budget with serialized 50 MiB reservations
- [x] Oldest-first cleanup runs before every upload reservation (expired objects first)
- [x] Avatar storage uses one permanent user-id/avatar.webp object with upsert (no history)
- [x] Local image/avatar WebP compression implemented before upload
- [x] Local video WebM compression implemented: 720p/30 fps maximum, adaptive bitrate, 120-second maximum
- [x] Attachment composer preview/progress, private signed rendering, and expired attachment state
- [x] Avatar upload UI + cache-busted public avatar rendering
- [x] Media CSP entries added to the production Tauri config
- [x] Sidebar active state and requested 1.25px chat/user-panel borders strengthened for Mica
- [x] Message sender name and header timestamp now use the same text size
- [x] Deploy purge-chat-media v2 with platform JWT verification enabled
- [x] Verify scheduled cleanup returns HTTP 200 (live request 8)
- [x] Install pinned @fontsource-variable/google-sans-flex@5.2.3 and switch the global font
- [x] Production Vite build + TypeScript + Cargo checks pass
- [ ] E2E attachment/avatar upload with two authenticated users
- [x] Re-run advisors; foreign-key indexes added; remaining notices are intentional RPC/private-table findings, the dashboard password setting, and fresh-db unused-index INFO


### Phase 8 - Local retention + live profile UX (July 10)
- [x] 30-day per-user local attachment cache in IndexedDB
- [x] Local cache expiry + oldest-first disk quota enforcement (1 GiB hard ceiling / 25% browser quota)
- [x] Received media is cached from signed URLs; sent compressed media is cached after successful insert
- [x] Server-purged media falls back to the local copy and shows a Saved locally badge
- [x] Chat settings show cache usage and allow clearing the current user's local cache
- [x] Profiles added to Supabase Realtime; avatar/display-name/color changes update both participants live
- [x] Constrained chat name colors added to profiles and rendered in chat/sidebar
- [x] Discord-style tabbed settings: My Account, Chat, Voice & Video
- [x] Persisted chat behavior and audio input/output preferences
- [x] Sidebar 16:9 black stream-preview placeholder above the user panel
- [x] Continued-message timestamps forced to a single line
- [x] Slightly raised surfaces, borders, foreground contrast, and consistent hover states


### Phase 10 - Shell refinement + component rollback (July 11)
- [x] Main surface fills the window with a 5px inset, 25px left corners, 5px right corners, and a 1.25px white/15% border
- [x] Experimental Figma component styling rolled back to the projectÃ¢â‚¬â„¢s simple shadcn controls, avatars, badges, and hover states
- [x] Mica restored as the default native material; a persisted Settings toggle switches to Acrylic live, with a 0-100% native black-tint slider and a continuous full-window material backdrop
- [x] Native minimize/maximize/close controls live inside the chat header rather than in an overlay
- [x] Chat header supports native window dragging without intercepting chat, voice, or window controls; transparent root lets Mica/Acrylic show through the sidebar
- [x] Compact channel tree and transparent user footer verified in the running Tauri app
- [x] Replaced malformed UI separator and ellipsis characters with ASCII-safe labels in chat and friends views
- [x] UI polish pass: richer empty states, composer send guidance, clearer Friends rows, and consistent shadcn soundboard cards
- [x] Production Vite build, TypeScript build, and Cargo check pass
### Phase 9 - Voice channels + soundboard (July 11)
- [x] Persistent Supabase voice rooms/participants, authenticated channel discovery, heartbeats, and stale-room cleanup
- [x] Direct WebRTC voice with perfect negotiation, device switching, mute/deafen, reconnect, and elapsed channel time
- [x] Screen sharing with remote sidebar PiP that stays hidden when no stream is active
- [x] Private Realtime signaling authorization scoped to 1:1 conversation participants
- [x] Private soundboard bucket and owner-only catalog deployed
- [x] Serialized soundboard limits: 96 MiB global, 16 MiB/user, 24 clips/user, 512 KiB/object
- [x] Local sound preparation: silence trim, normalization, mono 48 kHz Opus at 96 kbps, 15-second maximum
- [x] Live synchronized in-call soundboard playback through the existing private voice channel
- [x] Shared in-call library groups your clips and your chat partner's clips; either participant can play both
- [x] Bundled join, leave, mute/deafen toggle, and notification sounds from stuff/newsounds (July 13 MP3 refresh; enabled/disabled mappings corrected)
- [x] Persistent configurable voice keybinds and in-app keybind capture
- [x] Settings enlarged/reworked into Discord-style Account, Chat, Voice & Video, Keybinds, and Soundboard pages
- [x] Settings backdrop uses dimming only; no blur
- [x] Production Vite build + TypeScript + Cargo checks pass
- [ ] Two-client voice/screen-share/soundboard E2E on separate authenticated accounts
- [x] Final Supabase advisors reviewed; no new soundboard security or performance warnings
## Decisions & gotchas

- **RLS helpers live in `private` schema** (`private.is_participant(conv_id)`, `private.message_conversation(msg_id)`) ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â NOT `public` ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â so PostgREST doesn't expose them as RPC. `authenticated` has EXECUTE + USAGE on the schema (policies evaluate as the querying role).
- **Username is plain `text`**, not citext ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â check constraint forces `^[a-z0-9_]{3,20}$`; client must lowercase before insert/lookup.
- **No client INSERT policy on `conversations`** ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â rows are only created by the `handle_friendship_accepted` trigger (SECURITY DEFINER bypasses RLS).
- **Decline/cancel/unfriend = DELETE on friendships** (no 'declined' status). Accept = addressee updates status pendingÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢accepted.
- **`reactions` + `friendships` have `replica identity full`** so realtime DELETE events pass RLS authorization.
- Supabase key used is the modern `sb_publishable_...` key (works with supabase-js v2), stored under the `VITE_SUPABASE_ANON_KEY` env name.
- **RPCs for the client:** `public.username_available(text)` (anon-callable SECURITY DEFINER ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â intentional, signup-time check; the security advisor will WARN about it, that's accepted) and `public.conversation_overview()` (SECURITY INVOKER, sidebar previews + unread counts in one call).
- **Known realtime caveat:** postgres_changes DELETE events are not RLS-filtered by Supabase; with replica identity full the old row (uuids + emoji only, never message content) is visible to any authenticated subscriber of that table. Accepted for reactions/friendships.
- **Unfriending keeps the conversation + history** (Discord-like). Messages can still be sent after unfriend (participants keep insert rights).
- shadcn CLI is now v4-era: `init -b radix -p nova`; no `--base-color` flag anymore.
- `sonner.tsx` was edited to hardcode `theme="dark"` (next-themes removed).
- **July 10 UI refinement:** the sidebar user controls live in `features/chat/user-panel.tsx` as a 5px-inset solid panel; the chat surface uses 10px top-left rounding; the user panel uses 5px rounding; both use 1.25px borders, and voice controls are now live channel actions.

- **Free-tier storage guard:** Supabase currently includes 1 GiB of Storage on Free, not 5 GiB. Dislight deliberately budgets only 512 MiB for purgeable chat media, leaving headroom for avatars and metadata.
- **Chat media lifecycle:** chat-media is private. Clients can only read conversation media through signed URLs; writes use a short-lived signed upload token issued by purge-chat-media after participation, quota, and cleanup checks. Media expires after 3 days.
- **Strict quota concurrency:** private.chat_media_reservations reserves the full 50 MiB maximum under a Postgres advisory transaction lock. Actual Storage object sizes plus active reservations must remain below 512 MiB.
- **Avatar lifecycle:** avatars is public for display, but authenticated users can only write their user-id/avatar.webp object. Upload uses upsert, so a new avatar replaces the old object and there is no avatar history.
- **Video compression choice:** use WebView2 MediaRecorder (VP9/VP8 WebM) locally instead of silently bundling the machine's GPL FFmpeg build. Videos are resampled to at most 1280x720/720x1280 at 30 fps with an adaptive bitrate and 120-second cap.
- **Deployment checkpoint:** Edge Function v2 is live with platform JWT verification enabled. The hourly Vault/pg_cron request is active and a direct verification returned HTTP 200.
- **Local media retention:** IndexedDB keeps compressed attachments for 30 days from message creation, scoped by signed-in user id. The cache uses an oldest-first local quota (1 GiB hard ceiling and at most 25% of the browser-reported quota). Soft-deleting a message removes its local cached media.
- **Live profiles:** profiles is now in the Realtime publication. The client only updates known cached profiles plus the current auth profile, which makes avatar, display-name, and name-color updates visible without reloads.
- **Settings preferences:** chat density, Enter-to-send, media previews, input/output device ids, and volumes persist in localStorage. Voice device settings apply live while connected; microphone permission is requested only when joining a channel.

## How to run

```
npm install
npm run tauri dev     # desktop app
npm run dev           # frontend only, for testing a 2nd account in a browser
npx tsc --noEmit      # typecheck
```

- **Voice channels:** channels are joinable independently rather than ringing calls. Supabase persists/discovers membership; WebRTC carries media directly; private Realtime Broadcast carries signaling and synchronized soundboard events.
- **Voice takeover race (July 13):** Supabase Realtime omits non-primary-key fields from DELETE payloads under RLS, even with REPLICA IDENTITY FULL. The client now treats an active-room DELETE as tentative and verifies the current server lease before disconnecting; an old room/session event can no longer tear down a successful takeover. Intentional leave teardown also ignores its own DELETE event.
- **Soundboard budget:** the private bucket is limited to WebM/Opus at 512 KiB/object. Server-side advisory locking enforces 96 MiB global, 16 MiB/user, and 24 clips/user using actual Storage object sizes plus active reservations. This keeps it separate from the 512 MiB purgeable chat-media budget and leaves substantial headroom under Free's 1 GiB quota.
- **Soundboard privacy:** clients cannot write/delete bucket objects directly. The authenticated Edge Function issues narrow signed upload/download URLs, verifies active voice membership and conversation ownership before listing or playing either participant's clips, and uses 60-second playback URLs. Direct catalog RLS remains owner-only. The receiver validates the Supabase origin and rate-limits events.
- **Audio preparation:** uploads accept up to 25 MiB source audio, trim silence, reject clips over 15 seconds, normalize conservatively, resample to mono 48 kHz, and encode Opus/WebM at 96 kbps locally before any upload.
- **Keybind defaults:** Ctrl+Shift+M mute, Ctrl+Shift+D deafen, Ctrl+Shift+L leave, Ctrl+Shift+S screen share, Ctrl+Shift+B soundboard. They persist locally and are editable in Settings.
### Phase 11 - Message replies (July 13)
- [x] Persistent message replies: `messages.reply_to_message_id` has a self-reference with `ON DELETE SET NULL`
- [x] Server-side trigger rejects self-replies and cross-conversation reply references
- [x] Composer reply context with cancellation; replies support text, image, and video messages
- [x] Reply action, compact referenced-message preview, and in-view jump-to-original behavior
- [x] Reply targets hydrate for page loads and realtime inserts; target edits/deletions update their previews live
- [x] Moved the `Saved locally` media cache label beneath the attachment so it never obscures image/video content
- [x] Production Vite build + TypeScript check pass
### Phase 12 - Shell and preferences follow-up (July 13)
- [x] Removed the duplicate left chat-pane inset; sidebar spacing now provides the only separation
- [x] Window controls remain available in friends and empty-chat views
- [x] Composer shortcut guidance is dismissible and persisted locally
- [x] Preferences audit: chat behavior, material/tint, device ids, input/output volume, interface/soundboard volume, and keybinds persist in localStorage; active voice applies device/volume changes immediately
- [ ] Native Windows message notifications (separate Tauri notification-plugin task; no inactive preference is exposed yet)
- [ ] Click-through native stream overlay (requires a second Tauri overlay window and video-frame relay; sidebar live preview remains the supported stream viewer)

### Phase 13 - Live browser layout verification (July 13)
- [x] Created a disposable browser account and completed a real friend/conversation flow with `@tlgf`
- [x] Traced the message-avatar decoration drift to an auto-height wrapper plus an intrinsic image aspect ratio
- [x] Avatar decoration anchors now have explicit size-aware square wrappers and a 1:1 decoration layer; sidebar, header, and message instances all measure from the same avatar center
- [x] Verified live: no document overflow in the active conversation; settings uses `overflow-y: auto` with content larger than its viewport
- [x] Production Vite build and TypeScript check pass after the layout correction- [x] Avatar decoration playback now switches from the static frame to the animated asset while hovering a message avatar; user panel and chat header remain continuously animated
- [x] Online/away/offline avatar badges now use z-index 30, above the decoration layer (z-index 20), and were verified in the live browser DOM
- [x] Avatar decorations now animate when hovering anywhere within a message row, not only when the pointer is directly over the avatar
- [x] The newest message in the visible conversation is marked animated automatically, whether it was sent by the local user or their partner
- [x] Verified the above change with `npm run build` (TypeScript and Vite production build pass; existing chunk-size advisory only)
- [x] Newest-message autoplay resolves to the header avatar for its current grouped-message run, so it remains visible even when the final message omits a duplicate avatar
- [x] Decoration autoplay now applies to the latest message group for each participant, rather than only the conversation-wide latest message
- [x] Hovering any message in a grouped run now activates that run's header avatar decoration; verified with a passing production build
- [x] Composer overlay now has a higher stacking layer than avatar decorations, preventing decorations from rendering over the message input and its gradient
- [x] Consecutive messages now group only when they are from the same sender and less than one minute apart
- [x] Chat input surface now uses a subtle translucent card background with backdrop blur while retaining its existing border and focus treatment

### Release Candidate 1 (July 13)
- [x] RC1 is identified by Git tag `v0.1.0-rc.1`; Windows MSI packaging retains compatible product version `0.1.0` because WiX allows numeric-only prerelease identifiers
- [x] GitHub source published to `nostalg1cc/Chatterbox` on `main`
- [x] Vercel Production deployed successfully from Chatterbox commit `36c9188`; `https://dislight.vercel.app` returns HTTP 200
- [x] Final Windows RC1 installers built: MSI and NSIS
- [x] Vercel ignore rule scoped to root `Decorations/`; public decoration PNGs are included in web deployments and verified at `/decorations/bubble-still.png` (HTTP 200)

- **Soundboard latency (July 13):** Source change pending Edge Function deployment: opening the shared soundboard obtains 60-second signed preview URLs and warms the max-32 MiB in-memory clip cache; playback reuses any in-flight download, and the synchronized lead is 200 ms rather than 350 ms. This removes the first-download wait from normal click-to-play without persisting clips or widening storage access.

- **GIF avatars (July 13):** avatar uploads now retain a static WebP cover plus, for GIFs, a capped 1 MiB vatar.gif original. GIFs only render when an avatar would already animate (header/user panel, message hover, or each sender's latest message group); every other instance uses the cover frame.

- **Global voice shortcuts (July 13):** settings now persist an opt-in global mute/deafen switch. The official Tauri global-shortcut plugin is only registered in the native desktop build; web keeps app-focused keybinds.

- **Soundboard local playback (July 13):** sender and partner both play the same synchronized clip locally; playback now also honors the selected output device (with a Windows-default fallback).

- **Last chat restoration (July 13):** the last opened conversation is persisted locally per signed-in account and automatically restored after conversations load; invalid or removed conversations safely fall back to the normal empty state.

- **Animated GIF avatar optimization (July 13):** GIFs over the 1 MiB Storage cap are decoded and recompressed locally with progressively lower dimensions, frame sampling, and palette depth while retaining animation; only source files over 25 MiB or GIFs that still exceed the cap after every pass are rejected.

- **Motion stream + updater (July 14):** streams now target 1280x720 at 60 fps with an 8 Mbps ceiling, motion content hint, and frame-rate-first adaptation. Version 0.1.1 installs Tauri's signed updater; future desktop releases publish a signed NSIS EXE, its .sig, and updates/latest.json as the GitHub latest.json asset. The private signing key remains only at %USERPROFILE%\.tauri\dislight-updater.key and must never be committed or lost.

### Phase 11 - Floating navigation shell (July 14)
- [x] Created local rollback branch `backup/pre-header-sidebar-redesign-20260714` at `2d92465` before changing the shell
- [x] Disabled the persistent sidebar without deleting its implementation
- [x] Added a floating, blurred chat header with partner-triggered chat switcher, Friends, add-friend, and Chat/Media/Voice controls
- [x] Moved mute/deafen into the active voice controls and removed the duplicate sidebar leave action
- [x] Added a composer-adjacent Send as account popover with Settings and sign out
- [x] Moved the remote screen preview to an on-demand floating PiP beneath the header
- [x] Made the main chat surface a 5px-inset, 1.25px all-edge bordered pane; window controls fade in on their hover grace area
- [x] Reserved message-list space around the floating header/composer and preserved drag handling in the header/top grace area
- [x] TypeScript and production Vite build pass
- [ ] Authenticated visual/E2E pass of the new navigation flow
### Phase 14 - Visual consistency polish (July 14)
- [x] Removed duplicate online/offline wording from the floating header; the partner trigger now identifies the current channel and the adjacent status remains the single live status source
- [x] Converted chat switcher and Send as menu actions to vertical lists; shared Add friend and Settings triggers now support full-width menu actions
- [x] Added shared outer/surface/control geometry tokens (5px/8px/6px) and progressive Chromium `corner-shape: superellipse(...)` smoothing
- [x] Applied the shared geometry to buttons, fields, popovers, dialogs, chat header/composer, and the main shell
- [x] Increased grouped-message vertical rhythm: message rows receive more breathing room and sender-group starts are clearly separated
- [x] Browser verification: WebView2 reports `corner-shape: superellipse(1.25)` support; empty shell/header geometry renders without console errors
- [x] TypeScript and production Vite build pass; `git diff --check` passes- **July 14 header follow-up:** Floating header and composer now share the same 21px visual inset (5px wrapper + 16px inner padding). Presence lives under the partner name; the secondary status only appears for typing/media/voice states. Native window controls are always visible inside the header rather than reserved as an invisible right-side gap. Friends/empty header uses the same layout, and content clears its taller inset.
### Phase 15 - Figma-inspired conversation composition (July 14)
- [x] Inspected Figma frame `519:493` (`jKSbx77MuEBuHXTCbR2an7`) and used it as the visual reference for the active conversation view
- [x] Replaced the full-width chat header with a centered compact floating conversation dock; voice controls remain in the same dock and native window controls sit independently at the top-right
- [x] Reworked the no-chat/Friends fallback into the same compact dock, avoiding a different empty-state header language
- [x] Added a quiet charcoal/green conversation canvas, fine inset highlights, and a compact centered composer to create the intended spacious conversation hierarchy
- [x] Aligned local-user message groups to the right (including avatars, names, timestamps, hover actions, reactions, and media) while retaining left-aligned incoming groups and existing grouping/reply behavior
- [x] Production TypeScript/Vite build, diff whitespace check, and a live browser render pass completed without console errors
### Phase 16 - Hot soundboard and floating-surface follow-up (July 14)
- [x] Soundboard now warms the shared library immediately on voice-channel join for both participants; clip blobs and temporary URLs are retained in a session cache
- [x] Normal soundboard clicks no longer wait on the Supabase Edge Function: they broadcast the already-authorized hot clip over the existing private realtime voice socket and schedule both local players at the same near-immediate timestamp; the function remains a safe cache-miss/expired-URL recovery path
- [x] Unified dock, composer, and message-action surfaces under the same blurred material; fixed composer Send as padding and removed the composer bottom gradient entirely
- [x] Replaced desktop window buttons with compact macOS-style traffic lights, added black outer/inset-white shell framing, and added the requested green top radial while active in voice
- [x] Production TypeScript/Vite build, diff whitespace check, and live browser render pass completed without console errors
### Phase 17 - Dock hierarchy and blur framing (July 14)
- [x] Header is now a deliberate three-part rail: fixed-width partner chip, status chip only when context requires it, and a separated fixed-size action cluster; icon controls share a 36px target and Join voice remains a deliberate text control
- [x] Composer now restores a 21px floating inset and uses the same 15px rounded material geometry as the conversation dock
- [x] App frame is a thin 1px black outer edge plus 2px low-opacity white inset; no chat-canvas frame remains
- [x] Added transparent, mask-based progressive backdrop blur at the top and bottom of the message viewport without adding colour gradients
- [x] Production TypeScript/Vite build and diff whitespace checks pass
### Phase 18 - Surface primitive correction (July 14)
- [x] Dock, composer, and message action tray now share a single `surface-panel` recipe: 14px radius, 1.25px edge, inset highlight, material opacity, 24px blur, and shared shadow
- [x] Reworked chat-edge treatment to use a stronger backdrop blur behind intentional dark top/bottom scrims, replacing the hazy transparent-mask result
- [x] App edge frame is now rounded and clipped consistently, with a 1px black outer line and a 1px low-opacity white inset
- [x] Production TypeScript/Vite build and diff whitespace checks pass
### Phase 19 - Full surface-system convergence (July 14)
- [x] Applied the shared panel treatment to popovers, dropdown menus, context menus, dialogs, composer reply/media subpanels, and the existing dock/composer/message action tray
- [x] Added shared transient-menu item sizing and rounding so menu internals follow the same control language
- [x] Moved the app edge frame to a pointer-safe topmost pseudo-element so it remains visible above every dialog, PiP, and floating surface
- [x] Tuned message-edge overlays to explicit black-to-transparent top-down and bottom-up scrims behind the stronger blur rather than nearly opaque haze
- [x] Live browser shell/dialog render and production TypeScript/Vite build pass without console errors
### Phase 20 - Composer-style message action capsule (July 14)
- [x] Message hover actions (reaction, reply, edit, delete) now render as a compact attached capsule derived from the composer material rather than a full floating card
- [x] Standardized the action tray to 30px controls, 12px capsule geometry, shared 1.25px edge/inset highlight/blur, and restrained hover treatment
- [x] Production TypeScript/Vite build and diff whitespace checks pass
### Phase 21 — Native-edge frame correction (July 14)
- [x] Restored the custom application-edge frame at a restrained 10px corner radius: 1px black outer edge plus a subtle 1px white inset.
- [x] Kept that treatment on the actual application boundary only; the chat pane remains free of the oversized card-like frame.
- [x] Rebuilt successfully and restarted the Tauri development app so the current window uses the live Vite UI.
### Phase 22 — Controls and viewport fade correction (July 14)
- [x] Removed the visible “Send as” label; the composer account trigger is now avatar-only while retaining account/settings access.
- [x] Restored compact Windows-style minimize, maximize, and close controls in a hover-reveal top-right grace area.
- [x] Disabled the native window shadow to avoid a second Windows-looking outer frame while preserving the compact custom app-edge treatment.
- [x] Unified floating panels, popovers, dialogs, and action trays under a thin black edge, white inset, and blurred material.
- [x] Removed the opaque message blur overlays. The actual message viewport now uses a top-and-bottom transparency mask, so avatars, decorations, and media fade with the scroll content.
- [x] TypeScript, production Vite build, and whitespace verification pass; Tauri development app restarted.
### Phase 23 — App-edge corner rasterization fix (July 14)
- [x] Replaced the pseudo-element’s 1px border with rounded inset shadow rings inside a 10px clip, eliminating the sharp corner seam at the transparent app edge.
- [x] TypeScript, production Vite build, and diff whitespace verification pass.
### Phase 24 — Action visibility and native canvas fix (July 14)
- [x] Fixed message hover controls: the tray is hidden by default, revealed only on its message hover, and kept visible only while its reaction picker is open.
- [x] Disabled Tauri transparent-window rendering while retaining the configured native Mica/Acrylic material, removing the Acrylic layer that could escape the DOM’s 10px custom clip as a square edge.
- [x] Production TypeScript/Vite build and whitespace verification pass; Tauri development app restarted.
### Phase 25 — Acrylic restoration and full-root clip correction (July 14)
- [x] Restored `transparent: true`; Acrylic/Mica remains an available native material as requested.
- [x] Applied the 10px round clip to the full HTML/body/root paint chain and inset the custom ring by 1px, addressing the seam without removing Acrylic.
- [x] Production TypeScript/Vite build and whitespace verification pass; Tauri development app restarted.
### Phase 26 — Native compositor corner preference (July 14)
- [x] Verified and applied Windows `DWMWA_WINDOW_CORNER_PREFERENCE` with the small-round setting through the Tauri native HWND at startup.
- [x] This targets the actual Acrylic/Mica compositor surface; CSS clipping alone cannot round that OS-level surface.
- [x] Cargo check, TypeScript, production Vite build, and whitespace verification pass; native Tauri dev window restarted.
### Phase 27 — v0.1.2 release (July 14)
- [x] Versioned, built, and published Dislight v0.1.2 from GitHub main.
- [x] Release assets: signed NSIS installer, `.sig`, MSI, and updater `latest.json` manifest.
- [x] GitHub repository visibility set to public so installer downloads and the configured in-app updater endpoint are reachable (HTTP 200 verified).
- [x] Source commits: `eb423f9` and `6c957cf`; release tag `v0.1.2`.