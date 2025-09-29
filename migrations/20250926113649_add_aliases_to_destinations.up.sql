CREATE TABLE IF NOT EXISTS aliases (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id),
    slug VARCHAR NOT NULL,
    destination_id UUID NOT NULL REFERENCES destinations(id),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMP,
    CONSTRAINT aliases_single_slug UNIQUE (slug)
);

ALTER TABLE hits ADD alias_id UUID NULL REFERENCES aliases(id);

-- no reverse, postgres does not support it
ALTER TYPE audit_trail_entry_type ADD VALUE IF NOT EXISTS 'create-alias' AFTER 'delete-destination';
ALTER TYPE audit_trail_entry_type ADD VALUE IF NOT EXISTS 'delete-alias' AFTER 'create-alias';

ALTER TABLE audit_trail ADD COLUMN alias_id UUID REFERENCES aliases(id);
