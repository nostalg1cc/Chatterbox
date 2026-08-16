-- Short/test calls (< 15 minutes) shouldn't leave "Voice chat started" /
-- "Call lasted N" clutter in chat history. The "started" message still
-- appears immediately when a call begins (so it's visible in real time,
-- same as before) - but once the call actually finalizes, if it turns out
-- to have lasted under 15 minutes, both messages are purged instead of
-- inserting the "ended" one. Realtime DELETE needs the full old row to pass
-- messages_select's conversation_id-based RLS check (see the
-- realtime_publication migration's identical reasoning for reactions and
-- friendships), so messages needs the same replica identity change.
alter table public.messages replica identity full;

alter table public.voice_rooms
  add column if not exists started_message_id uuid references public.messages(id) on delete set null;

create or replace function private.cleanup_stale_voice_rooms()
returns void
language plpgsql
security definer
set search_path = ''
as $$
declare
  v_grace_seconds constant integer := 20;
  v_short_call_seconds constant integer := 900;
  v_stale_room record;
  v_duration_seconds integer;
begin
  perform pg_advisory_xact_lock(
    hashtextextended('dislight:voice-membership', 0)
  );

  delete from public.voice_participants
  where last_seen_at < now() - interval '120 seconds';

  -- A room a stale participant just vacated may now be empty and wasn't
  -- marked empty_since by a deliberate leave - mark it so it goes through
  -- the same grace period rather than vanishing without a "Call lasted".
  update public.voice_rooms r
  set empty_since = now()
  where r.empty_since is null
    and not exists (select 1 from public.voice_participants p where p.conversation_id = r.conversation_id);

  for v_stale_room in
    select r.conversation_id, r.started_by, r.both_joined_at, r.empty_since, r.started_message_id
    from public.voice_rooms r
    where r.empty_since is not null
      and r.empty_since < now() - make_interval(secs => v_grace_seconds)
      and not exists (select 1 from public.voice_participants p where p.conversation_id = r.conversation_id)
  loop
    v_duration_seconds := case
      when v_stale_room.both_joined_at is null then 0
      else greatest(0, floor(extract(epoch from (v_stale_room.empty_since - v_stale_room.both_joined_at)))::integer)
    end;

    if v_duration_seconds < v_short_call_seconds then
      -- Too short to be worth a system message - remove the "started" line
      -- too rather than leaving an orphaned start with no matching end.
      -- (Rooms created before this migration have no started_message_id to
      -- clean up - nothing to do for those, they just won't get an "ended"
      -- message either.)
      if v_stale_room.started_message_id is not null then
        delete from public.messages where id = v_stale_room.started_message_id;
      end if;
    else
      insert into public.messages (conversation_id, sender_id, content, message_kind, voice_duration_seconds)
      values (v_stale_room.conversation_id, v_stale_room.started_by, 'Voice chat ended', 'voice_ended', v_duration_seconds);
    end if;

    delete from public.voice_rooms where conversation_id = v_stale_room.conversation_id;
  end loop;
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
  v_created_room boolean := false;
  v_started_message_id uuid;
begin
  if v_user_id is null then
    raise exception 'Authentication required' using errcode = '28000';
  end if;
  if p_session_id is null then
    raise exception 'A voice session id is required' using errcode = '22004';
  end if;
  if not private.is_participant(p_conversation_id) then
    raise exception 'Not a participant of this conversation' using errcode = '42501';
  end if;

  perform private.cleanup_stale_voice_rooms();
  select * into v_existing from public.voice_participants where user_id = v_user_id;

  if found and v_existing.conversation_id = p_conversation_id and v_existing.session_id = p_session_id then
    update public.voice_participants set last_seen_at = now()
      where user_id = v_user_id and session_id = p_session_id;
  elsif found and not p_takeover then
    return jsonb_build_object('status', 'conflict', 'conversation_id', v_existing.conversation_id, 'joined_at', v_existing.joined_at);
  else
    if found then
      v_replaced_conversation_id := v_existing.conversation_id;
      delete from public.voice_participants where user_id = v_user_id;
      if v_existing.conversation_id <> p_conversation_id then
        delete from public.voice_rooms r
          where r.conversation_id = v_existing.conversation_id
            and not exists (select 1 from public.voice_participants p where p.conversation_id = r.conversation_id);
      end if;
    end if;

    select * into v_room from public.voice_rooms where conversation_id = p_conversation_id;
    if not found then
      insert into public.voice_rooms (conversation_id, started_by)
      values (p_conversation_id, v_user_id)
      returning * into v_room;
      v_created_room := true;
    end if;

    insert into public.voice_participants (conversation_id, user_id, session_id)
    values (p_conversation_id, v_user_id, p_session_id);

    if v_created_room then
      insert into public.messages (conversation_id, sender_id, content, message_kind)
      values (p_conversation_id, v_user_id, 'Voice chat started', 'voice_started')
      returning id into v_started_message_id;
    end if;

    -- Rejoining within the grace period resumes the same call - clear the
    -- pending-end marker instead of letting it finalize underneath us.
    -- Also mark the first moment the room reaches 2 participants, so the
    -- eventual duration only counts time spent together.
    update public.voice_rooms
      set updated_at = now(),
          empty_since = null,
          started_message_id = coalesce(v_started_message_id, started_message_id),
          both_joined_at = coalesce(
            both_joined_at,
            case
              when (select count(*) from public.voice_participants p where p.conversation_id = p_conversation_id) >= 2
                then now()
              else null
            end
          )
      where conversation_id = p_conversation_id;
  end if;

  select * into v_room from public.voice_rooms where conversation_id = p_conversation_id;
  select coalesce(jsonb_agg(jsonb_build_object(
    'user_id', p.user_id, 'session_id', p.session_id, 'joined_at', p.joined_at,
    'last_seen_at', p.last_seen_at, 'sharing_screen', p.sharing_screen
  ) order by p.joined_at), '[]'::jsonb)
  into v_participants
  from public.voice_participants p where p.conversation_id = p_conversation_id;

  return jsonb_build_object(
    'status', 'joined', 'conversation_id', v_room.conversation_id,
    'generation', v_room.generation, 'started_at', v_room.started_at,
    'started_by', v_room.started_by, 'participants', v_participants,
    'replaced_conversation_id', v_replaced_conversation_id
  );
end;
$$;
