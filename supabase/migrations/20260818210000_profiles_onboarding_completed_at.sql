-- Onboarding flow completion flag (mirror of migration applied to project lapjrxdgcbdseskmyfru via Supabase MCP)
-- Nullable and account-level (not a local preference) so it follows the user
-- across devices and defaults to "not done" for everyone, including
-- existing accounts predating this feature - there's no backfill here on
-- purpose. profiles_update_own (see 20260709164800_rls_policies.sql) already
-- covers this column with no changes needed.

alter table public.profiles
  add column onboarding_completed_at timestamptz;
