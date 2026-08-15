-- Cloudinary chat media was inheriting the same 3-day media_expires_at and
-- 512MiB budget that only make sense for the tiny legacy Supabase Storage
-- bucket. That defeated the point of moving chat media to Cloudinary: it
-- still vanished remotely on the same schedule, so once the 30-day local
-- IndexedDB cache (src/lib/media-cache.ts) had never been populated (or had
-- evicted the entry), the media was gone for good. Cloudinary-hosted media
-- now gets an effectively permanent expiry, governed only by its own much
-- larger storage budget (raised in lockstep in the purge-chat-media Edge
-- Function) as a safety net against runaway free-tier usage.
--
-- media_expires_at may still never be shortened or changed on legacy-storage
-- rows, but a Cloudinary row's expiry may be extended (never reduced) - this
-- is what lets the one-time backfill below bring pre-fix Cloudinary rows up
-- to the new no-expiry default without disabling the trigger outright. New
-- Cloudinary inserts already get the 100-year expiry directly, so this adds
-- no real capability beyond what insert-time already grants.
create or replace function private.enforce_message_media_retention()
returns trigger
language plpgsql
set search_path = ''
as $$
begin
  if tg_op = 'INSERT' then
    if new.media_kind is not null then
      new.media_expires_at := new.created_at + case
        when new.media_path like 'cloudinary:%' then interval '100 years'
        else interval '3 days'
      end;
    end if;
    return new;
  end if;

  if new.media_kind is distinct from old.media_kind
    or new.media_mime_type is distinct from old.media_mime_type
    or new.media_size_bytes is distinct from old.media_size_bytes
    or new.media_width is distinct from old.media_width
    or new.media_height is distinct from old.media_height
    or new.media_duration_seconds is distinct from old.media_duration_seconds
  then
    raise exception 'message media metadata is immutable';
  end if;

  if new.media_expires_at is distinct from old.media_expires_at
    and not (
      old.media_path like 'cloudinary:%'
      and new.media_path is not distinct from old.media_path
      and new.media_expires_at > old.media_expires_at
    )
  then
    raise exception 'message media metadata is immutable';
  end if;

  if new.media_path is distinct from old.media_path
    or new.media_deleted_at is distinct from old.media_deleted_at
  then
    if not (
      old.media_path is not null
      and old.media_deleted_at is null
      and new.media_path is null
      and new.media_deleted_at is not null
    ) then
      raise exception 'invalid message media deletion transition';
    end if;
  end if;

  return new;
end;
$$;

-- Bring pre-fix Cloudinary attachments up to the new no-expiry default so
-- they don't get swept by the next hourly purge run.
update public.messages
set media_expires_at = created_at + interval '100 years'
where media_path like 'cloudinary:%'
  and media_deleted_at is null
  and media_expires_at < now() + interval '90 years';

create or replace function public.reserve_cloud_chat_media_upload(
  p_path text,
  p_conversation uuid,
  p_message uuid,
  p_user uuid,
  p_bytes bigint
)
returns boolean
language plpgsql
security definer
set search_path = ''
as $$
declare
  stored_bytes bigint;
  pending_reservation_bytes bigint;
begin
  if p_path <> 'cloudinary:image:' || p_conversation::text || '_' || p_message::text
     and p_path <> 'cloudinary:video:' || p_conversation::text || '_' || p_message::text
     or p_bytes < 1 or p_bytes > 104857600
  then
    return false;
  end if;

  if not exists (
    select 1 from public.conversations c
    where c.id = p_conversation and p_user in (c.user1_id, c.user2_id)
  ) then
    return false;
  end if;

  perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtext('dislight-cloud-chat-media-quota'));
  delete from private.cloud_chat_media_reservations where expires_at <= now();

  select coalesce(sum(m.media_size_bytes), 0)
    into stored_bytes
    from public.messages m
   where m.media_path like 'cloudinary:%'
     and m.media_deleted_at is null;

  select coalesce(sum(r.reserved_bytes), 0)
    into pending_reservation_bytes
    from private.cloud_chat_media_reservations r
   where r.expires_at > now();

  -- 4GiB, up from the legacy bucket's 512MiB - Cloudinary's free-tier
  -- storage headroom is currently ~24 of 25 credits unused account-wide,
  -- so this is just a runaway-cost guard, not a meaningful constraint.
  if stored_bytes + pending_reservation_bytes + p_bytes > 4294967296 then
    return false;
  end if;

  insert into private.cloud_chat_media_reservations (
    path, conversation_id, message_id, user_id, reserved_bytes
  ) values (
    p_path, p_conversation, p_message, p_user, p_bytes
  )
  on conflict (path) do update set
    reserved_bytes = excluded.reserved_bytes,
    expires_at = now() + interval '15 minutes';

  return true;
end;
$$;

revoke all on function public.reserve_cloud_chat_media_upload(text, uuid, uuid, uuid, bigint)
  from public, anon, authenticated;
grant execute on function public.reserve_cloud_chat_media_upload(text, uuid, uuid, uuid, bigint)
  to service_role;
