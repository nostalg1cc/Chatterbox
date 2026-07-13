-- GIF avatars retain an animated original plus a static WebP cover frame.

alter table public.profiles
  add column avatar_animated_path text;

alter table public.profiles
  add constraint profiles_avatar_animated_path_owned
  check (
    avatar_animated_path is null
    or avatar_animated_path = id::text || '/avatar.gif'
  );

update storage.buckets
set allowed_mime_types = array['image/webp', 'image/gif']
where id = 'avatars';

drop policy if exists "avatar_owner_read" on storage.objects;
create policy "avatar_owner_read"
on storage.objects for select
to authenticated
using (
  bucket_id = 'avatars'
  and name in (
    (select auth.uid())::text || '/avatar.webp',
    (select auth.uid())::text || '/avatar.gif'
  )
);

drop policy if exists "avatar_owner_insert" on storage.objects;
create policy "avatar_owner_insert"
on storage.objects for insert
to authenticated
with check (
  bucket_id = 'avatars'
  and name in (
    (select auth.uid())::text || '/avatar.webp',
    (select auth.uid())::text || '/avatar.gif'
  )
);

drop policy if exists "avatar_owner_update" on storage.objects;
create policy "avatar_owner_update"
on storage.objects for update
to authenticated
using (
  bucket_id = 'avatars'
  and name in (
    (select auth.uid())::text || '/avatar.webp',
    (select auth.uid())::text || '/avatar.gif'
  )
)
with check (
  bucket_id = 'avatars'
  and name in (
    (select auth.uid())::text || '/avatar.webp',
    (select auth.uid())::text || '/avatar.gif'
  )
);

drop policy if exists "avatar_owner_delete" on storage.objects;
create policy "avatar_owner_delete"
on storage.objects for delete
to authenticated
using (
  bucket_id = 'avatars'
  and name in (
    (select auth.uid())::text || '/avatar.webp',
    (select auth.uid())::text || '/avatar.gif'
  )
);