CREATE TABLE IF NOT EXISTS members (
    discord_id BIGINT PRIMARY KEY,
    cid INTEGER,
    vatusa_json JSONB,
    vatsim_json JSONB,
    synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
