CREATE SCHEMA IF NOT EXISTS data_api;

CREATE TABLE IF NOT EXISTS data_api.sessions (
    id text PRIMARY KEY NOT NULL,
    data bytea NOT NULL,
    expiry_date timestamptz NOT NULL
);