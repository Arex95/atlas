-- Account disablement: a disabled account loses access through
-- every path — password login, GitLab OAuth login, and any session
-- token issued before it was disabled. NULL means enabled; the
-- timestamp records when it happened. Checked by joining this
-- column wherever a user authenticates, rather than deleting
-- session_tokens rows — a disabled row simply stops matching.
ALTER TABLE users ADD COLUMN disabled_at TEXT;
