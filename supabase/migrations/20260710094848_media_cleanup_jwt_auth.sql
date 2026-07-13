-- Keep platform JWT verification enabled for the cleanup Edge Function.
-- The legacy anon JWT is stored in Vault during deployment and is never committed.
select cron.schedule(
  'purge-chat-media-hourly',
  '17 * * * *',
  $job$
    select net.http_post(
      url := (select decrypted_secret from vault.decrypted_secrets where name = 'project_url')
        || '/functions/v1/purge-chat-media',
      headers := jsonb_build_object(
        'Content-Type', 'application/json',
        'apikey', (select decrypted_secret from vault.decrypted_secrets where name = 'legacy_anon_key'),
        'Authorization', 'Bearer ' ||
          (select decrypted_secret from vault.decrypted_secrets where name = 'legacy_anon_key')
      ),
      body := jsonb_build_object('mode', 'scheduled'),
      timeout_milliseconds := 10000
    ) as request_id;
  $job$
);
