-- RPC helpers for the client

-- Signup-time username availability (anon-callable by design: signup happens before login.
-- SECURITY DEFINER is intentional; returns only a boolean for one exact username.)
create or replace function public.username_available(check_username text)
returns boolean
language sql
security definer
stable
set search_path = ''
as $$
  select not exists (
    select 1 from public.profiles p where p.username = lower(check_username)
  );
$$;

revoke all on function public.username_available(text) from public;
grant execute on function public.username_available(text) to anon, authenticated;

-- Sidebar overview: last message + unread count per conversation, one round trip.
-- SECURITY INVOKER: RLS on conversations/messages/conversation_reads applies to the caller.
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
    lm.content,
    lm.sender_id,
    lm.deleted_at is not null,
    lm.created_at,
    coalesce(uc.cnt, 0)
  from public.conversations c
  left join lateral (
    select m.id, m.content, m.sender_id, m.deleted_at, m.created_at
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
