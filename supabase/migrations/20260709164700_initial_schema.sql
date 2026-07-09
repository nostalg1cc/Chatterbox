-- Dislight initial schema: profiles, friendships, conversations, messages, reactions, conversation_reads
-- (mirror of migration applied to project lapjrxdgcbdseskmyfru via Supabase MCP)

create type public.friendship_status as enum ('pending', 'accepted', 'blocked');

-- Profiles (created automatically on signup via trigger)
create table public.profiles (
  id uuid primary key references auth.users(id) on delete cascade,
  username text not null unique,
  display_name text not null,
  created_at timestamptz not null default now(),
  constraint username_format check (username ~ '^[a-z0-9_]{3,20}$'),
  constraint display_name_length check (char_length(display_name) between 1 and 50)
);

-- Friend requests / friendships
create table public.friendships (
  id uuid primary key default gen_random_uuid(),
  requester_id uuid not null references public.profiles(id) on delete cascade,
  addressee_id uuid not null references public.profiles(id) on delete cascade,
  status public.friendship_status not null default 'pending',
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  constraint no_self_friendship check (requester_id <> addressee_id)
);
create unique index friendships_pair_unique
  on public.friendships (least(requester_id, addressee_id), greatest(requester_id, addressee_id));
create index friendships_addressee_idx on public.friendships (addressee_id);
create index friendships_requester_idx on public.friendships (requester_id);

-- 1:1 conversations (ordered pair, auto-created when a friendship is accepted)
create table public.conversations (
  id uuid primary key default gen_random_uuid(),
  user1_id uuid not null references public.profiles(id) on delete cascade,
  user2_id uuid not null references public.profiles(id) on delete cascade,
  created_at timestamptz not null default now(),
  last_message_at timestamptz not null default now(),
  constraint ordered_pair check (user1_id < user2_id),
  constraint conversations_pair_unique unique (user1_id, user2_id)
);
create index conversations_user2_idx on public.conversations (user2_id);

-- Messages (soft delete via deleted_at)
create table public.messages (
  id uuid primary key default gen_random_uuid(),
  conversation_id uuid not null references public.conversations(id) on delete cascade,
  sender_id uuid not null references public.profiles(id) on delete cascade,
  content text not null,
  created_at timestamptz not null default now(),
  edited_at timestamptz,
  deleted_at timestamptz,
  constraint content_length check (char_length(content) between 1 and 4000)
);
create index messages_conversation_created_idx on public.messages (conversation_id, created_at desc);
create index messages_sender_idx on public.messages (sender_id);

-- Emoji reactions
create table public.reactions (
  id uuid primary key default gen_random_uuid(),
  message_id uuid not null references public.messages(id) on delete cascade,
  user_id uuid not null references public.profiles(id) on delete cascade,
  emoji text not null,
  created_at timestamptz not null default now(),
  constraint emoji_length check (char_length(emoji) between 1 and 8),
  constraint reactions_unique unique (message_id, user_id, emoji)
);
create index reactions_message_idx on public.reactions (message_id);
create index reactions_user_idx on public.reactions (user_id);

-- Per-user read markers (unread counts)
create table public.conversation_reads (
  conversation_id uuid not null references public.conversations(id) on delete cascade,
  user_id uuid not null references public.profiles(id) on delete cascade,
  last_read_at timestamptz not null default now(),
  primary key (conversation_id, user_id)
);
create index conversation_reads_user_idx on public.conversation_reads (user_id);

-- Trigger: create profile on signup (username/display_name from signup metadata)
create or replace function public.handle_new_user()
returns trigger
language plpgsql
security definer
set search_path = ''
as $$
declare
  v_username text;
  v_display_name text;
begin
  v_username := lower(coalesce(nullif(new.raw_user_meta_data->>'username', ''), split_part(new.email, '@', 1)));
  v_display_name := coalesce(nullif(new.raw_user_meta_data->>'display_name', ''), v_username);
  insert into public.profiles (id, username, display_name)
  values (new.id, v_username, v_display_name);
  return new;
end;
$$;

create trigger on_auth_user_created
  after insert on auth.users
  for each row execute function public.handle_new_user();

-- Trigger: create the 1:1 conversation when a friendship flips to accepted
create or replace function public.handle_friendship_accepted()
returns trigger
language plpgsql
security definer
set search_path = ''
as $$
begin
  if new.status = 'accepted' and old.status is distinct from 'accepted' then
    insert into public.conversations (user1_id, user2_id)
    values (least(new.requester_id, new.addressee_id), greatest(new.requester_id, new.addressee_id))
    on conflict (user1_id, user2_id) do nothing;
  end if;
  return new;
end;
$$;

create trigger on_friendship_accepted
  after update on public.friendships
  for each row execute function public.handle_friendship_accepted();

-- Trigger: keep friendships.updated_at fresh
create or replace function public.handle_updated_at()
returns trigger
language plpgsql
set search_path = ''
as $$
begin
  new.updated_at := now();
  return new;
end;
$$;

create trigger friendships_updated_at
  before update on public.friendships
  for each row execute function public.handle_updated_at();

-- Trigger: bump conversation.last_message_at on new message
create or replace function public.handle_new_message()
returns trigger
language plpgsql
security definer
set search_path = ''
as $$
begin
  update public.conversations
  set last_message_at = new.created_at
  where id = new.conversation_id;
  return new;
end;
$$;

create trigger on_message_created
  after insert on public.messages
  for each row execute function public.handle_new_message();
