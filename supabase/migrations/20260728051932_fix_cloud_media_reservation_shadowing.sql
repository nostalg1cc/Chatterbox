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