-- Service-only invoker RPCs bridge PostgREST to the unexposed private table.
-- The private schema and table remain inaccessible to application roles.
create or replace function public.read_cloudflare_screen_session(
  p_session_id text, p_conversation_id uuid
) returns jsonb
language sql stable security invoker set search_path = ''
as $$
  select to_jsonb(s)
  from private.cloudflare_screen_sessions s
  where s.session_id = p_session_id
    and s.conversation_id = p_conversation_id
    and s.expires_at > now();
$$;
revoke all on function public.read_cloudflare_screen_session(text,uuid) from public, anon, authenticated;
grant execute on function public.read_cloudflare_screen_session(text,uuid) to service_role;

alter table private.cloudflare_screen_sessions
  drop constraint cloudflare_screen_sessions_track_name_check;
alter table private.cloudflare_screen_sessions
  add constraint cloudflare_screen_sessions_track_name_check
  check (track_name is null or char_length(track_name) between 1 and 643);

create or replace function public.save_cloudflare_screen_session(
  p_session_id text, p_conversation_id uuid, p_owner_id uuid,
  p_voice_session_id uuid, p_track_name text default null
) returns boolean
language plpgsql security invoker set search_path = ''
as $$
declare
  v_count integer;
begin
  if not exists (
    select 1 from public.voice_participants v
    where v.conversation_id = p_conversation_id
      and v.user_id = p_owner_id and v.session_id = p_voice_session_id
      and v.last_seen_at > now() - interval '120 seconds'
  ) then
    return false;
  end if;

  if p_track_name is null then
    delete from private.cloudflare_screen_sessions where expires_at <= now();
    insert into private.cloudflare_screen_sessions as s
      (session_id, conversation_id, owner_id, voice_session_id, updated_at, expires_at)
    values
      (p_session_id, p_conversation_id, p_owner_id, p_voice_session_id, now(), now() + interval '2 hours')
    on conflict (session_id) do update
      set updated_at = now(), expires_at = now() + interval '2 hours'
      where s.conversation_id = excluded.conversation_id
        and s.owner_id = excluded.owner_id
        and s.voice_session_id = excluded.voice_session_id;
  else
    update private.cloudflare_screen_sessions
      set track_name = p_track_name, updated_at = now(),
          expires_at = now() + interval '2 hours'
      where session_id = p_session_id and conversation_id = p_conversation_id
        and owner_id = p_owner_id and voice_session_id = p_voice_session_id
        and expires_at > now();
  end if;
  get diagnostics v_count = row_count;
  return v_count = 1;
end;
$$;
revoke all on function public.save_cloudflare_screen_session(text,uuid,uuid,uuid,text) from public, anon, authenticated;
grant execute on function public.save_cloudflare_screen_session(text,uuid,uuid,uuid,text) to service_role;
notify pgrst, 'reload schema';
