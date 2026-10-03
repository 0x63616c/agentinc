-- One command-receipt table replaces the four per-family copies. The hash rule
-- lives in SQL so stored rows, migrated rows and new lookups share one form.
CREATE FUNCTION receipt_hash(request jsonb) RETURNS text
    LANGUAGE sql IMMUTABLE STRICT
    RETURN encode(sha256(convert_to(request::text, 'UTF8')), 'hex');
CREATE TABLE receipts (
    scope text NOT NULL,
    operation_id uuid NOT NULL,
    request_hash text NOT NULL,
    result jsonb NOT NULL,
    created_at bigint NOT NULL DEFAULT extract(epoch FROM now())::bigint,
    PRIMARY KEY (scope, operation_id)
);
INSERT INTO receipts(scope, operation_id, request_hash, result)
SELECT 'product', operation_id::uuid,
       receipt_hash(jsonb_build_object('workspace', workspace_id, 'command', command)),
       COALESCE(to_jsonb(result_id), 'null'::jsonb)
FROM command_receipts;
INSERT INTO receipts(scope, operation_id, request_hash, result)
SELECT 'ticket/' || workspace_id || '/' || actor_id, operation_id::uuid,
       receipt_hash(command), COALESCE(to_jsonb(result_id), 'null'::jsonb)
FROM ticket_receipts;
INSERT INTO receipts(scope, operation_id, request_hash, result)
SELECT 'automation/' || workspace_id, operation_id::uuid,
       receipt_hash(command), to_jsonb(result_id)
FROM automation_receipts;
INSERT INTO receipts(scope, operation_id, request_hash, result)
SELECT 'workspace', operation_id::uuid, receipt_hash(command), to_jsonb(result_id)
FROM workspace_receipts;
DROP TABLE command_receipts;
DROP TABLE ticket_receipts;
DROP TABLE automation_receipts;
DROP TABLE workspace_receipts;
