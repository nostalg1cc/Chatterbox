# Dislight — Agent Handoff Document

> **Purpose:** This file is the single source of truth for cross-agent handoff (Claude Code ↔ Codex).
> Whoever works on this project: **update the checklist below as you complete work**, and add notes
> in "Decisions & gotchas" when you make a non-obvious choice. Keep it current after every phase.

## What is this project?

**Dislight** — a Windows desktop 1-on-1 chat app (Discord-inspired, but no servers/guilds; strictly DM-focused).
Sleek, minimal, **dark-only** UI. No gradients, no glassmorphism. Windows 11 **Mica** window material.

## Stack

- **Shell:** Tauri 2.x — `decorations: false`, `transparent: true`, custom titlebar, Mica via `window-vibrancy` crate (fallback: solid bg on Win10 via `.no-mica` class on `<html>`, driven by `is_mica_supported` Rust command)
- **Frontend:** React 19 + TypeScript + Vite, Tailwind CSS v4, shadcn/ui only (zinc base, dark-only — `<html class="dark">`, no theme toggle)
- **State:** Zustand stores (`src/stores/`) + Supabase Realtime subscriptions pushing into stores. No react-query.
- **Backend:** Supabase (auth + Postgres/RLS + realtime). No custom server. Rust surface is minimal (window effects only).
- **Package manager:** npm (pnpm not installed on this machine)
- **Routing:** none — top-level state machine in `App.tsx`: `booting → auth → main`

## Supabase

- **Org:** "Nate's projects" (`vercel_icfg_oxyNEBfLgvm7JSw54EpiKrX9`)
- **Project:** `dislight` — ref `lapjrxdgcbdseskmyfru`, region `eu-north-1`, free tier ($0/month)
- **DO NOT TOUCH** the other project `supabase-red-village` (`wqxtjrkvdhitcduztcju`)
- Client env vars in `.env` (gitignored): `VITE_SUPABASE_URL`, `VITE_SUPABASE_ANON_KEY` (see `.env.example`)
- Migrations are applied via Supabase MCP `apply_migration` AND mirrored in `supabase/migrations/*.sql` (repo copy is the reference; keep both in sync)
- **Email confirmation:** new projects default to ON. App handles both states. User can disable in Dashboard → Authentication → Sign In / Up → Email → "Confirm email" for instant signup in dev.

### Schema (all tables RLS-enabled, in `public`)

| Table | Purpose | Key points |
|---|---|---|
| `profiles` | user profile | PK = auth.users id; `username` citext unique (3–20, `^[a-z0-9_]+$`); created by `handle_new_user()` trigger on auth.users |
| `friendships` | requests + friends | `requester_id`, `addressee_id`, `status` enum pending/accepted/blocked; unique on ordered pair |
| `conversations` | 1:1 threads | `user1_id < user2_id` ordered pair, unique; auto-created by trigger when friendship → accepted; `last_message_at` bumped by message trigger |
| `messages` | chat messages | soft delete (`deleted_at`), `edited_at`; content 1–4000 chars |
| `reactions` | emoji reactions | unique (message_id, user_id, emoji); emoji ≤ 8 chars |
| `conversation_reads` | unread tracking | PK (conversation_id, user_id), `last_read_at` |

- Helper: `public.is_participant(conv_id uuid)` — SECURITY DEFINER, used by policies to avoid recursion
- Realtime: `supabase_realtime` publication includes `messages`, `reactions`, `friendships`, `conversations`
- Presence/typing: Supabase Realtime channels (presence channel `online`, broadcast `typing:{conversation_id}`), no tables

## Layout / design language

- Grid: `[titlebar 32px] / [sidebar 280px | chat pane]`
- Mica shows through **titlebar + sidebar** (transparent layers); **chat pane is solid** `bg-background` (this is native Win11 style à la File Explorer — NOT glassmorphism; do not add blur/gradients)
- Near-black zinc tokens, hairline borders, system font stack (Segoe UI Variable), lucide-react icons
- Destructive red is the only accent color

## Checklist (keep updated!)

### Phase 0 — Handoff
- [x] AGENTS-HANDOFF.md + AGENTS.md created

### Phase 1 — Supabase backend
- [x] Project created (`lapjrxdgcbdseskmyfru`)
- [x] Migration: schema (tables, enum, triggers, indexes) — `initial_schema`
- [x] Migration: RLS policies + helpers — `rls_policies`
- [x] Migration: realtime publication + replica identity full on reactions/friendships — `realtime_publication`
- [x] Migration: helpers moved to `private` schema, trigger fn EXECUTE revoked — `lock_down_function_exposure`
- [x] Advisors: security = 0 findings; performance = only "unused index" INFO (expected, fresh DB)
- [x] `.env` + `.env.example` written (URL + `sb_publishable_...` key in `VITE_SUPABASE_ANON_KEY`)
- [x] Migrations mirrored to `supabase/migrations/`

### Phase 2 — Scaffold
- [x] Vite + React + TS scaffold (hand-written, in repo root; create-vite refused non-empty dir)
- [x] Tailwind v4 + shadcn init (radix base, **nova preset**, Geist font bundled via @fontsource) + 19 components in `src/components/ui/`
- [x] Tauri init: undecorated/transparent/shadow/dark window, strict prod CSP (devCsp null for HMR), window-control capabilities
- [x] Rust: window-vibrancy `apply_mica` in setup + `is_mica_supported` command (cargo check passes)
- [x] git init + .gitignore + initial commit

### Phase 3 — Window shell
- [ ] `components/titlebar.tsx` (drag region, min/max/close, hides outside Tauri)
- [ ] App layout grid, no window scrollbars

### Phase 4 — Auth + boot
- [ ] `lib/supabase.ts` client
- [ ] Boot screen + session restore + onAuthStateChange routing
- [ ] Login form (email+password)
- [ ] Signup form (username availability check, display name, verify-email state + resend)
- [ ] `stores/auth.ts`

### Phase 5 — Main app
- [ ] `stores/friends.ts`, `stores/chat.ts`, `stores/presence.ts`
- [ ] Sidebar: conversation list (unread badge, online dot, last message, context menu), user footer
- [ ] Add friend dialog (by username, distinct error toasts)
- [ ] Friends view (tabs: All / Pending; accept/decline/cancel; realtime + toast on incoming)
- [ ] Chat view: header (status + typing), message list (grouping, day separators, pagination, autoscroll)
- [ ] Message item: hover actions, edit, soft delete, reactions (quick-8 popover, toggle chips)
- [ ] Composer (Enter send / Shift+Enter newline, optimistic send)
- [ ] Realtime wiring: messages, reactions, friendships, conversations, presence, typing broadcast
- [ ] Unread tracking (conversation_reads upsert on view/focus)
- [ ] Settings dialog (edit display name, logout)

### Phase 6 — Polish + verify
- [ ] Empty states, skeletons, error toasts
- [ ] `npx tsc --noEmit` + `npm run build` + `cargo check` clean
- [ ] E2E: two accounts (app + browser on Vite URL), friend request → accept → chat → edit/delete → reactions → typing → unread
- [ ] RLS negative test (third account can't read others' messages)
- [ ] Final advisors pass

## Decisions & gotchas

- **RLS helpers live in `private` schema** (`private.is_participant(conv_id)`, `private.message_conversation(msg_id)`) — NOT `public` — so PostgREST doesn't expose them as RPC. `authenticated` has EXECUTE + USAGE on the schema (policies evaluate as the querying role).
- **Username is plain `text`**, not citext — check constraint forces `^[a-z0-9_]{3,20}$`; client must lowercase before insert/lookup.
- **No client INSERT policy on `conversations`** — rows are only created by the `handle_friendship_accepted` trigger (SECURITY DEFINER bypasses RLS).
- **Decline/cancel/unfriend = DELETE on friendships** (no 'declined' status). Accept = addressee updates status pending→accepted.
- **`reactions` + `friendships` have `replica identity full`** so realtime DELETE events pass RLS authorization.
- Supabase key used is the modern `sb_publishable_...` key (works with supabase-js v2), stored under the `VITE_SUPABASE_ANON_KEY` env name.
- **RPCs for the client:** `public.username_available(text)` (anon-callable SECURITY DEFINER — intentional, signup-time check; the security advisor will WARN about it, that's accepted) and `public.conversation_overview()` (SECURITY INVOKER, sidebar previews + unread counts in one call).
- **Known realtime caveat:** postgres_changes DELETE events are not RLS-filtered by Supabase; with replica identity full the old row (uuids + emoji only, never message content) is visible to any authenticated subscriber of that table. Accepted for reactions/friendships.
- **Unfriending keeps the conversation + history** (Discord-like). Messages can still be sent after unfriend (participants keep insert rights).
- shadcn CLI is now v4-era: `init -b radix -p nova`; no `--base-color` flag anymore.
- `sonner.tsx` was edited to hardcode `theme="dark"` (next-themes removed).

## How to run

```
npm install
npm run tauri dev     # desktop app
npm run dev           # frontend only, for testing a 2nd account in a browser
npx tsc --noEmit      # typecheck
```
