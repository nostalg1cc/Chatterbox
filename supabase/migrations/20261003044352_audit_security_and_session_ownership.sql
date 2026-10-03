-- Immutable friendship identities: accepting a request cannot change its parties.
create or replace function private.protect_friendship_identity()
returns trigger language plpgsql set search_path = '' as $$
begin
  if new.id is distinct from old.id or new.requester_id is distinct from old.requester_id
    or new.addressee_id is distinct from old.addressee_id or new.created_at is distinct from old.created_at then
    raise exception 'Friendship identity is immutable' using errcode = '42501';
  end if;
  return new;
end;
$$;
create trigger protect_friendship_identity before update on public.friendships
for each row execute function private.protect_friendship_identity();
revoke update on public.friendships from authenticated, anon;
grant update (status) on public.friendships to authenticated;

-- Service-only, atomic fixed-window quotas. No raw network addresses are stored.
create table private.edge_request_limits (
  key text not null, action text not null, bucket bigint not null, hits integer not null,
  expires_at timestamptz not null, primary key (key, action, bucket)
);
create index edge_request_limits_expiry on private.edge_request_limits(expires_at);
alter table private.edge_request_limits enable row level security;
revoke all on private.edge_request_limits from public, anon, authenticated;
create or replace function public.consume_edge_quota(p_key text, p_action text, p_limit integer, p_window_seconds integer)
returns boolean language plpgsql security definer set search_path = '' as $$
declare v_hits integer; v_bucket bigint;
begin
  if p_limit < 1 or p_window_seconds < 1 or p_window_seconds > 86400 or length(p_key) > 200 then
    raise exception 'Invalid quota';
  end if;
  v_bucket := floor(extract(epoch from now()) / p_window_seconds)::bigint;
  delete from private.edge_request_limits where expires_at < now();
  insert into private.edge_request_limits as limits (key,action,bucket,hits,expires_at)
  values (p_key,p_action,v_bucket,1,now()+make_interval(secs => p_window_seconds * 2))
  on conflict (key,action,bucket) do update set hits=limits.hits+1 returning hits into v_hits;
  return v_hits <= p_limit;
end;
$$;
revoke all on function public.consume_edge_quota(text,text,integer,integer) from public,anon,authenticated;
grant execute on function public.consume_edge_quota(text,text,integer,integer) to service_role;

alter table private.cloudflare_screen_sessions add column voice_session_id uuid;
-- Old provider sessions remain inaccessible; new sessions must carry a live voice lease.

-- The channel secret is disclosed only to the current signed-in voice seat.
create table private.voice_signaling_access (
  conversation_id uuid primary key references public.voice_rooms(conversation_id) on delete cascade,
  generation uuid not null, token uuid not null default gen_random_uuid()
);
alter table private.voice_signaling_access enable row level security;
revoke all on private.voice_signaling_access from public,anon,authenticated;
-- Revoke cached channel capabilities after every departed or replaced seat.
create or replace function private.rotate_voice_signaling_on_leave()
returns trigger language plpgsql security definer set search_path = '' as $$
declare v_generation uuid := gen_random_uuid();
begin
  update public.voice_rooms set generation=v_generation,updated_at=now()
    where conversation_id=old.conversation_id;
  update private.voice_signaling_access set generation=v_generation,token=gen_random_uuid()
    where conversation_id=old.conversation_id;
  return old;
end;
$$;
create trigger rotate_voice_signaling_on_leave after delete on public.voice_participants
for each row execute function private.rotate_voice_signaling_on_leave();
create or replace function public.join_voice_room(p_conversation_id uuid,p_session_id uuid,p_takeover boolean default false)
returns jsonb language plpgsql security definer set search_path = '' as $$
declare v_result jsonb; v_token uuid; v_previous uuid;
begin
  perform pg_advisory_xact_lock(hashtext('nitro-voice-signaling'));
  select session_id into v_previous from public.voice_participants where user_id=auth.uid();
  v_result := private.join_voice_room(p_conversation_id,p_session_id,p_takeover);
  if v_result->>'status' <> 'joined' then return v_result; end if;
  if p_takeover and v_previous is not null and v_previous <> p_session_id then
    update public.voice_rooms set generation=gen_random_uuid(),updated_at=now()
      where conversation_id=p_conversation_id;
    v_result := jsonb_set(v_result,'{generation}',to_jsonb((select generation from public.voice_rooms where conversation_id=p_conversation_id)));
  end if;
  insert into private.voice_signaling_access as access(conversation_id,generation)
    values(p_conversation_id,(v_result->>'generation')::uuid)
    on conflict(conversation_id) do update set generation=excluded.generation,
      token=case when access.generation=excluded.generation then access.token else gen_random_uuid() end
    returning token into v_token;
  return v_result || jsonb_build_object('channel_token',v_token);
end;
$$;
revoke execute on function private.join_voice_room(uuid,uuid,boolean) from authenticated;
create or replace function private.can_access_voice_topic(p_topic text)
returns boolean language plpgsql stable security definer set search_path = '' as $$
begin
  if p_topic !~* '^voice:[0-9a-f-]{36}:[0-9a-f-]{36}:[0-9a-f-]{36}$' then return false; end if;
  return exists (
    select 1 from private.voice_signaling_access a join public.voice_participants p using(conversation_id)
    where a.conversation_id=split_part(p_topic,':',2)::uuid
      and a.generation=split_part(p_topic,':',3)::uuid and a.token=split_part(p_topic,':',4)::uuid
      and p.user_id=auth.uid() and p.last_seen_at>now()-interval '120 seconds'
      and private.is_participant(a.conversation_id)
  );
exception when invalid_text_representation then return false;
end;
$$;

-- The public JWT alone must never authorize scheduled destructive cleanup.
select vault.create_secret(encode(extensions.gen_random_bytes(32),'hex'),'chat_media_cleanup_key')
where not exists(select 1 from vault.secrets where name='chat_media_cleanup_key');
create or replace function public.validate_media_cleanup_key(p_key text)
returns boolean language sql stable security definer set search_path = '' as $$
  select length(p_key)=64 and exists(select 1 from vault.decrypted_secrets
    where name='chat_media_cleanup_key' and decrypted_secret=p_key);
$$;
revoke all on function public.validate_media_cleanup_key(text) from public,anon,authenticated;
grant execute on function public.validate_media_cleanup_key(text) to service_role;
select cron.schedule('purge-chat-media-hourly','17 * * * *',$job$
  select net.http_post(
    url := (select decrypted_secret from vault.decrypted_secrets where name='project_url') || '/functions/v1/purge-chat-media',
    headers := jsonb_build_object('Content-Type','application/json',
      'apikey',(select decrypted_secret from vault.decrypted_secrets where name='publishable_key'),
      'Authorization','Bearer '||(select decrypted_secret from vault.decrypted_secrets where name='legacy_anon_key'),
      'x-cleanup-key',(select decrypted_secret from vault.decrypted_secrets where name='chat_media_cleanup_key')),
    body := jsonb_build_object('mode','scheduled'),timeout_milliseconds := 10000);
$job$);
