ALTER TABLE ai_sessions ADD COLUMN is_saved INTEGER DEFAULT 0; 
ALTER TABLE ai_sessions ADD COLUMN custom_name TEXT;
ALTER TABLE ai_sessions ADD COLUMN custom_description TEXT;
ALTER TABLE ai_sessions ADD COLUMN color TEXT;
ALTER TABLE ai_sessions ADD COLUMN icon TEXT;
