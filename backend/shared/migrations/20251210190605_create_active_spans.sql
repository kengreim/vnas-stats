-- Add active_span columns to sessions tables, backfill, and index them for time-slice queries.

-- Helper function to keep active_span in sync for all session tables.
CREATE OR REPLACE FUNCTION set_active_span() RETURNS trigger AS $$
BEGIN
    NEW.active_span := tstzrange(
        NEW.start_time,
        COALESCE(NEW.end_time, 'infinity'::timestamptz),
        '[)'
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Controller sessions
ALTER TABLE controller_sessions ADD COLUMN IF NOT EXISTS active_span tstzrange;
UPDATE controller_sessions
SET active_span = tstzrange(start_time, COALESCE(end_time, 'infinity'::timestamptz), '[)');
ALTER TABLE controller_sessions ALTER COLUMN active_span SET NOT NULL;

DROP TRIGGER IF EXISTS trg_controller_sessions_active_span ON controller_sessions;
CREATE TRIGGER trg_controller_sessions_active_span
BEFORE INSERT OR UPDATE OF start_time, end_time ON controller_sessions
FOR EACH ROW EXECUTE FUNCTION set_active_span();

CREATE INDEX IF NOT EXISTS idx_controller_sessions_active_span
    ON controller_sessions
    USING GIST (active_span);
CREATE INDEX IF NOT EXISTS idx_controller_sessions_active_only
    ON controller_sessions (start_time)
    WHERE is_active = TRUE;

-- Callsign sessions
ALTER TABLE callsign_sessions ADD COLUMN IF NOT EXISTS active_span tstzrange;
UPDATE callsign_sessions
SET active_span = tstzrange(start_time, COALESCE(end_time, 'infinity'::timestamptz), '[)');
ALTER TABLE callsign_sessions ALTER COLUMN active_span SET NOT NULL;

DROP TRIGGER IF EXISTS trg_callsign_sessions_active_span ON callsign_sessions;
CREATE TRIGGER trg_callsign_sessions_active_span
BEFORE INSERT OR UPDATE OF start_time, end_time ON callsign_sessions
FOR EACH ROW EXECUTE FUNCTION set_active_span();

CREATE INDEX IF NOT EXISTS idx_callsign_sessions_active_span
    ON callsign_sessions
    USING GIST (active_span);
CREATE INDEX IF NOT EXISTS idx_callsign_sessions_active_only
    ON callsign_sessions (start_time)
    WHERE is_active = TRUE;

-- Position sessions
ALTER TABLE position_sessions ADD COLUMN IF NOT EXISTS active_span tstzrange;
UPDATE position_sessions
SET active_span = tstzrange(start_time, COALESCE(end_time, 'infinity'::timestamptz), '[)');
ALTER TABLE position_sessions ALTER COLUMN active_span SET NOT NULL;

DROP TRIGGER IF EXISTS trg_position_sessions_active_span ON position_sessions;
CREATE TRIGGER trg_position_sessions_active_span
BEFORE INSERT OR UPDATE OF start_time, end_time ON position_sessions
FOR EACH ROW EXECUTE FUNCTION set_active_span();

CREATE INDEX IF NOT EXISTS idx_position_sessions_active_span
    ON position_sessions
    USING GIST (active_span);
CREATE INDEX IF NOT EXISTS idx_position_sessions_active_only
    ON position_sessions (start_time)
    WHERE is_active = TRUE;
