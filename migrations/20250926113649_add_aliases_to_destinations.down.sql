ALTER TABLE audit_trail DROP COLUMN alias_id;

ALTER TABLE hits DROP COLUMN alias_id;

DROP TABLE IF EXISTS aliases;
