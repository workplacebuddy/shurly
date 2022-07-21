CREATE TYPE user_role_type AS ENUM(
    'admin',
    'manager'
);

CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY,
    session_id UUID NOT NULL,
    username VARCHAR NOT NULL,
    hashed_password VARCHAR NOT NULL,
    role user_role_type NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMP,
    CONSTRAINT single_username UNIQUE (username)
);

CREATE TABLE IF NOT EXISTS destinations (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id),
    slug VARCHAR NOT NULL,
    url VARCHAR NOT NULL,
    is_permanent BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMP,
    CONSTRAINT single_slug UNIQUE (slug)
);

CREATE TABLE IF NOT EXISTS notes (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id),
    destination_id UUID NOT NULL REFERENCES destinations(id),
    content VARCHAR NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMP
);

CREATE TABLE IF NOT EXISTS hits (
    id UUID PRIMARY KEY,
    destination_id UUID NOT NULL REFERENCES destinations(id),
    ip_address INET,
    user_agent VARCHAR,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TYPE audit_trail_entry_type AS ENUM(
    'create-user',
    'change-password',
    'delete-user',
    'create-destination',
    'update-destination',
    'delete-destination',
    'create-note',
    'update-note',
    'delete-note'
);

CREATE TABLE IF NOT EXISTS audit_trail (
    id UUID PRIMARY KEY,
    created_by UUID NOT NULL REFERENCES users(id),
    type audit_trail_entry_type NOT NULL,
    user_id UUID REFERENCES users(id),
    destination_id UUID REFERENCES destinations(id),
    note_id UUID REFERENCES notes(id),
    ip_address INET,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
