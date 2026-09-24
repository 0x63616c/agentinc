CREATE TABLE tool_effects (
    run_id text NOT NULL REFERENCES ticket_runs,
    effect_key text NOT NULL,
    tool text NOT NULL,
    arguments jsonb NOT NULL,
    result jsonb,
    PRIMARY KEY(run_id,effect_key)
);
ALTER TABLE comments ADD COLUMN effect_key text UNIQUE;
