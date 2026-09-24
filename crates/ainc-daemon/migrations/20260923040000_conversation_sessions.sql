CREATE TABLE conversation_sessions (
    id text PRIMARY KEY,
    conversation_id bigint REFERENCES conversations ON DELETE SET NULL,
    model text NOT NULL,
    history jsonb NOT NULL,
    event_offset bigint NOT NULL DEFAULT 0,
    state text NOT NULL CHECK (state IN ('active','failed','closed','cancelled'))
);
CREATE UNIQUE INDEX one_active_session ON conversation_sessions(conversation_id) WHERE state='active';
ALTER TABLE turns ADD COLUMN session_id text REFERENCES conversation_sessions;
ALTER TABLE turns ADD COLUMN attempt bigint NOT NULL DEFAULT 0;
