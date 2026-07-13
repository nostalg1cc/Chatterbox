-- Voice channels: durable room discovery + expiring participant leases.
-- WebRTC media remains peer-to-peer; Supabase stores metadata and carries signaling only.

create table public.voice_rooms (
  conversation_id uuid primary key references public.conversations(id) on delete cascade,
  generation uuid not null default gen_random_uuid(),
  started_at timestamptz not null default now(),
  started_by uuid not null references public.profiles(id) on delete cascade,
  updated_at timestamptz not null default now(),
  constraint voice_rooms_generation_unique unique (generation)
);

create index voice_rooms_started_by_idx
  on public.voice_rooms (started_by);

create table public.voice_participants (
  conversation_id uuid not null references public.voice_rooms(conversation_id) on delete cascade,
  user_id uuid not null references public.profiles(id) on delete cascade,
  session_id uuid not null,
  joined_at timestamptz not null default now(),
  last_seen_at timestamptz not null default now(),
  sharing_screen boolean not null default false,
  primary key (conversation_id, user_id),
  constraint voice_participants_user_unique unique (user_id),
  constraint voice_participants_session_unique unique (session_id)
);

create index voice_participants_last_seen_idx
  on public.voice_participants (last_seen_at);

alter table public.voice_rooms enable row level security;
alter table public.voice_participants enable row level security;

grant select on public.voice_rooms, public.voice_participants to authenticated;
revoke insert, update, delete on public.voice_rooms, public.voice_participants
  from anon, authenticated;

create policy "voice_rooms_participant_select"
on public.voice_rooms
for select
to authenticated
using ((select private.is_participant(conversation_id)));

create policy "voice_participants_participant_select"
on public.voice_participants
for select
to authenticated
using ((select private.is_participant(conversation_id)));

create or replace function private.cleanup_stale_voice_rooms()
returns void
language plpgsql
security definer
set search_path = ''
as $$
begin
  perform pg_advisory_xact_lock(
    hashtextextended('dislight:voice-membership', 0)
  );

  delete from public.voice_participants
  where last_seen_at < now() - interval '120 seconds';

  delete from public.voice_rooms r
  where not exists (
    select 1
    from public.voice_participants p
    where p.conversation_id = r.conversation_id
  );
end;
$$;

create or replace function private.join_voice_room(
  p_conversation_id uuid,
  p_session_id uuid,
  p_takeover boolean default false
)
returns jsonb
language plpgsql
security definer
set search_path = ''
as $$
declare
  v_user_id uuid := (select auth.uid());
  v_existing public.voice_participants%rowtype;
  v_room public.voice_rooms%rowtype;
  v_replaced_conversation_id uuid;
  v_participants jsonb;
begin
  if v_user_id is null then
    raise exception 'Authentication required'
      using errcode = '28000';
  end if;

  if p_session_id is null then
    raise exception 'A voice session id is required'
      using errcode = '22004';
  end if;

  if not private.is_participant(p_conversation_id) then
    raise exception 'Not a participant of this conversation'
      using errcode = '42501';
  end if;

  -- Cleanup also acquires the short transaction-level membership lock.
  perform private.cleanup_stale_voice_rooms();

  select *
  into v_existing
  from public.voice_participants
  where user_id = v_user_id;

  if found
     and v_existing.conversation_id = p_conversation_id
     and v_existing.session_id = p_session_id then
    update public.voice_participants
    set last_seen_at = now()
    where user_id = v_user_id
      and session_id = p_session_id;
  elsif found and not p_takeover then
    return jsonb_build_object(
      'status', 'conflict',
      'conversation_id', v_existing.conversation_id,
      'joined_at', v_existing.joined_at
    );
  else
    if found then
      v_replaced_conversation_id := v_existing.conversation_id;

      delete from public.voice_participants
      where user_id = v_user_id;

      if v_existing.conversation_id <> p_conversation_id then
        delete from public.voice_rooms r
        where r.conversation_id = v_existing.conversation_id
          and not exists (
            select 1
            from public.voice_participants p
            where p.conversation_id = r.conversation_id
          );
      end if;
    end if;

    select *
    into v_room
    from public.voice_rooms
    where conversation_id = p_conversation_id;

    if not found then
      insert into public.voice_rooms (
        conversation_id,
        started_by
      )
      values (
        p_conversation_id,
        v_user_id
      )
      returning * into v_room;
    end if;

    insert into public.voice_participants (
      conversation_id,
      user_id,
      session_id
    )
    values (
      p_conversation_id,
      v_user_id,
      p_session_id
    );

    update public.voice_rooms
    set updated_at = now()
    where conversation_id = p_conversation_id;
  end if;

  select *
  into v_room
  from public.voice_rooms
  where conversation_id = p_conversation_id;

  select coalesce(
    jsonb_agg(
      jsonb_build_object(
        'user_id', p.user_id,
        'session_id', p.session_id,
        'joined_at', p.joined_at,
        'last_seen_at', p.last_seen_at,
        'sharing_screen', p.sharing_screen
      )
      order by p.joined_at
    ),
    '[]'::jsonb
  )
  into v_participants
  from public.voice_participants p
  where p.conversation_id = p_conversation_id;

  return jsonb_build_object(
    'status', 'joined',
    'conversation_id', v_room.conversation_id,
    'generation', v_room.generation,
    'started_at', v_room.started_at,
    'started_by', v_room.started_by,
    'participants', v_participants,
    'replaced_conversation_id', v_replaced_conversation_id
  );
end;
$$;

create or replace function private.heartbeat_voice_room(
  p_session_id uuid,
  p_sharing_screen boolean default false
)
returns jsonb
language plpgsql
security definer
set search_path = ''
as $$
declare
  v_user_id uuid := (select auth.uid());
  v_conversation_id uuid;
begin
  if v_user_id is null then
    raise exception 'Authentication required'
      using errcode = '28000';
  end if;

  update public.voice_participants
  set
    last_seen_at = now(),
    sharing_screen = p_sharing_screen
  where user_id = v_user_id
    and session_id = p_session_id
  returning conversation_id into v_conversation_id;

  if v_conversation_id is null then
    return jsonb_build_object('status', 'not_found');
  end if;

  return jsonb_build_object(
    'status', 'ok',
    'conversation_id', v_conversation_id
  );
end;
$$;

create or replace function private.leave_voice_room(
  p_session_id uuid
)
returns jsonb
language plpgsql
security definer
set search_path = ''
as $$
declare
  v_user_id uuid := (select auth.uid());
  v_conversation_id uuid;
begin
  if v_user_id is null then
    raise exception 'Authentication required'
      using errcode = '28000';
  end if;

  perform pg_advisory_xact_lock(
    hashtextextended('dislight:voice-membership', 0)
  );

  delete from public.voice_participants
  where user_id = v_user_id
    and session_id = p_session_id
  returning conversation_id into v_conversation_id;

  if v_conversation_id is null then
    return jsonb_build_object('status', 'not_found');
  end if;

  delete from public.voice_rooms r
  where r.conversation_id = v_conversation_id
    and not exists (
      select 1
      from public.voice_participants p
      where p.conversation_id = r.conversation_id
    );

  update public.voice_rooms
  set updated_at = now()
  where conversation_id = v_conversation_id;

  return jsonb_build_object(
    'status', 'left',
    'conversation_id', v_conversation_id
  );
end;
$$;

create or replace function public.join_voice_room(
  p_conversation_id uuid,
  p_session_id uuid,
  p_takeover boolean default false
)
returns jsonb
language sql
volatile
security invoker
set search_path = ''
as $$
  select private.join_voice_room(
    p_conversation_id,
    p_session_id,
    p_takeover
  );
$$;

create or replace function public.heartbeat_voice_room(
  p_session_id uuid,
  p_sharing_screen boolean default false
)
returns jsonb
language sql
volatile
security invoker
set search_path = ''
as $$
  select private.heartbeat_voice_room(
    p_session_id,
    p_sharing_screen
  );
$$;

create or replace function public.leave_voice_room(
  p_session_id uuid
)
returns jsonb
language sql
volatile
security invoker
set search_path = ''
as $$
  select private.leave_voice_room(p_session_id);
$$;

revoke all on function private.cleanup_stale_voice_rooms()
  from public, anon, authenticated, service_role;
revoke all on function private.join_voice_room(uuid, uuid, boolean)
  from public, anon, service_role;
revoke all on function private.heartbeat_voice_room(uuid, boolean)
  from public, anon, service_role;
revoke all on function private.leave_voice_room(uuid)
  from public, anon, service_role;

grant execute on function private.join_voice_room(uuid, uuid, boolean)
  to authenticated;
grant execute on function private.heartbeat_voice_room(uuid, boolean)
  to authenticated;
grant execute on function private.leave_voice_room(uuid)
  to authenticated;

revoke all on function public.join_voice_room(uuid, uuid, boolean)
  from public, anon;
revoke all on function public.heartbeat_voice_room(uuid, boolean)
  from public, anon;
revoke all on function public.leave_voice_room(uuid)
  from public, anon;

grant execute on function public.join_voice_room(uuid, uuid, boolean)
  to authenticated;
grant execute on function public.heartbeat_voice_room(uuid, boolean)
  to authenticated;
grant execute on function public.leave_voice_room(uuid)
  to authenticated;

create or replace function private.voice_topic_conversation(
  p_topic text
)
returns uuid
language sql
immutable
security invoker
set search_path = ''
as $$
  select case
    when p_topic ~* (
      '^voice:' ||
      '[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}:' ||
      '[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$'
    )
      then split_part(p_topic, ':', 2)::uuid
    else null
  end;
$$;

create or replace function private.can_access_voice_topic(
  p_topic text
)
returns boolean
language plpgsql
stable
security definer
set search_path = ''
as $$
declare
  v_conversation_id uuid;
  v_generation uuid;
begin
  v_conversation_id := private.voice_topic_conversation(p_topic);
  if v_conversation_id is null then
    return false;
  end if;

  v_generation := split_part(p_topic, ':', 3)::uuid;

  return private.is_participant(v_conversation_id)
    and exists (
      select 1
      from public.voice_rooms r
      where r.conversation_id = v_conversation_id
        and r.generation = v_generation
    );
end;
$$;

revoke all on function private.voice_topic_conversation(text)
  from public, anon, authenticated, service_role;
revoke all on function private.can_access_voice_topic(text)
  from public, anon, service_role;
grant execute on function private.can_access_voice_topic(text)
  to authenticated;

alter table realtime.messages enable row level security;

create policy "voice_channel_receive"
on realtime.messages
for select
to authenticated
using (
  realtime.messages.extension in ('broadcast', 'presence')
  and (select private.can_access_voice_topic((select realtime.topic())))
);

create policy "voice_channel_send"
on realtime.messages
for insert
to authenticated
with check (
  realtime.messages.extension in ('broadcast', 'presence')
  and (select private.can_access_voice_topic((select realtime.topic())))
);

alter publication supabase_realtime
  add table public.voice_rooms, public.voice_participants;

select cron.schedule(
  'cleanup-stale-voice-rooms',
  '* * * * *',
  'select private.cleanup_stale_voice_rooms();'
);