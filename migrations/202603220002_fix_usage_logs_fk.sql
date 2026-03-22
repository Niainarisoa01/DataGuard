-- Fix: allow schema deletion even when usage_logs reference it
-- Change schema_id FK from RESTRICT to SET NULL
ALTER TABLE usage_logs
    DROP CONSTRAINT IF EXISTS usage_logs_schema_id_fkey;

ALTER TABLE usage_logs
    ADD CONSTRAINT usage_logs_schema_id_fkey
    FOREIGN KEY (schema_id) REFERENCES schemas(id) ON DELETE SET NULL;
