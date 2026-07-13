-- Live profile changes for avatars, display names, and constrained chat name colors.

alter table public.profiles
  add column name_color text not null default 'default';

alter table public.profiles
  add constraint profiles_name_color_valid
  check (
    name_color in (
      'default',
      'slate',
      'red',
      'orange',
      'amber',
      'green',
      'cyan',
      'blue',
      'violet',
      'pink'
    )
  );

do $$
begin
  if not exists (
    select 1
    from pg_catalog.pg_publication_tables
    where pubname = 'supabase_realtime'
      and schemaname = 'public'
      and tablename = 'profiles'
  ) then
    alter publication supabase_realtime add table public.profiles;
  end if;
end;
$$;
