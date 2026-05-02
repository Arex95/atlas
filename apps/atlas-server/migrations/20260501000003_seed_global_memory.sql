-- Seed essential global memory entries available to all agents from first run.
-- INSERT OR IGNORE so user edits are never overwritten.

INSERT OR IGNORE INTO global_memory (key, value) VALUES
  ('ATLAS_SERVER_URL',    'http://localhost:4000'),
  ('ATLAS_DASHBOARD_URL', 'http://localhost:4000'),
  ('ATLAS_VERSION',       '0.1.0');
