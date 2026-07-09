-- Realtime: postgres_changes for chat tables (RLS-aware via WALRUS)

alter publication supabase_realtime add table
  public.messages,
  public.reactions,
  public.friendships,
  public.conversations;

-- Full replica identity so DELETE events carry the old row
-- (required for RLS-authorized delete notifications on reactions/friendships)
alter table public.reactions replica identity full;
alter table public.friendships replica identity full;
