CREATE TABLE IF NOT EXISTS tower_sessions (
    id text PRIMARY KEY NOT NULL,
    expiry_date timestamptz NOT NULL,
    data bytea NOT NULL
);