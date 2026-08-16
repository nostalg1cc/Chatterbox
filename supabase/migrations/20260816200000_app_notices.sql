-- A lightweight way to push a message into every signed-in, live app
-- instance without shipping a release. Read-only for clients (delivered via
-- realtime INSERT + a one-time fetch of the latest row on load); only ever
-- written by the push-notice Edge Function (service role) - no insert/
-- update/delete policy exists for authenticated/anon at all.

create table public.app_notices (
  id uuid primary key default gen_random_uuid(),
  severity text not null default 'neutral' check (severity in ('neutral', 'warning', 'danger')),
  message text not null check (char_length(message) between 1 and 500),
  created_at timestamptz not null default now()
);

alter table public.app_notices enable row level security;

create policy "app_notices_select" on public.app_notices
  for select to authenticated
  using (true);

alter publication supabase_realtime add table public.app_notices;
