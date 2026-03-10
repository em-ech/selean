-- Add billing tier to workspaces (default free for existing rows).
ALTER TABLE workspaces ADD COLUMN billing_tier TEXT NOT NULL DEFAULT 'free'
  CHECK (billing_tier IN ('free', 'pro', 'team', 'enterprise'));
