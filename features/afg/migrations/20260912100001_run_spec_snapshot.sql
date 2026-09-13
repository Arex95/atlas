-- A run pins the spec it started with.
--
-- Until now a run resolved its nodes from the `workflows` row, which
-- was fine only because that row never changed after registration.
-- It changes now: starting a run re-reads the workflow's source file,
-- so the repository is the source of truth rather than a copy taken
-- once. Without a snapshot, editing the file would rewrite the rules
-- of every run already in flight — an agent would be judged against
-- criteria that arrived after it was dispatched.
--
-- Nullable only so existing rows can be backfilled below; every run
-- created from here on writes it.
ALTER TABLE workflow_runs ADD COLUMN spec_json TEXT;

UPDATE workflow_runs
SET spec_json = (
    SELECT spec_json FROM workflows WHERE workflows.id = workflow_runs.workflow_id
)
WHERE spec_json IS NULL;
