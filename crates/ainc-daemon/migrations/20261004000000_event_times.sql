-- Event times come from clock_timestamp(), not now(): now() is the transaction's start, so
-- records written one after another in a command (a Ticket, its Comment, its Activity) would
-- share a time and sort by accident. Defaults only; no row or reader changes.
ALTER TABLE tickets ALTER COLUMN created_at SET DEFAULT extract(epoch FROM clock_timestamp())::bigint;
ALTER TABLE tickets ALTER COLUMN updated_at SET DEFAULT extract(epoch FROM clock_timestamp())::bigint;
ALTER TABLE comments ALTER COLUMN created_at SET DEFAULT extract(epoch FROM clock_timestamp())::bigint;
ALTER TABLE ticket_links ALTER COLUMN created_at SET DEFAULT extract(epoch FROM clock_timestamp())::bigint;
ALTER TABLE conversations ALTER COLUMN updated_at SET DEFAULT extract(epoch FROM clock_timestamp())::bigint;
ALTER TABLE receipts ALTER COLUMN created_at SET DEFAULT extract(epoch FROM clock_timestamp())::bigint;
ALTER TABLE occurrences ALTER COLUMN scheduled_at SET DEFAULT extract(epoch FROM clock_timestamp())::bigint;
ALTER TABLE automation_history ALTER COLUMN observed_at SET DEFAULT extract(epoch FROM clock_timestamp())::bigint;
