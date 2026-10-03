-- Only an undeleted message can authorize a new legacy attachment download.
create index if not exists messages_active_media_path_idx on public.messages(media_path)
where media_path is not null and deleted_at is null and media_deleted_at is null;
alter policy chat_media_participant_read on storage.objects
using (
  bucket_id = 'chat-media'
  and private.is_participant(private.media_conversation_id(name))
  and exists (select 1 from public.messages m where m.media_path = name
    and m.deleted_at is null and m.media_deleted_at is null)
);
