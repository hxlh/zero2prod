-- Add migration script here
BEGIN;
-- set default value for status
UPDATE
  subscriptions
SET
  status = 'confirmed'
WHERE
  status IS NULL;

ALTER TABLE subscriptions ALTER COLUMN status SET NOT NULL;

COMMIT;