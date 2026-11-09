-- Switches the app reads at runtime, set through the push-notice function, so a small change of
-- behaviour doesn't need a release.
create table public.app_flags (
  name text primary key check (char_length(name) between 1 and 64),
  enabled boolean not null default false,
  updated_at timestamptz not null default now()
);

alter table public.app_flags enable row level security;

create policy "app_flags_select" on public.app_flags
  for select to authenticated
  using (true);

insert into public.app_flags (name, enabled) values ('snow', false);
