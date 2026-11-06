-- Monthly budget and leases for guarded screen sharing (functions/screen-share).
-- Additive: cloudflare-realtime and its session table keep working unchanged.

create table private.screen_share_budget (
  month date primary key,
  -- An application limit of 500 GB of Cloudflare egress. Set it to 0 to stop new shares.
  limit_bytes bigint not null default 500000000000 check (limit_bytes >= 0),
  reserved_bytes bigint not null default 0 check (reserved_bytes >= 0)
);

create table private.screen_share_leases (
  id uuid primary key default gen_random_uuid(),
  kind text not null check (kind in ('publish', 'view')),
  month date not null references private.screen_share_budget (month),
  conversation_id uuid not null,
  user_id uuid not null,
  voice_session_id uuid not null,
  session_id text,
  remote_session_id text,
  mids text[] not null default '{}',
  reserved_bytes bigint not null default 0 check (reserved_bytes >= 0),
  created_at timestamptz not null default now(),
  expires_at timestamptz not null,
  hard_expires_at timestamptz not null,
  closed_at timestamptz,
  close_failures integer not null default 0
);
create unique index screen_share_one_publisher
  on private.screen_share_leases (voice_session_id) where kind = 'publish' and closed_at is null;
create unique index screen_share_one_viewer
  on private.screen_share_leases (remote_session_id) where kind = 'view' and closed_at is null;
create index screen_share_open_leases
  on private.screen_share_leases (expires_at) where closed_at is null;

alter table private.screen_share_budget enable row level security;
alter table private.screen_share_leases enable row level security;
revoke all on private.screen_share_budget, private.screen_share_leases from public, anon, authenticated;
grant select, insert, update on private.screen_share_budget, private.screen_share_leases to service_role;
create policy screen_share_budget_service on private.screen_share_budget for all to service_role using (true) with check (true);
create policy screen_share_leases_service on private.screen_share_leases for all to service_role using (true) with check (true);

-- Reserves a whole view before Cloudflare is asked for anything. 2.628 Mbit/s for 10 minutes
-- with 25% headroom is 246,375,000 bytes. Publishing is ingress, which Cloudflare doesn't
-- bill, so a publish lease reserves nothing. The budget row's lock serializes admissions,
-- so concurrent requests can't overspend.
create or replace function public.screen_share_admit(
  p_kind text, p_conversation_id uuid, p_user_id uuid, p_voice_session_id uuid, p_remote_session_id text
) returns jsonb
language plpgsql security invoker set search_path = ''
as $$
declare
  v_month date := date_trunc('month', now() at time zone 'utc')::date;
  v_budget private.screen_share_budget;
  v_amount bigint := case when p_kind = 'view' then 246375000 else 0 end;
  v_hard timestamptz := now() + interval '10 minutes';
  v_share_ends timestamptz;
  v_id uuid;
begin
  insert into private.screen_share_budget (month) values (v_month) on conflict (month) do nothing;
  select * into v_budget from private.screen_share_budget where month = v_month for update;

  -- Tracks that repeatedly won't close might still be forwarding media.
  if exists (
    select 1 from private.screen_share_leases
    where closed_at is null and close_failures >= 3
  ) then
    return jsonb_build_object('ok', false, 'reason', 'cleanup_pending');
  end if;

  -- A stopped limit also stops publishing, not only the viewing that costs money.
  if v_budget.reserved_bytes + greatest(v_amount, 1) > v_budget.limit_bytes then
    return jsonb_build_object('ok', false, 'reason', 'budget');
  end if;

  if p_kind = 'view' then
    -- A publish lease, when there is one, caps the view at what's left of the share.
    -- Shares from main Nitro have none, so they get the plain 10 minutes.
    select l.hard_expires_at into v_share_ends
    from private.screen_share_leases l
    where l.session_id = p_remote_session_id and l.kind = 'publish' and l.closed_at is null;
    -- least() ignores a null, which is the no-lease case.
    v_hard := least(v_hard, v_share_ends);
    if v_hard <= now() then
      return jsonb_build_object('ok', false, 'reason', 'not_available');
    end if;
  end if;

  begin
    insert into private.screen_share_leases (
      kind, month, conversation_id, user_id, voice_session_id, remote_session_id,
      reserved_bytes, expires_at, hard_expires_at
    ) values (
      p_kind, v_month, p_conversation_id, p_user_id, p_voice_session_id, p_remote_session_id,
      v_amount, least(v_hard, now() + interval '45 seconds'), v_hard
    ) returning id into v_id;
  exception when unique_violation then
    return jsonb_build_object('ok', false, 'reason',
      case when p_kind = 'view' then 'one_viewer' else 'already_sharing' end);
  end;

  update private.screen_share_budget set reserved_bytes = reserved_bytes + v_amount where month = v_month;
  return jsonb_build_object('ok', true, 'id', v_id,
    'seconds_left', floor(extract(epoch from v_hard - now()))::integer);
end;
$$;

create or replace function public.screen_share_attach(p_lease_id uuid, p_session_id text, p_mids text[])
returns void
language sql security invoker set search_path = ''
as $$
  update private.screen_share_leases set session_id = p_session_id, mids = p_mids
  where id = p_lease_id and closed_at is null;
$$;

-- Never reaches past the hard limit, and a lease already past its time can't be revived.
create or replace function public.screen_share_renew(p_lease_id uuid, p_user_id uuid)
returns boolean
language plpgsql security invoker set search_path = ''
as $$
begin
  update private.screen_share_leases
  set expires_at = least(hard_expires_at, now() + interval '45 seconds')
  where id = p_lease_id and user_id = p_user_id and closed_at is null
    and expires_at > now() and hard_expires_at > now();
  return found;
end;
$$;

create or replace function public.screen_share_owned(p_lease_id uuid, p_user_id uuid)
returns jsonb
language sql stable security invoker set search_path = ''
as $$
  select to_jsonb(l) from private.screen_share_leases l
  where l.id = p_lease_id and l.user_id = p_user_id and l.closed_at is null;
$$;

-- Makes a lease due at once, leaving its reservation in place until tracks are closed.
create or replace function public.screen_share_expire(p_lease_id uuid)
returns void
language sql security invoker set search_path = ''
as $$
  update private.screen_share_leases set expires_at = now(), hard_expires_at = now()
  where id = p_lease_id and closed_at is null;
$$;

create or replace function public.screen_share_due(p_limit integer)
returns setof jsonb
language sql stable security invoker set search_path = ''
as $$
  select to_jsonb(l) from private.screen_share_leases l
  where l.closed_at is null and l.expires_at <= now()
  order by l.expires_at limit p_limit;
$$;

-- Keeps only what the time actually used could have cost, and returns the rest.
create or replace function public.screen_share_finish(p_lease_id uuid)
returns void
language plpgsql security invoker set search_path = ''
as $$
declare
  v_lease private.screen_share_leases;
  v_used bigint;
begin
  select * into v_lease from private.screen_share_leases where id = p_lease_id for update;
  if not found or v_lease.closed_at is not null then return; end if;
  v_used := least(v_lease.reserved_bytes,
    ceil(v_lease.reserved_bytes * extract(epoch from now() - v_lease.created_at) / 600)::bigint);
  update private.screen_share_leases set closed_at = now(), reserved_bytes = v_used where id = p_lease_id;
  update private.screen_share_budget
  set reserved_bytes = greatest(0, reserved_bytes - (v_lease.reserved_bytes - v_used))
  where month = v_lease.month;
end;
$$;

create or replace function public.screen_share_note_failure(p_lease_id uuid)
returns void
language sql security invoker set search_path = ''
as $$
  update private.screen_share_leases
  set close_failures = close_failures + 1, expires_at = least(expires_at, now())
  where id = p_lease_id and closed_at is null;
$$;

do $$
declare
  v_signature text;
begin
  foreach v_signature in array array[
    'public.screen_share_admit(text, uuid, uuid, uuid, text)',
    'public.screen_share_attach(uuid, text, text[])',
    'public.screen_share_renew(uuid, uuid)',
    'public.screen_share_owned(uuid, uuid)',
    'public.screen_share_expire(uuid)',
    'public.screen_share_due(integer)',
    'public.screen_share_finish(uuid)',
    'public.screen_share_note_failure(uuid)'
  ] loop
    execute format('revoke all on function %s from public, anon, authenticated', v_signature);
    execute format('grant execute on function %s to service_role', v_signature);
  end loop;
end;
$$;

-- Checked every 15 seconds in SQL, and only calls the function when something is due, so an
-- idle month costs no invocations.
select cron.schedule('reap-screen-shares', '15 seconds', $job$
  select net.http_post(
    url := (select decrypted_secret from vault.decrypted_secrets where name = 'project_url') || '/functions/v1/screen-share',
    headers := jsonb_build_object('Content-Type', 'application/json',
      'apikey', (select decrypted_secret from vault.decrypted_secrets where name = 'publishable_key'),
      'Authorization', 'Bearer ' || (select decrypted_secret from vault.decrypted_secrets where name = 'legacy_anon_key'),
      'x-cleanup-key', (select decrypted_secret from vault.decrypted_secrets where name = 'chat_media_cleanup_key')),
    body := '{}'::jsonb, timeout_milliseconds := 10000)
  where exists (
    select 1 from private.screen_share_leases where closed_at is null and expires_at <= now()
  );
$job$);
