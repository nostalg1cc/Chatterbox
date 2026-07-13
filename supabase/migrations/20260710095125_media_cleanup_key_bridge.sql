-- The platform verifier still expects a JWT in Authorization, while the
-- current Edge Function SDK validates modern publishable keys from apikey.
select cron.schedule(
  'purge-chat-media-hourly',
  '17 * * * *',
  $job$
    select net.http_post(
      url := (select decrypted_secret from vault.decrypted_secrets where name = 'project_url')
        || '/functions/v1/purge-chat-media',
      headers := jsonb_build_object(
        'Content-Type', 'application/json',
        'apikey', (select decrypted_secret from vault.decrypted_secrets where name = 'publishable_key'),
        'Authorization', 'Bearer ' ||
          (select decrypted_secret from vault.decrypted_secrets where name = 'legacy_anon_key')
      ),
      body := jsonb_build_object('mode', 'scheduled'),
      timeout_milliseconds := 10000
    ) as request_id;
  $job$
);
