# Dislight ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â Agent Handoff Document

> **Purpose:** This file is the single source of truth for cross-agent handoff (Claude Code ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â Codex).
> Whoever works on this project: **update the checklist below as you complete work**, and add notes
> in "Decisions & gotchas" when you make a non-obvious choice. Keep it current after every phase.

## What is this project?

**Dislight** ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â a Windows desktop 1-on-1 chat app (Discord-inspired, but no servers/guilds; strictly DM-focused).
Sleek, minimal, **dark-only** UI: Mica by default (Acrylic optional) for the transparent sidebar/window frame, plus a bordered near-black chat surface.

## Stack

- **Shell:** Tauri 2.x ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â€šÂ¬Ã‚Â `decorations: false`, `transparent: false`, custom overlaid titlebar, solid black native window background
- **Frontend:** React 19 + TypeScript + Vite, Tailwind CSS v4, shadcn/ui, dark-only, bundled Google Sans Flex
- **State:** Zustand stores (`src/stores/`) + Supabase Realtime subscriptions pushing into stores. No react-query.
- **Backend:** Supabase (auth + Postgres/RLS + realtime). No custom server. Rust surface is minimal (window effects only).
- **Package manager:** npm (pnpm not installed on this machine)
- **Routing:** none ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â top-level state machine in `App.tsx`: `booting ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â‚¬Å¾Ã‚Â¢ auth ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â‚¬Å¾Ã‚Â¢ main`

## Supabase

- **Org:** "Nate's projects" (`vercel_icfg_oxyNEBfLgvm7JSw54EpiKrX9`)
- **Project:** `dislight` ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â ref `lapjrxdgcbdseskmyfru`, region `eu-north-1`, free tier ($0/month)
- **DO NOT TOUCH** the other project `supabase-red-village` (`wqxtjrkvdhitcduztcju`)
- Client env vars in `.env` (gitignored): `VITE_SUPABASE_URL`, `VITE_SUPABASE_ANON_KEY` (see `.env.example`)
- Migrations are applied via Supabase MCP `apply_migration` AND mirrored in `supabase/migrations/*.sql` (repo copy is the reference; keep both in sync)
- **Email confirmation:** new projects default to ON. App handles both states. User can disable in Dashboard ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â‚¬Å¾Ã‚Â¢ Authentication ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â‚¬Å¾Ã‚Â¢ Sign In / Up ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â‚¬Å¾Ã‚Â¢ Email ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â‚¬Å¾Ã‚Â¢ "Confirm email" for instant signup in dev.

### Schema (all tables RLS-enabled, in `public`)

| Table | Purpose | Key points |
|---|---|---|
| profiles | user profile | auth user id; username unique; live avatar metadata + constrained name color |
| `friendships` | requests + friends | `requester_id`, `addressee_id`, `status` enum pending/accepted/blocked; unique on ordered pair |
| `conversations` | 1:1 threads | `user1_id < user2_id` ordered pair, unique; auto-created by trigger when friendship ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â‚¬Å¾Ã‚Â¢ accepted; `last_message_at` bumped by message trigger |
| messages | chat messages | soft delete/edit; optional WebP/WebM attachment metadata; server media expires after 3 days |
| `reactions` | emoji reactions | unique (message_id, user_id, emoji); emoji ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â°ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¤ 8 chars |
| `conversation_reads` | unread tracking | PK (conversation_id, user_id), `last_read_at` |

- Helper: `public.is_participant(conv_id uuid)` ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â SECURITY DEFINER, used by policies to avoid recursion
- Realtime publication includes messages, reactions, friendships, conversations, and profiles
- Presence/typing: Supabase Realtime channels (presence channel `online`, broadcast `typing:{conversation_id}`), no tables

## Layout / design language

- Grid: `[titlebar 32px] / [sidebar 280px | chat pane]`
- Mica shows through **titlebar + sidebar** (transparent layers); **chat pane is solid** `bg-background` (this is native Win11 style ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â  la File Explorer ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â NOT glassmorphism; do not add blur/gradients)
- Near-black zinc tokens, raised hairline borders, bundled Google Sans Flex with Segoe UI Variable fallback, lucide icons
- Destructive red is the only accent color

## Checklist (keep updated!)

### Phase 0 ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â Handoff
- [x] AGENTS-HANDOFF.md + AGENTS.md created

### Phase 1 ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â Supabase backend
- [x] Project created (`lapjrxdgcbdseskmyfru`)
- [x] Migration: schema (tables, enum, triggers, indexes) ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â `initial_schema`
- [x] Migration: RLS policies + helpers ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â `rls_policies`
- [x] Migration: realtime publication + replica identity full on reactions/friendships ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â `realtime_publication`
- [x] Migration: helpers moved to `private` schema, trigger fn EXECUTE revoked ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â `lock_down_function_exposure`
- [x] Advisors: security = 0 findings; performance = only "unused index" INFO (expected, fresh DB)
- [x] `.env` + `.env.example` written (URL + `sb_publishable_...` key in `VITE_SUPABASE_ANON_KEY`)
- [x] Migrations mirrored to `supabase/migrations/`

### Phase 2 ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â Scaffold
- [x] Vite + React + TS scaffold (hand-written, in repo root; create-vite refused non-empty dir)
- [x] Tailwind v4 + shadcn init (radix base, **nova preset**, Geist font bundled via @fontsource) + 19 components in `src/components/ui/`
- [x] Tauri init: undecorated/transparent/shadow/dark window, strict prod CSP (devCsp null for HMR), window-control capabilities
- [x] Rust: window-vibrancy `apply_mica` in setup + `is_mica_supported` command (cargo check passes)
- [x] git init + .gitignore + initial commit

### Phase 3 ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â Window shell
- [x] `components/titlebar.tsx` (drag region, min/max/close, hides outside Tauri)
- [x] App layout grid, no window scrollbars

### Phase 4 ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â Auth + boot
- [x] `lib/supabase.ts` client
- [x] Boot screen + session restore + onAuthStateChange routing
- [x] Login form (email+password)
- [x] Signup form (username availability check, display name, verify-email state + resend)
- [x] `stores/auth.ts`

### Phase 5 ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â Main app
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

### Phase 6 ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â Polish + verify
- [x] Empty states, skeletons, error toasts
- [x] `npx tsc --noEmit` + `npm run build` + `cargo check` clean
- [ ] E2E: two accounts (app + browser on Vite URL), friend request ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â‚¬Å¾Ã‚Â¢ accept ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â‚¬Å¾Ã‚Â¢ chat ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â‚¬Å¾Ã‚Â¢ edit/delete ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â‚¬Å¾Ã‚Â¢ reactions ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â‚¬Å¾Ã‚Â¢ typing ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â‚¬Å¾Ã‚Â¢ unread
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
- [x] Experimental Figma component styling rolled back to the projectÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â‚¬Å¾Ã‚Â¢s simple shadcn controls, avatars, badges, and hover states
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

- **RLS helpers live in `private` schema** (`private.is_participant(conv_id)`, `private.message_conversation(msg_id)`) ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â NOT `public` ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â so PostgREST doesn't expose them as RPC. `authenticated` has EXECUTE + USAGE on the schema (policies evaluate as the querying role).
- **Username is plain `text`**, not citext ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â check constraint forces `^[a-z0-9_]{3,20}$`; client must lowercase before insert/lookup.
- **No client INSERT policy on `conversations`** ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â rows are only created by the `handle_friendship_accepted` trigger (SECURITY DEFINER bypasses RLS).
- **Decline/cancel/unfriend = DELETE on friendships** (no 'declined' status). Accept = addressee updates status pendingÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â‚¬Å¾Ã‚Â¢accepted.
- **`reactions` + `friendships` have `replica identity full`** so realtime DELETE events pass RLS authorization.
- Supabase key used is the modern `sb_publishable_...` key (works with supabase-js v2), stored under the `VITE_SUPABASE_ANON_KEY` env name.
- **RPCs for the client:** `public.username_available(text)` (anon-callable SECURITY DEFINER ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â intentional, signup-time check; the security advisor will WARN about it, that's accepted) and `public.conversation_overview()` (SECURITY INVOKER, sidebar previews + unread counts in one call).
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
### Phase 21 Ã¢â‚¬â€ Native-edge frame correction (July 14)
- [x] Restored the custom application-edge frame at a restrained 10px corner radius: 1px black outer edge plus a subtle 1px white inset.
- [x] Kept that treatment on the actual application boundary only; the chat pane remains free of the oversized card-like frame.
- [x] Rebuilt successfully and restarted the Tauri development app so the current window uses the live Vite UI.
### Phase 22 Ã¢â‚¬â€ Controls and viewport fade correction (July 14)
- [x] Removed the visible Ã¢â‚¬Å“Send asÃ¢â‚¬Â label; the composer account trigger is now avatar-only while retaining account/settings access.
- [x] Restored compact Windows-style minimize, maximize, and close controls in a hover-reveal top-right grace area.
- [x] Disabled the native window shadow to avoid a second Windows-looking outer frame while preserving the compact custom app-edge treatment.
- [x] Unified floating panels, popovers, dialogs, and action trays under a thin black edge, white inset, and blurred material.
- [x] Removed the opaque message blur overlays. The actual message viewport now uses a top-and-bottom transparency mask, so avatars, decorations, and media fade with the scroll content.
- [x] TypeScript, production Vite build, and whitespace verification pass; Tauri development app restarted.
### Phase 23 Ã¢â‚¬â€ App-edge corner rasterization fix (July 14)
- [x] Replaced the pseudo-elementÃ¢â‚¬â„¢s 1px border with rounded inset shadow rings inside a 10px clip, eliminating the sharp corner seam at the transparent app edge.
- [x] TypeScript, production Vite build, and diff whitespace verification pass.
### Phase 24 Ã¢â‚¬â€ Action visibility and native canvas fix (July 14)
- [x] Fixed message hover controls: the tray is hidden by default, revealed only on its message hover, and kept visible only while its reaction picker is open.
- [x] Disabled Tauri transparent-window rendering while retaining the configured native Mica/Acrylic material, removing the Acrylic layer that could escape the DOMÃ¢â‚¬â„¢s 10px custom clip as a square edge.
- [x] Production TypeScript/Vite build and whitespace verification pass; Tauri development app restarted.
### Phase 25 Ã¢â‚¬â€ Acrylic restoration and full-root clip correction (July 14)
- [x] Restored `transparent: true`; Acrylic/Mica remains an available native material as requested.
- [x] Applied the 10px round clip to the full HTML/body/root paint chain and inset the custom ring by 1px, addressing the seam without removing Acrylic.
- [x] Production TypeScript/Vite build and whitespace verification pass; Tauri development app restarted.
### Phase 26 Ã¢â‚¬â€ Native compositor corner preference (July 14)
- [x] Verified and applied Windows `DWMWA_WINDOW_CORNER_PREFERENCE` with the small-round setting through the Tauri native HWND at startup.
- [x] This targets the actual Acrylic/Mica compositor surface; CSS clipping alone cannot round that OS-level surface.
- [x] Cargo check, TypeScript, production Vite build, and whitespace verification pass; native Tauri dev window restarted.
### Phase 27 Ã¢â‚¬â€ v0.1.2 release (July 14)
- [x] Versioned, built, and published Dislight v0.1.2 from GitHub main.
- [x] Release assets: signed NSIS installer, `.sig`, MSI, and updater `latest.json` manifest.
- [x] GitHub repository visibility set to public so installer downloads and the configured in-app updater endpoint are reachable (HTTP 200 verified).
- [x] Source commits: `eb423f9` and `6c957cf`; release tag `v0.1.2`.
### Phase 28 Ã¢â‚¬â€ Vercel production deployment repair (July 14)
- [x] Removed the unused `shadcn` CLI package and its build-time stylesheet import; the app already owns its local component sources and theme tokens.
- [x] Pinned `radix-ui` to installable `1.6.1`, avoiding the subsequently unavailable internal Radix tarballs referenced by the floating `1.6.2` dependency tree.
- [x] Confirmed a local production TypeScript/Vite build and a clean Vercel install/build.
- [x] Deployed current `main` to Vercel production: `https://dislight.vercel.app` (deployment `dpl_A4TxqnHbqj2uwicK2F9R6KjtwVYR`, READY).
### Phase 29 Ã¢â‚¬â€ Dislight brand assets + material polish (July 14)
- [x] Generated the full Tauri icon family from `stuff/logo.png`: Windows ICO/taskbar, NSIS/MSI installer, Store/AppX, PNG, macOS, Android, and iOS outputs.
- [x] Configured the 32/64/128/256/ICO native asset set and added the matching web favicon asset.
- [x] Raised the release to v0.1.3 for the icon-bearing installer and updater.
- [x] Reworked the shared floating material so the header, composer, message reaction tray, popovers, dropdowns, context menus, and dialogs all visibly use the same translucent backdrop blur.
- [x] Production TypeScript/Vite build and Cargo check pass.- [x] Preserved composer focus across controlled input updates and vertically centered its single-line placeholder/action row before v0.1.3 packaging.

### Phase 30 Ã¢â‚¬â€ v0.1.3 branded installer release (July 14)
- [x] Built Windows NSIS and MSI artifacts with the new Dislight application icon.
- [x] Signed the NSIS installer with the existing updater key and generated a v0.1.3 `latest.json` manifest for the in-app updater.
- [x] Packaged after the composer focus/vertical alignment correction and shared backdrop-blur refinement.
### Phase 31 Ã¢â‚¬â€ Username typography presets (July 14)
- [x] Added constrained, live-persisted profile typography: Sans, Rounded, Serif, or Mono; Regular, Medium, Bold, or Black.
- [x] Completed the existing name-effect system so animated effects now run for active header/user-panel contexts and the newest or hovered message group.
- [x] Added matching settings controls, mirrored migration `20260714233000_profile_name_typography.sql`, and applied the migration to the live Supabase project.
- [x] Verified live profile columns and production TypeScript/Vite build.### Phase 32 Ã¢â‚¬â€ Real Sera UI text-effect renderers (July 14)
- [x] Replaced the former CSS-only name-effect placeholders with local React renderers adapted from Sera UI's public Fuzzy, Sparkles, Resize Handle, Bouncy, Wavy, Gradient, Glitch, and Particle components.
- [x] Kept static names inexpensive; only intentionally active contexts instantiate canvases, per-letter motion, sparkles, particles, or glitch layers.
- [x] Verified production TypeScript/Vite build and a clean live-app console after Vite HMR.
### Phase 33 Ã¢â‚¬â€ Always-on name effects (July 14)
- [x] Username effects are now continuous in every rendered context; they no longer wait for hover or the latest message group.
### Phase 34 Ã¢â‚¬â€ v0.1.5 updater-key reset release (July 15)
- [x] Rotated the lost updater signing key and configured the replacement public key in Tauri.
- [x] Built signed NSIS/MSI artifacts and updater signatures for 0.1.5.
- [x] 0.1.5 requires one manual install from 0.1.3 because the updater signing identity changed; future releases will update normally from 0.1.5.
### Phase 35 Ã¢â‚¬â€ v0.1.6 surface/composer/link hotfix (July 15)
- [x] Replaced unreliable WebView2 backdrop filtering with opaque dark-gray floating header, composer, menus, dialogs, and message-action surfaces.
- [x] Composer now recalculates after its controlled value clears, so a sent multiline message immediately returns to single-line height.
- [x] Added safe `http`/`https` link rendering and Tauri's scoped default-browser opener permission; regular text remains escaped React text.
- [x] Production web build, Cargo check, signed NSIS/MSI packages, and updater signatures pass.


### Phase 36 - Web mobile + PWA (July 15)
- [x] Added a browser-only installable PWA manifest, icon metadata, and a conservative service worker for app-shell resilience
- [x] Added safe-area-aware responsive layout rules for phones, touch-friendly composer controls, responsive media, and compact floating navigation
- [x] Kept the desktop shell and desktop release version unchanged; this is a web-only Vercel update


### Phase 37 - Soundboard storage-first management (July 15)
- [x] Repaired mojibake in the Soundboard settings copy and sound metadata rows
- [x] Removed the 24-clip quota from the live reservation function; 16 MiB per user and 96 MiB shared storage limits remain enforced
- [x] Added a sound-storage progress bar, per-sound prepared size, and inline rename controls
- [x] Added authenticated owner-only sound rename RPC; live verification confirms the count cap is removed and anonymous callers have no execute permission
- [ ] Supabase Edge Function deployment for the matching storage-only quota error copy returned a platform internal 500; local function source is ready for a later retry


### Phase 38 - v0.1.7 soundboard desktop hotfix (July 15)
- [x] Versioned the desktop release for the soundboard storage-first controls and encoding repair
- [x] Built signed NSIS/MSI + updater manifest for v0.1.7
- [x] Published GitHub v0.1.7 with signed NSIS/MSI artifacts, updater manifest, and signature files
- [x] Deployed v0.1.7 to Vercel production and verified the updater manifest returns version 0.1.7

### Phase 39 - Soundboard controls and Settings resilience (July 15)
- [x] Fixed sound rename state handling so a successful RPC response no longer inserts an undefined sound entry or temporarily whitescreens the app
- [x] Added local persisted star pins for own and partner sounds; pins sort first in the shared in-call soundboard and can be toggled off directly there
- [x] Added a Settings sound preview control plus an authenticated owner-only preview path in the Edge Function source
- [x] Fixed the voice elapsed-status column to a stable fixed width and elevated the floating header above chat media for reliable dragging
- [x] Updated Settings framing to use the shared panel treatment and added an About tab with version and update checks
- [ ] Edge Function preview deployment remains blocked by the Supabase platform returning an internal 500; retry later from CLI or dashboard once the service recovers

### Phase 40 - Expandable live screen share (July 15)
- [x] Screen-share PiP now toggles between a compact top-right preview and a canvas view bounded by the floating header and composer
- [x] Kept a visible fullscreen action in canvas view and prevented stream clicks from pausing the live video

### Phase 41 - v0.1.8 updater identity recovery (July 15)
- [x] Restored passwordless local signing key storage at `C:\Users\nrohd\.tauri\dislight-updater.key` and the user-scoped signing environment
- [x] Rotated the embedded updater public key to match that recovered key; 0.1.8 requires a one-time manual install from 0.1.7
- [x] Built and signed the 0.1.8 NSIS/MSI artifacts; future releases can use the restored key without another recovery step

### Phase 42 - v0.1.8 published release (July 15)
- [x] Published GitHub release `v0.1.8` with signed NSIS/MSI artifacts and the verified `latest.json` updater manifest
- [x] Deployed the matching web build to Vercel production (`https://dislight.vercel.app`); HTTP 200 verified
- [x] Verified the configured GitHub latest-manifest endpoint returns version 0.1.8

### Phase 43 - Header media drag ownership (July 15)
- [x] Raised and isolated the floating chat header above all message and screen-share media layers, preserving its dedicated native drag handler while controls remain interactive.


### Phase 44 - Remote screen-share teardown (July 15)
- [x] Clear the partner PiP from the authoritative sharing_screen room-presence update, covering WebRTC sender removal paths that leave a remote video track on its final frame.
### Phase 45 - Voice header hierarchy (July 15)
- [x] Widened the fixed elapsed-call status column to avoid clipping without timer-driven layout jitter, and added labeled neutral Voicechat, green Join voice, and red Leave voice primary controls.
### Phase 46 - v0.1.9 voice polish release (July 15)
- [x] Built and manually signed NSIS/MSI artifacts after Tauri's build process could not read the passwordless signing key environment.
- [x] Published GitHub release 0.1.9 with installers, signatures, and verified updater manifest.
- [x] Deployed the matching web build to Vercel production (https://dislight.vercel.app).

### Phase 47 - My Account live profile preview (July 16)
- [x] Added a sticky live account preview so avatar decoration, display name, name color, font, weight, and text effect render immediately while their selectors are changed.
- [x] The preview remains visible while scrolling the account form and uses the same animated avatar/text components as the chat UI.
- [x] Verified with `npx tsc --noEmit` and `git diff --check`.

### Phase 48 - Cloud decoration catalogue and picker (July 16)
- [x] Replaced the old user-facing 20-item decoration list with a 639-item manifest-backed catalogue.
- [x] Added a searchable, lazy-loading picker in My Account with an internal scroll area, so the account form and Save action never sit below an endless thumbnail grid.
- [x] Added Cloudinary on-demand delivery; the first 70 assets are imported and unimported entries temporarily fall back to their original public source URL while staged import continues.
- [x] Kept legacy local files as a compatibility fallback for existing profile selections; they are no longer shown in the picker.
- [x] Verified type checking, diff whitespace, HTTP 200 dev server, and a running native development window.

### Phase 49 - Cloud decoration profile persistence (July 16)
- [x] Fixed profile saves for the new avatar-decoration catalogue: the production `profiles_avatar_decoration_check` now accepts three-digit catalogue IDs as well as legacy values.
- [x] Created and applied `20260716174037_allow_cloud_avatar_decorations.sql`; verified the live constraint and retained ownership-scoped `profiles_update_own` RLS policy.
- [x] Supabase security advisor was checked after DDL; reported warnings predate this change and concern existing RPC functions/Auth protection, not profile-decoration access.

### Phase 50 - Cloud decoration initial live set (July 16)
- [x] Reset the Cloudinary decoration folder and uploaded only IDs 001�010 from the source manifest.
- [x] Restricted the app picker and delivery layer to those ten Cloudinary-hosted APNG decorations; removed local/original-source fallback from selectable items.

### Phase 51 - Live Cloudinary decoration discovery (July 16)
- [x] Added Cloudinary-tag catalogue discovery to the decoration picker, with a shipped fallback for the initial set and a 65-second refresh while My Account is open.
- [x] Selection/display always delivers the APNG original directly from Cloudinary so active contexts retain animation; picker thumbnails remain static first frames.
- [x] Ignored `stuff/new deco/` so the 639 source APNG files are never committed or packaged.
- [ ] Enable Cloudinary Settings > Security > Restricted media types > Resource list before the public tag list endpoint can serve live additions/removals to clients.

### Phase 52 - Complete Cloud avatar-decoration import (July 16)
- [x] Imported and reconciled all 639 APNG decorations to Cloudinary under `dislight/avatar-decorations`.
- [x] Verified the hosted catalogue contains exactly IDs 001�639, with no missing IDs or extras.
- [x] The raw `stuff/new deco/` sources remain Git-ignored; all clients use Cloudinary delivery instead.

- [x] Voice capture: fixed noise suppression argument propagation; off is now the safe default, with echo cancellation and automatic gain disabled.

- [x] Chat initial load: retry scroll-to-latest after the real scroll viewport mounts.
- [x] Voice settings: local microphone monitor test using the production voice capture pipeline and selected output device.

- [x] Header controls: fixed 3x36px geometry by removing accidental flex gaps; initial chat scroll now also follows late content resizing.

- [x] Message fade: mask now applies directly to the scroll viewport; top fade increased to 136px.

- [x] Message fade: increased top viewport fade again; content reaches full opacity at 224px.

- [x] Global voice keybinds: replaced cleanup-race registration with generation-safe native shortcut reconciliation; mute/deafen each use a dedicated native handler.

- [!] 0.1.10 release: installer/MSI built. GitHub updater signing is pending the password for the configured C288F8A63AA90C24 key; do not rotate the updater pubkey to an older signing key.

- [x] 0.1.11 hotfix: Tauri CSP now permits Cloudinary image delivery and live catalogue fetches; packaged decoration picker loads all live entries instead of ten blocked fallback previews.

- [x] Voice VPN connection guard: ICE setup now times out, attempts one restart, then shows the existing TURN-relay error instead of remaining in Connecting forever. A real TURN service is still required for VPN/restrictive-network media relay.

### Phase 53 - Cloudflare TURN fallback and voice overdrive (July 17)
- [x] Added a JWT-protected Supabase `realtime-credentials` broker that validates an active voice participant before issuing 12-hour Cloudflare TURN credentials; credentials never enter the bundle or repository.
- [x] Voice now preserves direct P2P as its default and retries failed ICE with fresh relay credentials when available; it remains direct-only if relay secrets are not configured.
- [x] Raised Settings output volume to 300% via a local Web Audio gain stage, with normal 100% behavior unchanged and a clipping warning for boosted output.
- [ ] Add `CLOUDFLARE_TURN_KEY_ID` and `CLOUDFLARE_TURN_KEY_SECRET` as Supabase Edge Function secrets before TURN fallback can become active in production.
- [ ] Cloudflare SFU screen-share route requires the supplied Calls App credentials to be stored as server-side secrets and Cloudflare account API reauthorization for app-side verification; do not place either token in the client.

- [x] Deployed authenticated Cloudflare Calls broker (`cloudflare-realtime`) and routed screen-share publishing/subscription through the SFU, with the existing direct screen route retained as automatic compatibility fallback.

### Phase 54 - Composer clipboard media and focus (July 18)
- [x] Added image/video paste support in chat. Clipboard files now use the existing local WebP/WebM preparation and attachment pipeline before send.
- [x] Restored composer focus after a successful message send, including attachment sends, so typing can continue immediately.
- [x] Verified TypeScript and whitespace checks.- [x] Added a protected drag-and-drop target on the composer for images and videos, reusing the same preparation pipeline and preventing file drops from navigating the WebView.
### Phase 55 - v0.1.15 chat media release (July 18)
- [x] Released a standalone Windows NSIS installer with chat clipboard paste, composer drag-and-drop, local attachment compression, and composer focus retention.
- [x] Published GitHub release `v0.1.15` with `Dislight_0.1.15_x64-setup.exe`.
- [!] The installer is intentionally standalone: updater signature generation remains blocked by the configured private-key password mismatch.
### Phase 56 - Screen-share reliability, preview, and audio (July 19)
- [x] Diagnosed the live screen-share failure: Cloudflare publisher requests succeeded, but the partner subscription returned 502, leaving no usable media path.
- [x] Kept direct WebRTC screen tracks active as the reliable route even when Cloudflare publish/subscribe is attempted, so an SFU failure cannot leave a false Streaming state.
- [x] Added separate own (left) and partner (right) screen previews, each with compact, expanded, and fullscreen views.
- [x] Requested display-capture audio and mix received screen audio with voice rather than replacing it; sources that do not offer audio now explain how to enable it in the picker.
- [x] Verified TypeScript, whitespace, and production build.
### Phase 57 - Soundboard playback surface (July 19)
- [x] Reworked in-call sound rows into a clear transport layout: real play/pause control at left, star pin at right, and a subtle live playback-progress fill behind the row.
- [x] Added local pause/resume state for the playing clip while retaining the existing broadcast-on-initial-play behavior for the partner.
- [x] Verified TypeScript, whitespace, and production build.- [x] Refined the Phase 57 transport after review: the active control is now Stop/reset (not Pause), the progress fill is a sharp white 15% bar with no gradient, and clips use a compact two-column grid.
### Phase 58 - Persistent soundboard cache and smooth progress (July 19)
- [x] Added a durable IndexedDB sound cache with a strict 16 MiB budget; cache hits survive app restarts and are warmed automatically when a voice room's shared soundboard loads.
- [x] Cache eviction is usage-aware: frequently played clips are retained first, then oldest least-used clips are removed only when required. Clips outside the durable budget remain available in the short-lived 32 MiB session cache.
- [x] Replaced coarse `timeupdate` progress with a requestAnimationFrame playback clock backed by each sound's known duration, so the progress fill starts immediately and advances smoothly.
- [x] Verified TypeScript and the production web build.
- [x] Removed the numeric percentage label and moved the fill to GPU-friendly linear transform interpolation for visibly smoother progress.
### Phase 59 - Signed silent updater (July 19)
- [x] Replaced the stalled manual update toast with a signed Tauri updater flow: desktop startup checks, downloads, installs quietly per-user, then restarts in place.
- [x] Added a functional About & Updates surface: manual check, version availability, install action, download progress, and clean restart handoff.
- [x] Created a new password-protected signing identity and a Git-ignored root recovery file (`UPDATER-RECOVERY.local.txt`) containing the encrypted key backup, password, public key, and endpoint. Never commit or share it.
- [x] Configured NSIS current-user installation and updater `quiet` mode; the new identity means v0.1.16 is the one-time manual bridge release, and later releases can update it in place.
- [x] Built signed v0.1.16 NSIS installer and `.sig` updater artifact from the final source.
- [x] Published GitHub release `v0.1.16` with the NSIS installer, signature, and live `latest.json` manifest; verified the public endpoint returns the expected version, URL, and signature.
### Phase 60 - Updater validation bridge (July 19)
- [x] Prepared v0.1.17 as a signed updater test release so an installed v0.1.16 client can exercise the complete real update path.
- [x] Added the actual updater failure message to About & Updates, so any remaining check/install failure is diagnosable without guessing from a generic toast.
- [x] Published signed GitHub release `v0.1.17` with a verified live manifest and installer URL; it is the controlled update target for v0.1.16.
### Phase 61 - Updater manifest encoding hotfix (July 19)
- [x] Identified a UTF-8 BOM in the original GitHub `latest.json` asset as the likely native updater parse failure.
- [x] Release manifests are now written as clean UTF-8 without a BOM. v0.1.18 is the fresh-URL bridge release, avoiding GitHub CDN retention of the previously uploaded v0.1.17 asset.
- [x] Published signed GitHub release `v0.1.18`; verified its public `latest.json` begins without a BOM, parses as valid JSON, and references a reachable signed installer.
### Updater release runbook — do not skip
- The first working chain is **v0.1.16 → v0.1.18**: update checks, quiet current-user installation, restart, signature verification, and GitHub delivery were all user-confirmed working on July 19.
- Use the signing identity recorded only in the Git-ignored root `UPDATER-RECOVERY.local.txt`. It contains `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, the public key, endpoint, and encrypted private-key backup. Never commit, log, paste, rotate, or expose that file.
- Every desktop release must: bump the matching version in `package.json`, `package-lock.json` root metadata, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json`; set both signing environment variables from the recovery file; run `npm run tauri build -- --bundles nsis`; publish the generated NSIS `.exe` **and** `.exe.sig`.
- Generate `latest.json` from the generated `.sig`, with `version`, `notes`, `pub_date`, and `platforms.windows-x86_64.url/signature`. Write it with `new Text.UTF8Encoding($false)` — **never** Windows PowerShell `Set-Content -Encoding utf8`, which writes a BOM and makes the native Tauri updater fail parsing.
- Upload an asset actually named `latest.json` to the newest non-prerelease GitHub release. Verify via `curl -L` that the first bytes are `7B 0D 0A` or another normal JSON start (never `EF BB BF`), that the manifest version is correct, that a signature exists, and that the installer URL returns HTTP 200.
- Updater configuration is intentionally current-user NSIS plus updater Windows `quiet` mode: updates replace the app in place and restart without installer UI or elevation prompts. Do not change the configured public key: existing installs trust it.
### Phase 62 - Expanded screen-share cleanup (July 19)
- [x] Removed both redundant expanded-PiP helper prompts; the stream remains clickable for compact/expanded switching and retains the fullscreen control.
### Phase 63 - Cloudflare screen-share negotiation repair (July 26)
- [x] Diagnosed a successful Cloudflare session creation that did not progress to a usable published track; the prior publisher attached display media before creating/connecting its Calls session, which is outside Cloudflare's documented session-then-track lifecycle.
- [x] Reordered the publisher and subscriber flow: create session, wait for WebRTC connectivity, then publish/subscribe; added Cloudflare STUN and more reliable ICE gathering waits. Direct P2P screen tracks remain the compatibility path if SFU negotiation fails.
- [x] Verified `npx tsc --noEmit`.
### Phase 64 - v0.1.19 screen-share repair release (July 26)
- [x] Built the signed v0.1.19 NSIS installer and matching updater signature after the Cloudflare negotiation repair.
- [x] Published GitHub release `v0.1.19` with the installer, `.sig`, and clean no-BOM `latest.json` updater manifest.
- [x] Deployed the matching production web build to Vercel and verified the deployment is ready on commit `d00f931`.
### Phase 65 - Durable voice-session status entries (July 26)
- [x] Added database-owned `voice_started` and `voice_ended` chat status rows. A room start posts one status; only the final deliberate participant leave posts `Call lasted M:SS` (or H:MM:SS), so a one-sided leave or reconnect does not falsely end the session.
- [x] Added a compact centered status treatment in the message timeline, with a timestamp and no avatar/actions; status rows always break normal message grouping.
- [x] Applied `20260726155712_voice_call_status_events.sql` to Supabase, verified the new columns/functions, and confirmed no new security-advisor warnings.
- [x] Verified TypeScript and the production web build.
### Phase 66 - v0.1.20 voice session status release (July 26)
- [x] Built and signed the v0.1.20 NSIS installer.
- [x] Published GitHub release `v0.1.20` with the installer, signature, and verified no-BOM `latest.json` updater manifest.
- [x] Deployed the matching web build to `https://dislight.vercel.app`.
### Phase 67 - Synchronized soundboard stop (July 26)
- [x] Added a nonce-scoped `soundboard-stop` Realtime broadcast. Stopping a clip now stops the exact matching playback on the partner as well as locally.
- [x] Covers rapid stop-before-load and switching from one clip to another, preventing a late download from beginning after its playback was cancelled.
- [x] Verified TypeScript and production build.
### Phase 68 - Low-latency soundboard transport (July 26)
- [x] Moved in-call soundboard play/stop commands onto an ordered reliable WebRTC data channel already negotiated with the voice peer.
- [x] Kept the private Supabase broadcast path as a fallback while peer media/data is connecting or unavailable.
- [x] Retained nonce-scoped cancellation so stale/cancelled clips cannot begin after delayed fetches.
- [x] Verified TypeScript and the production build.
### Phase 69 - v0.1.21 low-latency soundboard release (July 26)
- [x] Built and signed the v0.1.21 NSIS installer.
- [x] Published GitHub release `v0.1.21` with the installer, signature, and verified no-BOM `latest.json` updater manifest.
- [x] Deployed the matching production web build to `https://dislight.vercel.app`.
### Phase 70 - Automatic default profile avatars (July 27)
- [x] Added a storage-free fallback avatar for profiles without a custom image: one uppercase first letter, bold weight, and a deterministic colored background derived from the profile identity.
- [x] The same fallback is automatically used in every existing `UserAvatar` surface, and custom avatars still take precedence.
- [x] Verified TypeScript and production build.

### Phase 71 - v0.1.22 generated-avatar release (July 27)
- [x] Built and signed the v0.1.22 NSIS installer.
- [x] Published GitHub release `v0.1.22` with the installer, signature, and verified no-BOM `latest.json` updater manifest.
- [x] Deployed the matching production web build to `https://dislight.vercel.app`.
### Phase 72 - Live soundboard listening-volume updates (July 27)
- [x] Soundboard volume now applies immediately to every active clip, including partner clips already playing on receipt.
- [x] Incoming clips continue to start using the listener's own soundboard volume, rather than the sender's device setting.
- [x] Verified TypeScript and production build.
### Phase 73 - Reliable chat-link opening (July 27)
- [x] Reworked message links to use an explicit external-browser opening path in Tauri and a direct new-tab path on web.
- [x] Preserves modifier-click behavior and shows a visible error if the browser/opener rejects a link instead of failing silently.
- [x] Verified TypeScript and production build.
### Phase 74 - Authenticated rich-link previews (July 27)
- [x] Added the authenticated link-preview Edge Function: HTML-only Open Graph extraction with URL-scheme, local/private target, redirect, response-size, and timeout safeguards.
- [x] Deployed link-preview to Supabase with JWT verification enabled and verified unauthenticated requests are rejected with HTTP 401.
- [x] Added compact, cached rich-link cards in chat with title, description, site label, and optional HTTPS thumbnail; cards use the same safe external-link action as text links.
- [x] Verified TypeScript and production build.
### Phase 75 - In-soundboard listening-volume control (July 27)
- [x] Added a compact live volume slider, muted-state icon, and percent readout directly in the shared soundboard popover.
- [x] Uses the same persistent soundboard listening preference, so it updates local and incoming active clips immediately.
- [x] Verified TypeScript and whitespace checks.
### Phase 76 - v0.1.23 rich links and soundboard QoL release (July 27)
- [x] Built and signed the v0.1.23 NSIS installer.
- [x] Published GitHub release `v0.1.23` with the installer, signature, and verified no-BOM `latest.json` updater manifest.
- [x] Deployed the matching production web build to `https://dislight.vercel.app`.

### Phase 77 - Native global voice shortcuts (July 27)
- [x] Replaced the webview-owned global shortcut listener with a Tauri-native Windows registration command and app-event bridge for mute and deafen.
- [x] Prevented focused-app double toggles by letting the browser key handler stand down while native global shortcuts are enabled.
- [x] Registration conflicts now surface as a visible error; mute/deafen only execute while a voice room is active.
- [x] Verified TypeScript, production web build, native Cargo compilation, and whitespace checks.
### Phase 78 - Isolated V2 floating-controls preview (July 27)
- [x] Added an opt-in V2 design preview while leaving V1 as the default, with a My Account setting toggle and a direct in-preview return action.
- [x] Reused the existing chat history, composer/send/media path, chat switcher, soundboard, and voice stores instead of duplicating backend or WebRTC behaviour.
- [x] Added scoped V2 controls: partner avatar switcher, soundboard, mute/deafen, V1 return action, own-avatar account menu, and floating composer.
- [x] Verified TypeScript, production web build, and whitespace checks. This is preview-only and has not been released.
### Phase 79 - Button-study UI sound system (July 27)
- [x] Copied the button-study UI sound assets into the app and replaced the old generic interface/toggle mappings with click, hover, input-focus, menu open/close, and toggle on/off cues.
- [x] Added bounded hover/focus throttling and wired the new cues to the V2 controls, chat switcher, soundboard menu, and composer focus. Voice mute/deafen use only their dedicated new toggle cue.
- [x] Verified TypeScript, production web build, and whitespace checks. This remains unreleased.
### Phase 80 - V2 avatar, split-dropdown, and call-alert correction (July 27)
- [x] Made V2 partner/self controls true avatar buttons: the button surface no longer clips or sits behind the avatar; decorations stay above it.
- [x] Added real split mic/deafen dropdowns (primary action plus chevron menu) and an icon-plus-chevron Soundboard trigger, following the button-study controls.
- [x] Replaced durable voice-start/voice-ended timeline pills with the interactive V2 AlertBar treatment.
- [x] Verified TypeScript and whitespace checks. This remains unreleased.
### Phase 81 - Exact V2 avatar, icon, and type parity (July 27)
- [x] Rebuilt V2 avatar controls as 40px avatar surfaces with the same edge treatment as the button study; decoration remains above the avatar and presence remains topmost.
- [x] Swapped V2 soundboard/deafen symbols to the button-study AudioLines and HeadphoneOff icon family.
- [x] Ported the button study's bundled wide Google Sans Flex faces and scoped them to V2 controls and text.
- [x] Verified TypeScript, production web build, and whitespace checks. This remains unreleased.
### Phase 82 - V2 timeline and opaque-control parity (July 27)
- [x] Extended the V2 design flag through chat history: message avatars now use the 40px V2 avatar treatment and reaction/edit/delete trays use matching opaque controls and menus.
- [x] Restyled V2 window controls as matching 40px control buttons.
- [x] Removed acrylic-like translucency from the V2 canvas and V2 controls, leaving the preview with an intentionally opaque #1e1e1e background.
- [x] Verified TypeScript, production web build, and whitespace checks. This remains unreleased.
### Phase 83 - Centered V2 voice header and full action controls (July 27)
- [x] Centered the V2 header and restored compact voice state/actions: Voicechat, Join, Take over, and elapsed in-voice states.
- [x] Reduced V2 chat-history avatar scale from 40px to the compact 32px treatment while retaining the V2 decoration edge.
- [x] Rebuilt V2 reaction/reply/edit/delete trays and reaction picker as full 40px V2 button controls.
- [x] Verified TypeScript, production web build, and whitespace checks. This remains unreleased.### Phase 84 - V2 control geometry and voice-action parity (July 27)
- [x] Normalized the V2 composer, own-avatar control, floating header controls, and voice action to a centered 40px control row; the timeline remains compact at 32px.
- [x] Rebuilt the in-voice action using the button-study edge, shadow, and gradient hierarchy; its active fill alone becomes green.
- [x] Restored 45px backdrop blur only to V2 controls while retaining the intentional opaque V2 canvas.
- [x] Verified TypeScript, production web build, and whitespace checks. This remains unreleased.### Phase 85 - V2 soundboard and device-menu parity (July 27)
- [x] Restyled the V2 soundboard popover, progress cards, and listening-volume surface with the matching dark glass material, 1.25px edge, and 45px backdrop blur.
- [x] Stabilized the V2 voice action with a fixed 40px height, tabular timer figures, and a bounded flexible width so seconds do not make it jump.
- [x] Added live input-device selection to the microphone dropdown and output-device selection to the deafen dropdown; both write the existing persistent audio preferences.
- [x] Verified TypeScript, production web build, and whitespace checks. This remains unreleased.### Phase 86 - V2 action-tray hierarchy correction (July 27)
- [x] Reduced V2 message action and reaction trays to a single button-study outer shell; their inner actions are now compact flat targets with hover feedback only.
- [x] Matched the button-study glyph geometry: 20px primary action icons and 14px dropdown chevrons.
- [x] Verified TypeScript, production web build, and whitespace checks. This remains unreleased.### Phase 87 - V2 reply-composer parity (July 27)
- [x] Rebuilt the V2 reply banner with the button-study 25.01px outer material, gradient edge, 45px blur, and flat inner cancel action.
- [x] Anchored the V2 account avatar to the bottom composer line rather than vertically centering it across the reply/composer stack.
- [x] Re-checked the original button-study source and applied its 40px shell, 20px glyph, 14px chevron, gradient-edge, hover-shadow, and 45px blur hierarchy to V2 controls.
- [x] Verified TypeScript, production web build, and whitespace checks. This remains unreleased.### Phase 88 - Exact V2 idle/hover material correction (July 27)
- [x] Reset V2 reply, message-action, and reaction-menu shells to the source button study's exact resting material: transparent dark fill, single 1.25px black outline, masked gradient edge, and 45px blur.
- [x] Removed accidental resting elevation; the 0 4px 10px shadow and inner light overlay now occur only on hover, exactly as in the study.
- [x] Verified TypeScript, production web build, and whitespace checks. This remains unreleased.### Phase 89 - Exact V2 mute/deafen dropdown pass (July 27)
- [x] Rebuilt V2 microphone/deafen controls from the button-study source: 62px split button, 20px primary glyph, 14px chevron, black 1.25px outline, masked gradient edge, 45px blur, and hover-only elevation.
- [x] Toggled mute/deafen now use the study’s full #ff3333 active fill and play their corresponding on/off interface sound locally.
- [x] Verified TypeScript, production web build, and whitespace checks. This remains unreleased.### Phase 90 - V2 compact-tray and voice-idle correction (July 27)
- [x] Explicitly locked the V2 message action shell to a 40px height, preventing inherited legacy layout from stretching it vertically.
- [x] Reset the V2 voice label action to the button-study's flat idle material and hover-only 0 4px 10px elevation; retained its green active state.
- [x] Verified TypeScript, production web build, and whitespace checks. This remains unreleased.### Phase 91 - Standalone V2 reply surface rebuild (July 27)
- [x] Removed legacy `surface-panel`/`app-surface` classes entirely from the V2 reply path, so they cannot override the new design.
- [x] Rebuilt the reply banner as its own button-study surface with the source geometry, gradient edge, hover overlay/elevation, glyph sizing, and 45px blur.
- [x] Verified TypeScript, production web build, and whitespace checks. This remains unreleased.### Phase 92 - Button-study interaction-sound repair (July 27)
- [x] Diagnosed the missing V2 cues: `public/sounds/ui` existed but contained no files, so every mapped audio fallback was 404ing.
- [x] Copied the button-study UI assets into the public runtime folder and ported the study's Web Audio hover, click/menu, and enable/disable toggle tone sequences into `app-sounds.ts`.
- [x] The existing interface-sounds preference and volume still gate the cues; the copied assets remain a browser/WebView fallback.
- [x] Verified TypeScript, production web build, and whitespace checks. This remains unreleased.### Phase 93 - Definitive V2 button-study source pass (July 27)
- [x] Applied the test project's exact 40px/62px shell contract, 25.01px radius, 1.25px black ring, masked gradient edge, hover-only inner overlay/elevation, 20px glyphs, 14px chevrons, and 45px blur across V2 composer, header buttons, window controls, dropdowns, and menus.
- [x] Corrected portaled menus separately: they render outside `.v2-canvas`, so they now receive the same source material through V2-specific global selectors.
- [x] Verified TypeScript, production web build, and whitespace checks. This remains unreleased.### Phase 94 - Structural V2 composer input-bar isolation (July 27)
- [x] Removed legacy `surface-panel`, `composer-surface`, and floating-shell classes from the V2 Composer render path, rather than attempting to override them.
- [x] V2 composer now uses only the source-derived input-bar material; normalized its own glyphs to 20px and retained the app's multiline behaviour.
- [x] Verified TypeScript, production web build, and whitespace checks. This remains unreleased.### Phase 95 - Complete V2 dropdown shell pass (July 27)
- [x] Extended the V2 source-derived dropdown material to the partner chat switcher and account popover, which were previously still generic app popovers.
- [x] Preserved one outer 25.01px button-study shell per dropdown and normalized inner menu items to flat hover-only targets.
- [x] Verified TypeScript, production web build, and whitespace checks. This remains unreleased.### Phase 96 - V2 composer content-scale correction (July 27)
- [x] Reduced V2 composer text to 14px and attachment/send glyphs to 16px, keeping the button-study 40px input shell unchanged.
- [x] Verified TypeScript and whitespace checks. This remains unreleased.### Phase 97 - Structural V2 popover isolation and Cloudinary review (July 27)
- [x] Added a bare shared-popover path and moved every V2 popover onto it: account, partner/chat selector, input/output menus, soundboard, and reaction picker no longer inherit legacy `surface-panel` material.
- [x] Reused the standalone source-derived V2 shell for those popovers and kept their internal menu actions flat/hover-only.
- [x] Reviewed Cloudinary Free usage: 2.24 / 25 credits used, 751,401,793 bytes stored, and 54,933,602 bytes delivered; media limits allow 10 MiB images and 100 MiB videos. A full chat-media migration is not yet performed.
- [x] Verified TypeScript, production web build, and whitespace checks. This remains unreleased.
### Phase 98 - Cloudinary chat-media delivery foundation (July 27)
- [x] Added Cloudinary-aware chat attachment delivery while retaining the existing private Supabase Storage route as a safe automatic fallback.
- [x] Cloudinary mode preserves original image/video bytes for upload, then delivers fixed optimized renditions: WebP images and 720p/30fps MP4 with H.264, AAC, 2.5 Mbps video / 128 kbps audio; this removes the browser canvas re-recording quality and audio loss path.
- [x] Applied `20260727080522_cloudinary_chat_media.sql` and `20260727081515_cloudinary_chat_media_indexes.sql`: Cloudinary attachments retain the same 3-day server expiry, 512 MiB oldest-first budget, authenticated reservations, and existing 30-day IndexedDB cache.
- [x] Deployed `purge-chat-media` with authenticated signed Cloudinary upload parameters, direct-provider verification, automatic Cloudinary deletion, and legacy cleanup compatibility.
- [x] Added Cloudinary media/upload origins to the Tauri CSP; verified TypeScript, production build, migration presence, and advisor follow-up indexes.
- [x] Cloudinary Edge Function secrets are present. They remain server-only and must never be placed in `.env`, the frontend, Git, or this handoff.

### Phase 99 - Steady V2 dropdown material (July 27)
- [x] Changed V2 dropdown shells to a darker 70% resting material and removed hover-driven background recoloring. Hover can retain elevation but no longer flashes a different fill.
### Phase 100 - V2 default and media drop intake (July 27)
- [x] Made the V2 floating-controls conversation view the sole active chat shell; V1 source remains retained for compatibility/reference but is no longer selectable in the live UI.
- [x] Removed the V2 return-to-V1 button and the obsolete settings toggle.
- [x] Added app-wide drag/drop intake for supported image/video files: dropping anywhere over an active chat forwards the file into its composer, alongside existing paste and composer-drop flows.
- [x] Reduced the top/bottom message viewport mask depth so normal history stays visible closer to the floating controls.
- [x] Voice action behavior is now exact: idle uses the normal study material, open/joinable voice is `#00b339`, and the local in-call leave action transitions to `#ff3333` on hover.
- [x] Verified the `CLOUDINARY_API_KEY` and `CLOUDINARY_API_SECRET` secret names are present in Supabase. The deployed function reads secrets at invocation time, so a refresh activates Cloudinary without redeployment.
- [x] Verified TypeScript, production build, JSON configuration, and whitespace checks.
### Phase 101 - WebView2 V2 glass compositor hotfix (July 27)
- [x] Removed the V2 floating control isolation boundaries that caused packaged WebView2 to treat each control as its own backdrop root, leaving `backdrop-filter` with no chat content to blur.
- [x] Preserved the original button-study gradient edges, clipping, and pseudo-element layering while returning the header, composer, menus, and action trays to the shared chat backdrop tree.
- [x] Verified TypeScript, production web build, and whitespace checks for v0.1.25.
### Phase 102 - Cloud media reservation and synchronized soundboard repair (July 28)
- [x] Diagnosed media-upload `Edge Function returned a non-2xx status code`: the live reservation RPC had a PostgreSQL variable/column ambiguity on `reserved_bytes`, causing the `purge-chat-media` Edge Function to return HTTP 500 before Cloudinary upload.
- [x] Applied `20260728051932_fix_cloud_media_reservation_shadowing.sql` to production and updated the source migration. A live reservation-and-cleanup smoke test now returns `accepted: true`.
- [x] Soundboard play/stop remains volume-free across the wire; each listener applies only their own persistent local listening level. Added a WebRTC data-channel prepare/ready cache handshake and scheduled common playback timestamp to improve two-sided sync.
- [x] Restored the V2 header screen-share button with native V2 material and green active/stop state.
- [x] Verified TypeScript, production build, whitespace checks, and Supabase security/performance advisors. The advisor warnings are pre-existing private-table/RPC/auth configuration notices, not caused by this reservation correction.
### Phase 103 - Media action-overlay and Cloudinary audio verification (July 28)
- [x] Made the V2 message action tray strictly `position: absolute` with a dedicated overlay layer, so hover actions cannot participate in message-row flex sizing or shift media/text.
- [x] Verified the live Cloudinary test rendition with `ffprobe`: 1280x534 H.264 at ~2.46 Mbps, 30 fps, plus a 48 kHz stereo AAC stream at ~128 kbps; the delivered 9,804,710-byte file does contain audio. Any silence is therefore local HTML media/system output or stale-cache playback rather than storage/transcoding loss.
- [x] Routed native chat-video playback to the persistent selected output device and output volume, explicitly clearing media mute state.
- [x] Verified TypeScript, production build, and whitespace checks.
### Phase 104 - V2 material normalization and call-only controls (July 28)
- [x] Normalized the avatar/header/composer/window/voice controls to one `rgb(12 12 12 / .30)` V2 material with the shared 45px backdrop blur; only portaled dropdown menus remain intentionally darker at 70%.
- [x] Hid Soundboard, screen share, mute, and deafen until the local user has joined voice; the Voicechat/Join action remains available as the entry point.
- [x] Removed the V2 voice action's artificial 184px minimum; idle Voicechat is compact and centered, expanding only for live join/call labels and elapsed time.
- [x] Verified TypeScript, production build, and whitespace checks.
### Phase 105 - Media-forward rich link embeds (July 28)
- [x] Rebuilt rich link previews as full-width message embeds, aligned with the sender side.
- [x] Promoted link media to a large responsive 16:9 lead image with metadata below, rather than a cramped side thumbnail.
- [x] Applied the shared V2 30% glass surface and verified TypeScript, production build, and whitespace checks.

### Phase 106 - Rich-link opening and self-healing voice recovery (July 28)
- [x] Rich-link previews now replace the duplicate raw URL once their preview resolves; the full embed remains the accessible, native-openable link. Plain links stay visible/clickable when a preview cannot be fetched.
- [x] Fixed native external-link authorization by granting Tauri opener `allow-open-url` in addition to the existing HTTP(S) URL scope. Web/PWA links retain a direct-anchor fallback for restrictive popup policies.
- [x] Restored the V2 message action tray to the shared 30%/45px glass material while preserving its absolute overlay geometry, so hover actions do not shift message layout.
- [x] Added call self-healing: signaling channel retry with bounded backoff, refreshed ICE/TURN restart, two clean peer reconstructions before surfacing a manual retry, and a network-online recovery hook for VPN/network transitions. Existing room/session state is preserved throughout recovery.
- [x] Verified TypeScript, production web build, native Cargo compilation, and whitespace checks.


### Phase 107 - Rich embed edge and Windows control refinement (July 28)
- [x] Moved rich-link embeds onto the V2 button-study material with an in-layout 1.25px edge and masked highlight, avoiding the clipped outer border caused by an overflow-hidden media shell.
- [x] Replaced the circular V2 window controls with always-visible, floating Windows-style rectangular minimize/maximize/close targets. They retain the shared dark glass shell and Windows-red close hover.
- [x] Verified TypeScript, production web build, and whitespace checks.


### Phase 108 - Full rich-media material, app-edge controls, and remembered video volume (July 28)
- [x] Applied a light full-card material wash to rich-link media while retaining the full-resolution media image and the V2 edge treatment.
- [x] Moved desktop window buttons flush to the app’s top-right edge and reduced them to plain Windows-style titlebar targets with no glass shell.
- [x] Added a persistent, media-only playback-volume preference. Changing an HTML video’s native volume control now becomes the default for future chat videos without changing call output volume.
- [x] Verified TypeScript, production web build, and whitespace checks.

### Phase 109 - Fluent app-edge controls and WebView2 media-tray correction (July 28)
- [x] Added `@fluentui/react-icons` and replaced desktop titlebar glyphs with Fluent Subtract, Square/Copy, and Dismiss icons.
- [x] Kept titlebar buttons completely transparent and square at the app edge, with hover-only Windows treatment and no independent shell/radius.
- [x] Reworked the V2 action tray to use a dedicated dark translucent pseudo-pane with its own 45px backdrop filter, which reliably obscures/softens video behind it in WebView2.
- [x] Toned rich-link media with a full-card glass/media wash while retaining large responsive artwork and the button-study edge.
- [x] Verified TypeScript, production web build, and whitespace checks.

### Phase 110 - Neutral native controls (July 28)
- [x] Removed all titlebar button fill, radius, and hover color treatment; the Fluent glyphs are the only visible controls at the natural app edge.
- [x] Rebuilt and relaunched the separate Tauri dev app without touching the installed/live app.

### Phase 111 - Plain titlebar controls (July 28)
- [x] Removed the final titlebar backdrop-filter and shell; controls are transparent until hover, then brighten normally or turn Windows-red for Close.
- [x] Verified TypeScript and whitespace checks.


### Phase 112 - Single-layer V2 action tray (July 28)
- [x] Removed stacked tray pseudo-surfaces that caused media colour bleed and replaced them with a single neutral dark 45px glass shell, compact icon targets, and stable overlay sizing.
- [x] Verified TypeScript and whitespace checks.

### Phase 113 - Packaged V2 parity and device-menu hotfix (July 28)
- [x] Allowed HTTPS Open Graph artwork in the Tauri image CSP, fixing rich-preview images that worked in dev but were blocked by the packaged app.
- [x] Restored V2 message action trays to hover-only visibility; a final `display:flex` styling rule had unintentionally exposed every tray.
- [x] Replaced the imperceptible glass treatment on V2's intentionally opaque canvas with a consistent solid `#1e1e1e` resting surface across floating controls and portaled menus.
- [x] Refined the shared soundboard into a compact V2 library with clear sections, steady progress fill, modernized cards, and matching volume control.
- [x] Made microphone/output dropdowns refresh actual devices on open and hardware changes, with a permission-aware label refresh and clear loading/empty states.
- [x] Added the four valid wide Google Sans Flex TTF assets to `public/fonts`; prior requests returned the HTML fallback and triggered a 6OTS font parser warning.
- [x] Restored rich-link artwork to full brightness by removing legacy opacity/filter/dark overlay styling.
- [x] Verified TypeScript, production web build, and whitespace checks before v0.1.26 packaging.
### Phase 114 - Distinct voice lifecycle interface sounds (July 29)
- [x] Reviewed the existing Web Audio UI cue system: hover/click/menu/focus remain deliberately quiet; mute/deafen retain their dedicated up/down toggle sequences.
- [x] Replaced the generic voice start/end pop cues with dedicated `voice_join` and `voice_leave` sequences. Joining is a brighter three-note rise; leaving is a clearer descending three-note cue, both still governed by the interface-sound preference and selected output device.
- [x] Applied these lifecycle cues to local join/leave and partner presence join/leave events. Verified TypeScript and whitespace checks.
### Phase 115 - Voice-cue runtime hotfix (July 29)
- [x] Corrected the `waveform` destructuring omission in the shared tone player. This restores hover/click/menu/toggle cues and enables the new join/leave cue waveforms without runtime errors.
- [x] Verified TypeScript and whitespace checks before v0.1.27 packaging.
### Phase 116 - Backend reliability and security hardening (July 29)
- [x] Corrected the Cloudflare screen-share lifecycle: the client now waits for WebRTC connectivity only after the display track is published/subscribed, matching Cloudflare's session ? track ? media lifecycle.
- [x] Added `private.cloudflare_screen_sessions` and bound Cloudflare session/track operations to the active conversation and owning participant; the Edge Function now logs concise upstream failure codes rather than hiding every cause behind a generic 502.
- [x] Moved `rename_soundboard_sound` to SECURITY INVOKER with an owner-only update policy and explicitly removed anonymous execution. Verified the function is no longer SECURITY DEFINER and anon cannot call it.
- [x] Prioritized pinned soundboard clips during the existing background cache warm-up without altering the soundboard UI.
- [x] Hardened link-preview host rejection for common private/internal names and return a clean null preview for ordinary upstream refusals.
- [x] Applied both migrations and deployed `cloudflare-realtime` v6 plus `link-preview` v5. Verified TypeScript and production web build.
- [ ] Supabase Dashboard: enable Auth ? Password Security ? leaked password protection (management setting not exposed through the available project API).

### Phase 117 - v0.1.28 backend reliability release (July 29)
- [x] Packaged the Cloudflare session hardening, link-preview safety, pinned sound cache prioritization, and 720p/30fps stream stability updates as desktop v0.1.28.
- [x] Built the NSIS installer, signed it with the existing gitignored updater identity, and generated a UTF-8 no-BOM updater manifest.
- [x] Published GitHub release `v0.1.28` with the signed installer, `.sig`, and verified no-BOM `latest.json`; the configured `/releases/latest/download/latest.json` endpoint resolves to v0.1.28.

### Phase 118 - Nitro rebrand and PWA icon refresh (July 29)
- [x] Rebranded the visible desktop, web, PWA, auth, settings, and package identity from Dislight to Nitro while retaining the existing Tauri identifier and internal data keys for update/data compatibility.
- [x] Generated the complete Tauri app icon family from `Turbo.png` and used `Turbo_Mobile.png` as the transparent PWA/browser/Apple-touch icon.
- [x] Updated the service-worker cache namespace and shell assets; production web build passed before deployment.

### Phase 119 - Nitro mobile shell polish (July 29)
- [x] Switched the PWA/browser/Apple-touch icon to the dedicated `Turbo_Mobile_BlackBG.png` mark; transparent app icons do not receive a black plate automatically.
- [x] Added a mobile-only #0D0D0D canvas treatment and centered compact V2 header rail, including safe-area spacing and horizontal overflow protection for in-call controls.
- [x] Verified the production web build and whitespace checks before deployment.
