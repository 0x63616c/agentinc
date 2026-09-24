CREATE TABLE automations (
    id text PRIMARY KEY,
    workspace_id text NOT NULL REFERENCES workspaces,
    name text NOT NULL CHECK (length(btrim(name)) BETWEEN 1 AND 120),
    prompt text NOT NULL CHECK (length(btrim(prompt)) BETWEEN 1 AND 500),
    agent_id text NOT NULL,
    every_minutes bigint NOT NULL DEFAULT 30 CHECK (every_minutes BETWEEN 1 AND 525600),
    paused boolean NOT NULL DEFAULT false,
    revision bigint NOT NULL DEFAULT 0,
    applied_revision bigint NOT NULL DEFAULT -1,
    error text,
    missed bigint NOT NULL DEFAULT 0,
    overlap_skipped bigint NOT NULL DEFAULT 0,
    FOREIGN KEY (workspace_id,agent_id) REFERENCES agents(workspace_id,id)
);
CREATE TABLE automation_receipts (
    workspace_id text NOT NULL REFERENCES workspaces,
    operation_id text NOT NULL,
    command jsonb NOT NULL,
    result_id text NOT NULL,
    PRIMARY KEY(workspace_id,operation_id)
);
CREATE TABLE occurrences (
    id text PRIMARY KEY,
    automation_id text NOT NULL REFERENCES automations,
    revision bigint NOT NULL,
    manual boolean NOT NULL DEFAULT false,
    dispatched boolean NOT NULL DEFAULT false,
    ticket_id bigint REFERENCES tickets,
    state text NOT NULL DEFAULT 'waiting_for_worker',
    detail text,
    scheduled_at bigint NOT NULL DEFAULT extract(epoch FROM now())::bigint,
    UNIQUE(ticket_id)
);
CREATE TABLE automation_history (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    automation_id text NOT NULL REFERENCES automations,
    kind text NOT NULL,
    count bigint NOT NULL,
    observed_at bigint NOT NULL DEFAULT extract(epoch FROM now())::bigint
);
CREATE TABLE worker_health (
    id text PRIMARY KEY,
    last_seen bigint NOT NULL
);
