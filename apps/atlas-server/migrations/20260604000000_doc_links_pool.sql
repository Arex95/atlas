-- Add graph links to project and session documents.
-- links is a JSON array of document IDs: '["id1","id2"]'
ALTER TABLE project_documents ADD COLUMN links TEXT NOT NULL DEFAULT '[]';
ALTER TABLE session_documents  ADD COLUMN links TEXT NOT NULL DEFAULT '[]';
