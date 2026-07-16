-- Allow the Cloudinary-backed decoration catalogue while preserving profiles
-- that still reference a legacy local decoration.
alter table public.profiles
  drop constraint if exists profiles_avatar_decoration_check;

alter table public.profiles
  add constraint profiles_avatar_decoration_check
  check (
    avatar_decoration is null
    or avatar_decoration ~ '^\d{3}$'
    or avatar_decoration = any (array[
      'bubble', 'chun-li', 'dreamy', 'heart', 'helly-kitty', 'leafs', 'lights', 'milk',
      'moon', 'rainbow', 'rawr', 'sparkly', 'sparkly-pink', 'spider-man', 'toy-story',
      'venom', 'arcane-anomaly', 'arcane-boom', 'arcane-jynx', 'arcane-powder'
    ])
  );