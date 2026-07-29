-- Keep the public sound rename RPC subject to normal ownership RLS instead of
-- bypassing it with SECURITY DEFINER.
drop policy if exists "soundboard_owner_update" on public.soundboard_sounds;
create policy "soundboard_owner_update"
on public.soundboard_sounds for update to authenticated
using (owner_id = (select auth.uid()))
with check (owner_id = (select auth.uid()));

alter function public.rename_soundboard_sound(uuid, text) security invoker;
revoke all on function public.rename_soundboard_sound(uuid, text) from public;
grant execute on function public.rename_soundboard_sound(uuid, text) to authenticated;

-- Cloudflare session and track identifiers are not secrets, but binding them to
-- their owner/conversation prevents one active caller from operating another
-- conversation's SFU session through the Edge Function.
create table if not exists private.cloudflare_screen_sessions (
  session_id text primary key check (char_length(session_id) between 8 and 160),
  conversation_id uuid not null references public.conversations(id) on delete cascade,
  owner_id uuid not null references public.profiles(id) on delete cascade,
  track_name text check (track_name is null or char_length(track_name) between 1 and 160),
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  expires_at timestamptz not null default now() + interval '2 hours'
);

create index if not exists cloudflare_screen_sessions_lookup_idx
  on private.cloudflare_screen_sessions (conversation_id, owner_id, expires_at desc);

alter table private.cloudflare_screen_sessions enable row level security;
revoke all on private.cloudflare_screen_sessions from public, anon, authenticated;
grant all on private.cloudflare_screen_sessions to service_role;
