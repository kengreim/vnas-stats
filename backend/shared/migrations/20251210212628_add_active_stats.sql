-- Store per-datafeed snapshots of active session counts.

CREATE TABLE IF NOT EXISTS session_activity_stats (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    observed_at timestamptz NOT NULL,
    active_controllers integer NOT NULL,
    active_callsigns integer NOT NULL,
    active_positions integer NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);

-- Ensure we only keep one snapshot per observed timestamp.
CREATE UNIQUE INDEX IF NOT EXISTS uq_session_activity_stats_observed_at
    ON session_activity_stats (observed_at);

-- Helpful for range scans by time.
CREATE INDEX IF NOT EXISTS idx_session_activity_stats_observed_at
    ON session_activity_stats (observed_at);
