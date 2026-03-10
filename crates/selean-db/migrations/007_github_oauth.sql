-- GitHub OAuth: add GitHub identity and token columns to users table.

ALTER TABLE users ADD COLUMN github_id BIGINT UNIQUE;
ALTER TABLE users ADD COLUMN github_login TEXT;
ALTER TABLE users ADD COLUMN github_token TEXT;
