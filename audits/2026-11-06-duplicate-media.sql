-- Duplicate chat media clean-up. Review, then run it yourself in the Supabase SQL editor.
--
-- Evidence (2026-11-06): every message below has a Cloudinary asset whose etag (content
-- hash) matches the copy being kept, with the same size. None has a caption, reaction or
-- reply pointing at it.
--
--   Keep 6410bd63-c883-4365-adfc-986a39cfc4ba  nyxnie, image, 2.56 MB, removes 19 copies
--   Keep 57edfea5-ef70-4ea4-a3f8-5901a5be8336  nrohde, video, 4.82 MB, removes 1 copy
--   Frees about 53.4 MB of Cloudinary storage.
--
-- This uses Nitro's normal delete: it sets deleted_at. The hourly purge-chat-media job
-- (minute 17) then permanently destroys those Cloudinary assets. Until that job runs, the
-- UNDO statement at the bottom restores everything. After it runs, the copies are gone,
-- though the kept originals remain.

begin;

update public.messages
set deleted_at = now()
where deleted_at is null
  and media_deleted_at is null
  and (content is null or content = '')
  and not exists (select 1 from public.reactions r where r.message_id = messages.id)
  and not exists (select 1 from public.messages m where m.reply_to_message_id = messages.id)
  and id in (
    -- nyxnie, 2026-10-25, copies of 6410bd63
    '4f4d2f70-9c38-4137-961f-838a0a07e3c5',
    '5444badc-23e0-4756-9001-8d5826a0a3dc',
    '9acc1b19-25ec-4a4a-848e-6b1b4975e682',
    '241abab3-af61-4656-87a6-608149fce606',
    'b75b30bd-3743-4655-8656-538b015fddd0',
    '0f641af4-017e-45be-aefe-9338dcde2e60',
    '5684c672-a997-4363-963a-0a8a90aee24d',
    '9b0fd7f5-c578-4d07-bf34-abae799143ca',
    '2ba27df2-d619-45ea-858c-c10381d32761',
    'aa2b91a9-8ea3-49cf-8744-b37ffdcfc448',
    '9b7500e4-b42a-490e-a7f4-bf9bb974e578',
    '65b37b5e-59c2-4ece-9e19-00aa02f2befb',
    '4fb3f371-2410-491f-89fc-95cb6146479a',
    '4e2b04e0-c807-44e0-9692-8026f743cc16',
    '5cb183d5-9eed-4f53-b36e-c2c4e952a6e2',
    'eaddc256-e74d-4a15-91f7-b8e756a37852',
    '62f15bfd-1747-4e57-b866-9de5a9fb3b39',
    '77485e7e-6053-4dbf-82b9-80a0e1ad25c4',
    'fd3412e8-5001-4d0d-96c6-ae287839342b',
    -- nrohde, 2026-08-16, copy of 57edfea5
    'bab8721b-321c-4f77-a5d9-b9a9539dc49e'
  );

-- Expect exactly 20 rows updated. If it isn't 20, run ROLLBACK instead of COMMIT.
commit;

-- UNDO, only before the next run of the hourly job (minute 17):
-- update public.messages set deleted_at = null
-- where media_deleted_at is null and id in ( ...the same 20 ids... );
