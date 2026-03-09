-- Assets: uploaded files (images, fonts, etc.) stored externally.
CREATE TABLE IF NOT EXISTS assets (
    id            UUID PRIMARY KEY,
    workspace_id  UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    filename      TEXT NOT NULL,
    content_type  TEXT NOT NULL,
    size_bytes    BIGINT NOT NULL,
    storage_key   TEXT NOT NULL,
    uploaded_by   UUID NOT NULL REFERENCES users(id),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_assets_workspace ON assets (workspace_id);
