-- Cloudinary-backed chat-media delivery. The public client receives only a
-- short-lived signed upload request; Cloudinary credentials remain in Edge
-- Function secrets. Existing Supabase Storage attachments remain supported.

alter table public.messages
  drop constraint if exists messages_media_metadata_valid,
  drop constraint if exists messages_media_path_owned;

alter table public.messages
  add constraint messages_media_metadata_valid
  check (
    (
      media_kind is null
      and media_path is null
      and media_mime_type is null
      and media_size_bytes is null
      and media_width is null
      and media_height is null
      and media_duration_seconds is null
      and media_expires_at is null
      and media_deleted_at is null
    )
    or
    (
      media_kind is not null
      and media_mime_type = case media_kind
        when 'image' then 'image/webp'
        when 'video' then case when media_mime_type in ('video/webm', 'video/mp4') then media_mime_type end
      end
      and media_size_bytes between 1 and 104857600
      and media_width between 1 and 1920
      and media_height between 1 and 1920
      and (
        (media_kind = 'image' and media_duration_seconds is null)
        or
        (
          media_kind = 'video'
          and media_duration_seconds > 0
          and media_duration_seconds <= 120
          and greatest(media_width, media_height) <= 1280
          and least(media_width, media_height) <= 720
        )
      )
      and media_expires_at is not null
      and (
        (media_path is not null and media_deleted_at is null)
        or (media_path is null and media_deleted_at is not null)
      )
    )
  ),
  add constraint messages_media_path_owned
  check (
    media_path is null
    or media_path = conversation_id::text || '_' || id::text ||
      case media_kind when 'image' then '.webp' when 'video' then '.webm' end
    or media_path = 'cloudinary:' || media_kind || ':' || conversation_id::text || '_' || id::text
  );

create table if not exists private.cloud_chat_media_reservations (
  path text primary key,
  conversation_id uuid not null references public.conversations(id) on delete cascade,
  message_id uuid not null,
  user_id uuid not null references public.profiles(id) on delete cascade,
  reserved_bytes bigint not null check (reserved_bytes between 1 and 104857600),
  created_at timestamptz not null default now(),
  expires_at timestamptz not null default now() + interval '15 minutes'
);

alter table private.cloud_chat_media_reservations enable row level security;
revoke all on table private.cloud_chat_media_reservations from public, anon, authenticated;
create index if not exists cloud_chat_media_reservations_expiry_idx
  on private.cloud_chat_media_reservations (expires_at);

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
  delete from private.cloud_chat_media_reservations r where expires_at <= now();

  select coalesce(sum(m.media_size_bytes), 0)
    into stored_bytes
    from public.messages m
   where m.media_path like 'cloudinary:%'
     and m.media_deleted_at is null;

  select coalesce(sum(r.reserved_bytes), 0)
    into pending_reservation_bytes
    from private.cloud_chat_media_reservations r
   where r.expires_at > now();

  if stored_bytes + pending_reservation_bytes + p_bytes > 536870912 then
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

create or replace function public.release_cloud_chat_media_reservations(p_paths text[])
returns void
language sql
security definer
set search_path = ''
as $$
  delete from private.cloud_chat_media_reservations r where path = any(p_paths);
$$;

revoke all on function public.release_cloud_chat_media_reservations(text[]) from public, anon, authenticated;
grant execute on function public.release_cloud_chat_media_reservations(text[]) to service_role;