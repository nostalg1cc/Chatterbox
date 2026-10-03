-- Keep privileged access behind an authenticated private implementation.
create or replace function private.secure_join_voice_room(p_conversation_id uuid,p_session_id uuid,p_takeover boolean default false)
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
revoke all on function private.secure_join_voice_room(uuid,uuid,boolean) from public,anon,service_role;
grant execute on function private.secure_join_voice_room(uuid,uuid,boolean) to authenticated;
create or replace function public.join_voice_room(p_conversation_id uuid,p_session_id uuid,p_takeover boolean default false)
returns jsonb language sql volatile security invoker set search_path = '' as $$
  select private.secure_join_voice_room(p_conversation_id,p_session_id,p_takeover);
$$;
