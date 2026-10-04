-- S15: a lifecycle column is `status`. Expand step for occurrences.state; the contract step
-- (drop the trigger, the function and `state`) ships in a later release, after every daemon
-- inside the compatibility window reads `status`.
ALTER TABLE occurrences ADD COLUMN status text;
UPDATE occurrences SET status = state;
ALTER TABLE occurrences
    ALTER COLUMN status SET DEFAULT 'waiting_for_worker',
    ALTER COLUMN status SET NOT NULL;

-- An older daemon still writes `state`; a current one writes `status`. Whichever changed
-- wins, so both columns agree for either.
CREATE FUNCTION occurrences_sync_status() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        IF NEW.status <> 'waiting_for_worker' THEN
            NEW.state := NEW.status;
        ELSE
            NEW.status := NEW.state;
        END IF;
    ELSIF NEW.state IS DISTINCT FROM OLD.state THEN
        NEW.status := NEW.state;
    ELSE
        NEW.state := NEW.status;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER occurrences_sync_status BEFORE INSERT OR UPDATE ON occurrences
    FOR EACH ROW EXECUTE FUNCTION occurrences_sync_status();
