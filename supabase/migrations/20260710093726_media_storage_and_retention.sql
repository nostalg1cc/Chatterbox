-- Chat media and avatars with strict storage limits and retention.

alter table public.profiles
  add column avatar_path text,
  add column avatar_updated_at timestamptz;

alter table public.profiles
  add constraint profiles_avatar_path_owned
  check (avatar_path is null or avatar_path = id::text || '/avatar.webp');

alter table public.messages
  add column media_kind text,
  add column media_path text,
  add column media_mime_type text,
  add column media_size_bytes bigint,
  add column media_width integer,
  add column media_height integer,
  add column media_duration_seconds numeric(10, 3),
  add column media_expires_at timestamptz,
  add column media_deleted_at timestamptz;

alter table public.messages drop constraint content_length;

alter table public.messages
  add constraint messages_content_or_media
  check (
    char_length(content) <= 4000
    and (char_length(btrim(content)) >= 1 or media_kind is not null)
  ),
  add constraint messages_media_kind_valid
  check (media_kind is null or media_kind in ('image', 'video')),
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
        when 'video' then 'video/webm'
      end
      and media_size_bytes between 1 and 52428800
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
  );

create unique index messages_media_path_unique
  on public.messages (media_path)
  where media_path is not null;

create index messages_media_expiry_idx
  on public.messages (media_expires_at, created_at)
  where media_path is not null;

create or replace function private.enforce_message_media_retention()
returns trigger
language plpgsql
set search_path = ''
as $$
begin
  if tg_op = 'INSERT' then
    if new.media_kind is not null then
      new.media_expires_at := new.created_at + interval '3 days';
    end if;
    return new;
  end if;

  if new.media_kind is distinct from old.media_kind
    or new.media_mime_type is distinct from old.media_mime_type
    or new.media_size_bytes is distinct from old.media_size_bytes
    or new.media_width is distinct from old.media_width
    or new.media_height is distinct from old.media_height
    or new.media_duration_seconds is distinct from old.media_duration_seconds
    or new.media_expires_at is distinct from old.media_expires_at
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

revoke all on function private.enforce_message_media_retention()
  from public, anon, authenticated;

create trigger enforce_message_media_retention
  before insert or update on public.messages
  for each row execute function private.enforce_message_media_retention();

-- Safe parser for the flat chat object key: conversation_uuid_message_uuid.ext.
create or replace function private.media_conversation_id(object_name text)
returns uuid
language sql
immutable
set search_path = ''
as $$
  select case
    when split_part(object_name, '_', 1) ~
      '^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$'
    then split_part(object_name, '_', 1)::uuid
    else null
  end;
$$;

revoke all on function private.media_conversation_id(text) from public, anon;
grant execute on function private.media_conversation_id(text) to authenticated;

-- Signed uploads reserve the full per-file maximum under a transaction lock.
-- This keeps concurrent or modified clients from racing the 512 MiB budget.
create table private.chat_media_reservations (
  path text primary key,
  conversation_id uuid not null references public.conversations(id) on delete cascade,
  message_id uuid not null,
  user_id uuid not null references public.profiles(id) on delete cascade,
  reserved_bytes bigint not null default 52428800
    check (reserved_bytes = 52428800),
  created_at timestamptz not null default now(),
  expires_at timestamptz not null default now() + interval '15 minutes'
);

alter table private.chat_media_reservations enable row level security;
revoke all on table private.chat_media_reservations from public, anon, authenticated;

create index chat_media_reservations_expiry_idx
  on private.chat_media_reservations (expires_at);

create or replace function public.reserve_chat_media_upload(
  p_path text,
  p_conversation uuid,
  p_message uuid,
  p_user uuid
)
returns boolean
language plpgsql
security definer
set search_path = ''
as $$
declare
  stored_bytes bigint;
  reserved_bytes bigint;
begin
  if p_path not in (
      p_conversation::text || '_' || p_message::text || '.webp',
      p_conversation::text || '_' || p_message::text || '.webm'
    )
    or p_path !~ '^[0-9a-f-]{36}_[0-9a-f-]{36}\.(webp|webm)$'
  then
    return false;
  end if;

  if not exists (
    select 1
    from public.conversations c
    where c.id = p_conversation
      and p_user in (c.user1_id, c.user2_id)
  ) then
    return false;
  end if;

  perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtext('dislight-chat-media-quota'));

  delete from private.chat_media_reservations where expires_at <= now();

  select coalesce(sum(
    case
      when (o.metadata->>'size') ~ '^[0-9]+$' then (o.metadata->>'size')::bigint
      else 52428800
    end
  ), 0)
  into stored_bytes
  from storage.objects o
  where o.bucket_id = 'chat-media';

  select coalesce(sum(r.reserved_bytes), 0)
  into reserved_bytes
  from private.chat_media_reservations r
  where r.expires_at > now()
    and not exists (
      select 1 from storage.objects o
      where o.bucket_id = 'chat-media' and o.name = r.path
    );

  if stored_bytes + reserved_bytes + 52428800 > 536870912 then
    return false;
  end if;

  insert into private.chat_media_reservations (
    path, conversation_id, message_id, user_id
  ) values (
    p_path, p_conversation, p_message, p_user
  )
  on conflict (path) do update
    set expires_at = now() + interval '15 minutes';

  return true;
end;
$$;

revoke all on function public.reserve_chat_media_upload(text, uuid, uuid, uuid)
  from public, anon, authenticated;
grant execute on function public.reserve_chat_media_upload(text, uuid, uuid, uuid)
  to service_role;

create or replace function public.release_chat_media_reservations(p_paths text[])
returns void
language sql
security definer
set search_path = ''
as $$
  delete from private.chat_media_reservations where path = any(p_paths);
$$;

revoke all on function public.release_chat_media_reservations(text[])
  from public, anon, authenticated;
grant execute on function public.release_chat_media_reservations(text[])
  to service_role;
-- Bucket rows are deployment configuration. Object writes/deletes still go
-- through the Storage API so the underlying provider remains consistent.
insert into storage.buckets (id, name, public, file_size_limit, allowed_mime_types)
values
  ('chat-media', 'chat-media', false, 52428800, array['image/webp', 'video/webm']),
  ('avatars', 'avatars', true, 1048576, array['image/webp'])
on conflict (id) do update set
  public = excluded.public,
  file_size_limit = excluded.file_size_limit,
  allowed_mime_types = excluded.allowed_mime_types;

drop policy if exists "chat_media_participant_read" on storage.objects;
create policy "chat_media_participant_read"
on storage.objects for select
to authenticated
using (
  bucket_id = 'chat-media'
  and private.is_participant(private.media_conversation_id(name))
);

-- Chat uploads are deliberately not granted INSERT/UPDATE/DELETE policies.
-- The purge-chat-media Edge Function issues narrowly-scoped signed upload URLs
-- after checking participation, actual bucket usage, and retention.

drop policy if exists "avatar_owner_read" on storage.objects;
create policy "avatar_owner_read"
on storage.objects for select
to authenticated
using (
  bucket_id = 'avatars'
  and name = (select auth.uid())::text || '/avatar.webp'
);

drop policy if exists "avatar_owner_insert" on storage.objects;
create policy "avatar_owner_insert"
on storage.objects for insert
to authenticated
with check (
  bucket_id = 'avatars'
  and name = (select auth.uid())::text || '/avatar.webp'
);

drop policy if exists "avatar_owner_update" on storage.objects;
create policy "avatar_owner_update"
on storage.objects for update
to authenticated
using (
  bucket_id = 'avatars'
  and name = (select auth.uid())::text || '/avatar.webp'
)
with check (
  bucket_id = 'avatars'
  and name = (select auth.uid())::text || '/avatar.webp'
);

drop policy if exists "avatar_owner_delete" on storage.objects;
create policy "avatar_owner_delete"
on storage.objects for delete
to authenticated
using (
  bucket_id = 'avatars'
  and name = (select auth.uid())::text || '/avatar.webp'
);

-- Keep attachment-only messages meaningful in the sidebar without changing
-- the existing RPC response shape.
create or replace function public.conversation_overview()
returns table (
  conversation_id uuid,
  last_message_id uuid,
  last_message_content text,
  last_message_sender_id uuid,
  last_message_deleted boolean,
  last_message_at timestamptz,
  unread_count bigint
)
language sql
security invoker
stable
set search_path = ''
as $$
  select
    c.id,
    lm.id,
    coalesce(
      nullif(lm.content, ''),
      case lm.media_kind when 'image' then '[Image]' when 'video' then '[Video]' end
    ),
    lm.sender_id,
    lm.deleted_at is not null,
    lm.created_at,
    coalesce(uc.cnt, 0)
  from public.conversations c
  left join lateral (
    select m.id, m.content, m.sender_id, m.deleted_at, m.created_at, m.media_kind
    from public.messages m
    where m.conversation_id = c.id
    order by m.created_at desc
    limit 1
  ) lm on true
  left join lateral (
    select count(*) as cnt
    from public.messages m
    where m.conversation_id = c.id
      and m.sender_id <> (select auth.uid())
      and m.deleted_at is null
      and m.created_at > coalesce(
        (select r.last_read_at
         from public.conversation_reads r
         where r.conversation_id = c.id and r.user_id = (select auth.uid())),
        'epoch'::timestamptz
      )
  ) uc on true
$$;

revoke all on function public.conversation_overview() from public, anon;
grant execute on function public.conversation_overview() to authenticated;

create extension if not exists pg_net with schema extensions;
create extension if not exists pg_cron with schema pg_catalog;
grant usage on schema cron to postgres;

-- project_url and publishable_key are inserted into Vault during deployment,
-- not committed to this migration. Reusing the job name updates it safely.
select cron.schedule(
  'purge-chat-media-hourly',
  '17 * * * *',
  $job$
    select net.http_post(
      url := (select decrypted_secret from vault.decrypted_secrets where name = 'project_url')
        || '/functions/v1/purge-chat-media',
      headers := jsonb_build_object(
        'Content-Type', 'application/json',
        'apikey', (select decrypted_secret from vault.decrypted_secrets where name = 'publishable_key')
      ),
      body := jsonb_build_object('mode', 'scheduled'),
      timeout_milliseconds := 10000
    ) as request_id;
  $job$
);


