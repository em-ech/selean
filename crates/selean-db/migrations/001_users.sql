-- Users table: core identity for all authenticated users.
CREATE TABLE IF NOT EXISTS users (
    id          UUID PRIMARY KEY,
    email       TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL DEFAULT '',
    password_hash TEXT,  -- NULL for OAuth-only accounts
    avatar_url  TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_users_email ON users (email);
