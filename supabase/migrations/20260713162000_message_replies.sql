alter table public.messages
  add column if not exists reply_to_message_id uuid
  references public.messages(id) on delete set null;

create index if not exists messages_reply_to_message_idx
  on public.messages (reply_to_message_id)
  where reply_to_message_id is not null;

create or replace function private.validate_message_reply()
returns trigger
language plpgsql
set search_path = pg_catalog, public
as $$
begin
  if new.reply_to_message_id is null then
    return new;
  end if;

  if new.reply_to_message_id = new.id then
    raise exception 'A message cannot reply to itself';
  end if;

  if not exists (
    select 1
    from public.messages parent
    where parent.id = new.reply_to_message_id
      and parent.conversation_id = new.conversation_id
  ) then
    raise exception 'Replies must reference a message in the same conversation';
  end if;

  return new;
end;
$$;

revoke all on function private.validate_message_reply() from public, anon, authenticated;

drop trigger if exists validate_message_reply on public.messages;
create trigger validate_message_reply
  before insert or update of reply_to_message_id, conversation_id
  on public.messages
  for each row
  execute function private.validate_message_reply();