-- Takeover replaces one session with another for the same user. Realtime DELETE
-- consumers need the old session_id to distinguish that stale row from the
-- newly inserted replacement row.
alter table public.voice_participants replica identity full;