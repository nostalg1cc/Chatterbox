-- Move RLS helper functions to a non-API schema and remove RPC exposure of trigger functions
-- (fixes security advisor: anon/authenticated_security_definer_function_executable)

create schema if not exists private;
grant usage on schema private to authenticated;

create or replace function private.is_participant(conv_id uuid)
returns boolean
language sql
security definer
stable
set search_path = ''
as $$
  select exists (
    select 1 from public.conversations c
    where c.id = conv_id
      and (select auth.uid()) in (c.user1_id, c.user2_id)
  );
$$;

create or replace function private.message_conversation(msg_id uuid)
returns uuid
language sql
security definer
stable
set search_path = ''
as $$
  select m.conversation_id from public.messages m where m.id = msg_id;
$$;

revoke all on function private.is_participant(uuid) from public, anon;
revoke all on function private.message_conversation(uuid) from public, anon;
grant execute on function private.is_participant(uuid) to authenticated;
grant execute on function private.message_conversation(uuid) to authenticated;

-- Repoint policies at the private helpers
drop policy "messages_select" on public.messages;
create policy "messages_select" on public.messages
  for select to authenticated
  using (private.is_participant(conversation_id));

drop policy "messages_insert" on public.messages;
create policy "messages_insert" on public.messages
  for insert to authenticated
  with check (
    sender_id = (select auth.uid())
    and private.is_participant(conversation_id)
    and edited_at is null
    and deleted_at is null
  );

drop policy "reactions_select" on public.reactions;
create policy "reactions_select" on public.reactions
  for select to authenticated
  using (private.is_participant(private.message_conversation(message_id)));

drop policy "reactions_insert" on public.reactions;
create policy "reactions_insert" on public.reactions
  for insert to authenticated
  with check (
    user_id = (select auth.uid())
    and private.is_participant(private.message_conversation(message_id))
  );

drop policy "reads_insert_own" on public.conversation_reads;
create policy "reads_insert_own" on public.conversation_reads
  for insert to authenticated
  with check (user_id = (select auth.uid()) and private.is_participant(conversation_id));

drop function public.is_participant(uuid);
drop function public.message_conversation(uuid);

-- Trigger functions run as owner at trigger time; no role needs RPC EXECUTE on them
revoke execute on function public.handle_new_user() from public, anon, authenticated;
revoke execute on function public.handle_friendship_accepted() from public, anon, authenticated;
revoke execute on function public.handle_new_message() from public, anon, authenticated;
revoke execute on function public.handle_updated_at() from public, anon, authenticated;
