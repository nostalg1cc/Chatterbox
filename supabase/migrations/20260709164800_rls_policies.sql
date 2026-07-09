-- Dislight RLS: enable on all tables, participant helpers, per-table policies
-- NOTE: the helper functions created here in `public` were later moved to the
-- `private` schema by 20260709165000_lock_down_function_exposure.sql

alter table public.profiles enable row level security;
alter table public.friendships enable row level security;
alter table public.conversations enable row level security;
alter table public.messages enable row level security;
alter table public.reactions enable row level security;
alter table public.conversation_reads enable row level security;

-- Helper: is the current user a participant of this conversation?
-- SECURITY DEFINER so policies on messages/reactions don't recurse into conversations RLS.
create or replace function public.is_participant(conv_id uuid)
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

-- Helper: conversation a message belongs to (bypasses messages RLS for reaction policies)
create or replace function public.message_conversation(msg_id uuid)
returns uuid
language sql
security definer
stable
set search_path = ''
as $$
  select m.conversation_id from public.messages m where m.id = msg_id;
$$;

revoke execute on function public.is_participant(uuid) from anon;
revoke execute on function public.message_conversation(uuid) from anon;

-- profiles: readable by any signed-in user (friend search); only own row updatable
create policy "profiles_select" on public.profiles
  for select to authenticated using (true);
create policy "profiles_update_own" on public.profiles
  for update to authenticated
  using (id = (select auth.uid()))
  with check (id = (select auth.uid()));

-- friendships: visible to both parties; requester creates pending; addressee accepts/blocks; either deletes
create policy "friendships_select" on public.friendships
  for select to authenticated
  using (requester_id = (select auth.uid()) or addressee_id = (select auth.uid()));
create policy "friendships_insert" on public.friendships
  for insert to authenticated
  with check (requester_id = (select auth.uid()) and status = 'pending');
create policy "friendships_update_addressee" on public.friendships
  for update to authenticated
  using (addressee_id = (select auth.uid()) and status = 'pending')
  with check (status in ('accepted', 'blocked'));
create policy "friendships_delete" on public.friendships
  for delete to authenticated
  using (requester_id = (select auth.uid()) or addressee_id = (select auth.uid()));

-- conversations: participants read; created only by trigger (no insert policy)
create policy "conversations_select" on public.conversations
  for select to authenticated
  using (user1_id = (select auth.uid()) or user2_id = (select auth.uid()));

-- messages: participants read; sender inserts clean rows; sender edits/soft-deletes own
create policy "messages_select" on public.messages
  for select to authenticated
  using (public.is_participant(conversation_id));
create policy "messages_insert" on public.messages
  for insert to authenticated
  with check (
    sender_id = (select auth.uid())
    and public.is_participant(conversation_id)
    and edited_at is null
    and deleted_at is null
  );
create policy "messages_update_own" on public.messages
  for update to authenticated
  using (sender_id = (select auth.uid()))
  with check (sender_id = (select auth.uid()));

-- reactions: participants read; own reactions insert/delete only
create policy "reactions_select" on public.reactions
  for select to authenticated
  using (public.is_participant(public.message_conversation(message_id)));
create policy "reactions_insert" on public.reactions
  for insert to authenticated
  with check (
    user_id = (select auth.uid())
    and public.is_participant(public.message_conversation(message_id))
  );
create policy "reactions_delete_own" on public.reactions
  for delete to authenticated
  using (user_id = (select auth.uid()));

-- conversation_reads: own rows only, must be participant
create policy "reads_select_own" on public.conversation_reads
  for select to authenticated
  using (user_id = (select auth.uid()));
create policy "reads_insert_own" on public.conversation_reads
  for insert to authenticated
  with check (user_id = (select auth.uid()) and public.is_participant(conversation_id));
create policy "reads_update_own" on public.conversation_reads
  for update to authenticated
  using (user_id = (select auth.uid()))
  with check (user_id = (select auth.uid()));
