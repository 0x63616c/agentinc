-- The 1.0 board: six statuses, priority, description, labels, ordering inside a
-- column, timestamps, the Conversation a Ticket came from, relationships and history.
ALTER TABLE tickets DROP CONSTRAINT tickets_status_check;
ALTER TABLE tickets ADD CONSTRAINT tickets_status_check
    CHECK (status IN ('backlog','to_do','in_progress','blocked','done','cancelled'));
ALTER TABLE tickets ADD COLUMN description text NOT NULL DEFAULT ''
    CHECK (length(description) <= 32000);
ALTER TABLE tickets ADD COLUMN priority text NOT NULL DEFAULT 'none'
    CHECK (priority IN ('urgent','high','medium','low','none'));
ALTER TABLE tickets ADD COLUMN labels text[] NOT NULL DEFAULT '{}'
    CHECK (cardinality(labels) <= 10);
ALTER TABLE tickets ADD COLUMN position bigint NOT NULL DEFAULT 0;
ALTER TABLE tickets ADD COLUMN created_at bigint NOT NULL
    DEFAULT extract(epoch FROM now())::bigint;
ALTER TABLE tickets ADD COLUMN updated_at bigint NOT NULL
    DEFAULT extract(epoch FROM now())::bigint;
ALTER TABLE tickets ADD COLUMN conversation_id bigint
    REFERENCES conversations ON DELETE SET NULL;
-- Existing Tickets keep their newest-first order inside each column.
UPDATE tickets t SET position = o.n
FROM (SELECT id, row_number() OVER (PARTITION BY workspace_id, status ORDER BY id DESC) AS n
      FROM tickets) o
WHERE o.id = t.id;
CREATE INDEX tickets_board ON tickets(workspace_id, status, position);

-- Every change that bumps a Ticket's revision is an edit; ordering alone is not.
CREATE FUNCTION touch_ticket() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.revision <> OLD.revision THEN
        NEW.updated_at := extract(epoch FROM clock_timestamp())::bigint;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER tickets_touch BEFORE UPDATE ON tickets
    FOR EACH ROW EXECUTE FUNCTION touch_ticket();

-- Directed relationships. `relates_to` is symmetric and stored once, lower ID first.
CREATE TABLE ticket_links (
    workspace_id text NOT NULL REFERENCES workspaces,
    from_id bigint NOT NULL REFERENCES tickets ON DELETE CASCADE,
    to_id bigint NOT NULL REFERENCES tickets ON DELETE CASCADE,
    kind text NOT NULL CHECK (kind IN ('blocks','relates_to','duplicates','parent_of')),
    created_at bigint NOT NULL DEFAULT extract(epoch FROM now())::bigint,
    PRIMARY KEY (from_id, to_id, kind),
    CHECK (from_id <> to_id),
    CHECK (kind <> 'relates_to' OR from_id < to_id)
);
CREATE INDEX ticket_links_to ON ticket_links(to_id);
CREATE UNIQUE INDEX ticket_one_parent ON ticket_links(to_id) WHERE kind = 'parent_of';

-- What happened to a Ticket, by whom, and from which Conversation or run.
CREATE TABLE ticket_activity (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    ticket_id bigint NOT NULL REFERENCES tickets ON DELETE CASCADE,
    actor_id text NOT NULL,
    kind text NOT NULL CHECK (kind IN (
        'created','renamed','described','status','priority','assigned','labels',
        'linked','unlinked','work')),
    from_value text,
    to_value text,
    conversation_id bigint REFERENCES conversations ON DELETE SET NULL,
    run_id text,
    created_at bigint NOT NULL DEFAULT extract(epoch FROM clock_timestamp())::bigint
);
CREATE INDEX ticket_activity_ticket ON ticket_activity(ticket_id, id);
INSERT INTO ticket_activity(ticket_id, actor_id, kind, to_value)
    SELECT id, 'owner', 'created', title FROM tickets ORDER BY id;
