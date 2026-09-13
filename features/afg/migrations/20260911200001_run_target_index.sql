-- Finding the node a session is executing, on every MCP call it makes.
--
-- Tool scoping asks "is this session mid-node, and what did that node
-- allow?" before dispatching any tool. Without an index that is a scan
-- of every run ever started, paid per call — the kind of cost that
-- makes a useful check get removed later for being slow rather than
-- for being wrong.
--
-- Partial, on `running` only: a finished run scopes nothing, and
-- leaving completed and failed rows out keeps the index the size of
-- the work in flight rather than the size of the history.
CREATE INDEX IF NOT EXISTS idx_workflow_runs_target_session
    ON workflow_runs (target_session_id)
    WHERE status = 'running';
