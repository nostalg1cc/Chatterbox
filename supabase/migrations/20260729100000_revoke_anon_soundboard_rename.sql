-- Explicitly revoke the inherited anonymous execute privilege. PostgreSQL
-- function privileges can remain granted through role inheritance even after a
-- PUBLIC revoke in an older migration.
revoke execute on function public.rename_soundboard_sound(uuid, text) from anon;
revoke execute on function public.rename_soundboard_sound(uuid, text) from public;
grant execute on function public.rename_soundboard_sound(uuid, text) to authenticated;
