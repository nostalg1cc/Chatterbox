create or replace function public.rename_soundboard_sound(
  p_sound_id uuid,
  p_name text
)
returns public.soundboard_sounds
language plpgsql
security definer
set search_path = ''
as $$
declare
  renamed public.soundboard_sounds;
  clean_name text := left(trim(p_name), 32);
begin
  if auth.uid() is null then
    raise exception 'Authentication required';
  end if;
  if clean_name is null or clean_name = '' then
    raise exception 'Give the sound a name.';
  end if;

  update public.soundboard_sounds
  set name = clean_name
  where id = p_sound_id and owner_id = auth.uid()
  returning * into renamed;

  if not found then
    raise exception 'Sound not found';
  end if;
  return renamed;
end;
$$;

revoke all on function public.rename_soundboard_sound(uuid, text) from public;
grant execute on function public.rename_soundboard_sound(uuid, text) to authenticated;
