-- Persistent, constrained username typography presets. Profile updates remain covered by the existing owner-only RLS policy.

alter table public.profiles
  add column if not exists name_font text not null default 'sans',
  add column if not exists name_weight text not null default 'medium';

alter table public.profiles
  drop constraint if exists profiles_name_font_valid,
  drop constraint if exists profiles_name_weight_valid,
  drop constraint if exists profiles_name_decoration_valid;

alter table public.profiles
  add constraint profiles_name_font_valid
    check (name_font in ('sans', 'rounded', 'serif', 'mono')),
  add constraint profiles_name_weight_valid
    check (name_weight in ('regular', 'medium', 'bold', 'black')),
  add constraint profiles_name_decoration_valid
    check (name_decoration is null or name_decoration in (
      'fuzzy', 'sparkles', 'resize', 'bouncy', 'wavy', 'gradient', 'glitch', 'particle'
    ));