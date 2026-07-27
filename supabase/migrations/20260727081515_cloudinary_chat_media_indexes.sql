-- Keep cleanup and cascading deletes efficient for temporary Cloudinary reservations.
create index if not exists cloud_chat_media_reservations_conversation_idx
  on private.cloud_chat_media_reservations (conversation_id);
create index if not exists cloud_chat_media_reservations_user_idx
  on private.cloud_chat_media_reservations (user_id);