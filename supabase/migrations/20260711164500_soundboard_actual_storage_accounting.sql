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

  select count(*) into sound_count
  from public.soundboard_sounds
  where owner_id = p_owner_id and id <> p_sound_id;

  select coalesce(sum(
    case when (metadata->>'size') ~ '^[0-9]+$'
      then (metadata->>'size')::bigint else 524288 end
  ), 0)
  into global_stored
  from storage.objects
  where bucket_id = 'soundboard' and name <> p_path;

  select coalesce(sum(
    case when (metadata->>'size') ~ '^[0-9]+$'
      then (metadata->>'size')::bigint else 524288 end
  ), 0)
  into user_stored
  from storage.objects
  where bucket_id = 'soundboard'
    and name like p_owner_id::text || '/%'
    and name <> p_path;

  select coalesce(sum(reserved_bytes), 0)
  into global_reserved
  from private.soundboard_reservations r
  where expires_at > now() and sound_id <> p_sound_id
    and not exists (
      select 1 from storage.objects o
      where o.bucket_id = 'soundboard' and o.name = r.storage_path
    );

  select coalesce(sum(reserved_bytes), 0)
  into user_reserved
  from private.soundboard_reservations r
  where owner_id = p_owner_id and expires_at > now() and sound_id <> p_sound_id
    and not exists (
      select 1 from storage.objects o
      where o.bucket_id = 'soundboard' and o.name = r.storage_path
    );

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