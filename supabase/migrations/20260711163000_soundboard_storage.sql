create table public.soundboard_sounds (
  id uuid primary key,
  owner_id uuid not null references public.profiles(id) on delete cascade,
  name text not null check (char_length(name) between 1 and 32),
  storage_path text not null unique,
  size_bytes integer not null check (size_bytes between 1 and 524288),
  duration_ms integer not null check (duration_ms between 100 and 15000),
  created_at timestamptz not null default now()
);

create index soundboard_sounds_owner_created_idx
  on public.soundboard_sounds (owner_id, created_at desc);

alter table public.soundboard_sounds enable row level security;

create policy "soundboard_owner_read"
on public.soundboard_sounds for select to authenticated
using (owner_id = (select auth.uid()));

create schema if not exists private;
revoke all on schema private from public;
grant usage on schema private to authenticated, service_role;

create table private.soundboard_reservations (
  sound_id uuid primary key,
  owner_id uuid not null,
  storage_path text not null unique,
  reserved_bytes integer not null check (reserved_bytes between 1 and 524288),
  expires_at timestamptz not null default now() + interval '15 minutes'
);

revoke all on private.soundboard_reservations from public, anon, authenticated;
grant all on private.soundboard_reservations to service_role;

create or replace function public.reserve_soundboard_upload(
  p_sound_id uuid,
  p_owner_id uuid,
  p_path text,
  p_size_bytes integer
)
returns boolean
language plpgsql
security definer
set search_path = ''
as $$
declare
  global_stored bigint;
  user_stored bigint;
  global_reserved bigint;
  user_reserved bigint;
  sound_count integer;
begin
  if p_size_bytes < 1 or p_size_bytes > 524288
     or p_path <> p_owner_id::text || '/' || p_sound_id::text || '.webm' then
    return false;
  end if;

  perform pg_advisory_xact_lock(413092, 1);
  delete from private.soundboard_reservations where expires_at <= now();

  select coalesce(sum(size_bytes), 0), count(*)
  into user_stored, sound_count
  from public.soundboard_sounds
  where owner_id = p_owner_id and id <> p_sound_id;

  select coalesce(sum(size_bytes), 0)
  into global_stored
  from public.soundboard_sounds
  where id <> p_sound_id;

  select coalesce(sum(reserved_bytes), 0)
  into global_reserved
  from private.soundboard_reservations
  where expires_at > now() and sound_id <> p_sound_id;

  select coalesce(sum(reserved_bytes), 0)
  into user_reserved
  from private.soundboard_reservations
  where owner_id = p_owner_id and expires_at > now() and sound_id <> p_sound_id;

  if sound_count >= 24
     or user_stored + user_reserved + p_size_bytes > 16777216
     or global_stored + global_reserved + p_size_bytes > 100663296 then
    return false;
  end if;

  insert into private.soundboard_reservations (
    sound_id, owner_id, storage_path, reserved_bytes
  ) values (p_sound_id, p_owner_id, p_path, p_size_bytes)
  on conflict (sound_id) do update set
    storage_path = excluded.storage_path,
    reserved_bytes = excluded.reserved_bytes,
    expires_at = now() + interval '15 minutes';

  return true;
end;
$$;

revoke all on function public.reserve_soundboard_upload(uuid, uuid, text, integer)
  from public, anon, authenticated;
grant execute on function public.reserve_soundboard_upload(uuid, uuid, text, integer)
  to service_role;

create or replace function public.release_soundboard_reservation(p_sound_id uuid)
returns void
language sql
security definer
set search_path = ''
as $$
  delete from private.soundboard_reservations where sound_id = p_sound_id;
$$;

revoke all on function public.release_soundboard_reservation(uuid)
  from public, anon, authenticated;
grant execute on function public.release_soundboard_reservation(uuid)
  to service_role;

create or replace function public.soundboard_reservation_bytes(
  p_sound_id uuid,
  p_owner_id uuid
)
returns integer
language sql
security definer
set search_path = ''
as $$
  select reserved_bytes
  from private.soundboard_reservations
  where sound_id = p_sound_id
    and owner_id = p_owner_id
    and expires_at > now();
$$;

revoke all on function public.soundboard_reservation_bytes(uuid, uuid)
  from public, anon, authenticated;
grant execute on function public.soundboard_reservation_bytes(uuid, uuid)
  to service_role;
insert into storage.buckets (id, name, public, file_size_limit, allowed_mime_types)
values ('soundboard', 'soundboard', false, 524288, array['audio/webm'])
on conflict (id) do update set
  public = excluded.public,
  file_size_limit = excluded.file_size_limit,
  allowed_mime_types = excluded.allowed_mime_types;

-- Object operations are only performed by the soundboard-storage Edge Function.
-- The catalog is owner-readable; playback uses a short-lived signed URL after
-- the function verifies that the caller is currently in the target voice room.