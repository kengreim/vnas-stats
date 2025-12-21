CREATE TABLE IF NOT EXISTS members (
    discord_id BIGINT PRIMARY KEY,
    source TEXT NOT NULL,
    cid INTEGER NOT NULL,
    rating INTEGER,
    facility TEXT,
    synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
