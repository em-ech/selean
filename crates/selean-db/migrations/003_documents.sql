-- Documents: design files owned by a workspace.
CREATE TABLE IF NOT EXISTS documents (
    id            UUID PRIMARY KEY,
    workspace_id  UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,
    created_by    UUID NOT NULL REFERENCES users(id),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at    TIMESTAMPTZ  -- soft delete
);

CREATE INDEX idx_documents_workspace ON documents (workspace_id);
CREATE INDEX idx_documents_created_by ON documents (created_by);
CREATE INDEX idx_documents_deleted ON documents (deleted_at) WHERE deleted_at IS NOT NULL;

-- Document versions: immutable snapshots of document state.
CREATE TABLE IF NOT EXISTS document_versions (
    id            UUID PRIMARY KEY,
    document_id   UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    version       INT NOT NULL,
    data          JSONB NOT NULL,
    created_by    UUID NOT NULL REFERENCES users(id),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (document_id, version)
);

CREATE INDEX idx_document_versions_doc ON document_versions (document_id, version DESC);
