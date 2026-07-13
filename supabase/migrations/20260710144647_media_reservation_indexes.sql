-- Cover reservation foreign keys used by cascade cleanup and quota checks.

create index chat_media_reservations_conversation_idx
  on private.chat_media_reservations (conversation_id);

create index chat_media_reservations_user_idx
  on private.chat_media_reservations (user_id);
