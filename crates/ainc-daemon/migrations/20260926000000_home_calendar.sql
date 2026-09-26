-- Durable actions: a committed row is the acknowledgement; a durable task named
-- after the row applies its idempotent effect and records the outcome.
CREATE TABLE durable_actions (
    id text PRIMARY KEY,
    seq bigint GENERATED ALWAYS AS IDENTITY UNIQUE,
    workspace_id text NOT NULL REFERENCES workspaces,
    actor_id text NOT NULL,
    kind text NOT NULL CHECK (kind IN ('home','calendar_import')),
    input jsonb NOT NULL,
    summary text NOT NULL,
    digest text,
    state text NOT NULL DEFAULT 'queued'
        CHECK (state IN ('queued','running','completed','failed','superseded')),
    error text,
    dispatched boolean NOT NULL DEFAULT false,
    created_at bigint NOT NULL DEFAULT extract(epoch FROM now())::bigint,
    finished_at bigint
);
CREATE INDEX durable_actions_undispatched ON durable_actions(seq) WHERE NOT dispatched;
CREATE INDEX durable_actions_recent ON durable_actions(workspace_id, kind, seq DESC);

-- The control center endpoint. Its Access token lives in the Keychain; this row
-- only records whether one is stored.
CREATE TABLE home_connections (
    workspace_id text PRIMARY KEY REFERENCES workspaces,
    base_url text NOT NULL CHECK (base_url ~ '^https?://\S+$' AND length(base_url) <= 500),
    access_token boolean NOT NULL DEFAULT false
);
CREATE TABLE home_receipts (
    workspace_id text NOT NULL REFERENCES workspaces,
    actor_id text NOT NULL,
    operation_id text NOT NULL,
    command jsonb NOT NULL,
    result_id text NOT NULL,
    PRIMARY KEY (workspace_id, actor_id, operation_id)
);

CREATE TABLE calendar_events (
    id text PRIMARY KEY,
    workspace_id text NOT NULL REFERENCES workspaces,
    user_id text NOT NULL,
    source text NOT NULL CHECK (source IN ('agentinc','macos')),
    external_id text,
    calendar text NOT NULL CHECK (length(btrim(calendar)) BETWEEN 1 AND 120),
    color text CHECK (color IS NULL OR color ~ '^#[0-9A-Fa-f]{6}$'),
    title text NOT NULL CHECK (length(btrim(title)) BETWEEN 1 AND 500),
    location text CHECK (location IS NULL OR length(location) <= 500),
    notes text CHECK (notes IS NULL OR length(notes) <= 8000),
    starts_at bigint NOT NULL,
    ends_at bigint NOT NULL,
    all_day boolean NOT NULL DEFAULT false,
    revision bigint NOT NULL DEFAULT 0,
    CHECK (ends_at >= starts_at),
    CHECK ((source = 'agentinc') = (external_id IS NULL)),
    UNIQUE (workspace_id, user_id, source, external_id)
);
CREATE INDEX calendar_events_range ON calendar_events(workspace_id, user_id, starts_at);
-- The newest import applied per user, so an older import that runs late
-- never overwrites a newer one.
CREATE TABLE calendar_import_heads (
    workspace_id text NOT NULL REFERENCES workspaces,
    user_id text NOT NULL,
    applied_seq bigint NOT NULL,
    PRIMARY KEY (workspace_id, user_id)
);
CREATE TABLE calendar_receipts (
    workspace_id text NOT NULL REFERENCES workspaces,
    actor_id text NOT NULL,
    operation_id text NOT NULL,
    command jsonb NOT NULL,
    result_id text NOT NULL,
    PRIMARY KEY (workspace_id, actor_id, operation_id)
);
