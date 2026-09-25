ALTER TABLE workspaces ADD COLUMN name text NOT NULL DEFAULT 'World Wide Webb'
    CHECK (length(btrim(name)) BETWEEN 1 AND 120);
ALTER TABLE workspaces ADD COLUMN icon text
    CHECK (icon IS NULL OR length(icon) BETWEEN 1 AND 16);
ALTER TABLE workspaces ADD COLUMN color text
    CHECK (color IS NULL OR color ~ '^#[0-9A-Fa-f]{6}$');
CREATE TABLE selected_workspace (
    owner_id text PRIMARY KEY CHECK (owner_id = 'owner'),
    workspace_id text NOT NULL REFERENCES workspaces
);
INSERT INTO selected_workspace VALUES ('owner', 'local');
CREATE TABLE workspace_receipts (
    operation_id text PRIMARY KEY,
    command jsonb NOT NULL,
    result_id text NOT NULL
);
ALTER TABLE command_receipts ADD COLUMN workspace_id text NOT NULL DEFAULT 'local' REFERENCES workspaces;
