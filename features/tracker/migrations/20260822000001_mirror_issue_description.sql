-- Plan progress (checkbox counting) needs the issue body, which
-- the mirror did not previously store. Default '' distinguishes
-- "not yet backfilled by a resync" from a genuinely empty
-- description identically for the parser's purposes — both parse
-- to zero acceptance criteria.
ALTER TABLE mirror_issues ADD COLUMN description TEXT NOT NULL DEFAULT '';
