-- Preserve IDs and completion state from both phase-2 Postgres and legacy imports.
ALTER TABLE todos RENAME TO tickets;
ALTER TABLE tickets ADD COLUMN status text NOT NULL DEFAULT 'to_do'
    CHECK (status IN ('backlog','to_do','in_progress','done'));
UPDATE tickets SET status='done' WHERE completed;
ALTER TABLE tickets DROP COLUMN completed;
ALTER TABLE tickets ADD COLUMN revision bigint NOT NULL DEFAULT 0;
ALTER TABLE tickets ADD COLUMN generation bigint NOT NULL DEFAULT 0;

CREATE TABLE principals (
    workspace_id text NOT NULL REFERENCES workspaces,
    id text NOT NULL,
    kind text NOT NULL CHECK (kind IN ('human','agent')),
    name text NOT NULL CHECK (length(btrim(name)) BETWEEN 1 AND 120),
    PRIMARY KEY (workspace_id,id),
    UNIQUE (workspace_id,kind,id)
);
INSERT INTO principals SELECT id,'owner','human','You' FROM workspaces;
ALTER TABLE tickets ADD COLUMN assignee_kind text NOT NULL DEFAULT 'human';
ALTER TABLE tickets ADD COLUMN assignee_id text NOT NULL DEFAULT 'owner';
ALTER TABLE tickets ADD CONSTRAINT ticket_assignee FOREIGN KEY (workspace_id,assignee_kind,assignee_id)
    REFERENCES principals(workspace_id,kind,id);
CREATE TABLE agents (
    workspace_id text NOT NULL,
    id text NOT NULL,
    instructions text NOT NULL,
    model text NOT NULL CHECK (length(btrim(model)) > 0),
    PRIMARY KEY (workspace_id,id),
    FOREIGN KEY (workspace_id,id) REFERENCES principals(workspace_id,id)
);
CREATE TABLE comments (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    ticket_id bigint NOT NULL REFERENCES tickets ON DELETE CASCADE,
    author_id text NOT NULL,
    body text NOT NULL CHECK (length(btrim(body)) BETWEEN 1 AND 32000),
    created_at bigint NOT NULL DEFAULT extract(epoch FROM now())::bigint
);
CREATE TABLE ticket_receipts (
    workspace_id text NOT NULL REFERENCES workspaces,
    actor_id text NOT NULL,
    operation_id text NOT NULL,
    command jsonb NOT NULL,
    result_id bigint,
    PRIMARY KEY (workspace_id,actor_id,operation_id)
);
CREATE TABLE dispatch_outbox (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    ticket_id bigint NOT NULL REFERENCES tickets,
    generation bigint NOT NULL,
    action text NOT NULL CHECK (action IN ('start','cancel')),
    run_id text NOT NULL,
    dispatched boolean NOT NULL DEFAULT false,
    UNIQUE (ticket_id,generation,action)
);
CREATE TABLE ticket_runs (
    run_id text PRIMARY KEY,
    ticket_id bigint NOT NULL REFERENCES tickets,
    generation bigint NOT NULL,
    agent_id text NOT NULL,
    model text NOT NULL,
    instructions text NOT NULL,
    prompt text NOT NULL,
    state text NOT NULL CHECK (state IN ('queued','running','completed','failed','cancelled')),
    error text,
    UNIQUE(ticket_id,generation)
);
CREATE TABLE agent_credentials (
    token_hash text PRIMARY KEY,
    run_id text NOT NULL REFERENCES ticket_runs,
    workspace_id text NOT NULL REFERENCES workspaces
);
