use sqlx::{Pool, Postgres};
use crate::roles::LookupResult;
pub async fn persist_member(
    db: &Pool<Postgres>,
    discord_id: u64,
    data: &LookupResult,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r"
        INSERT INTO members (discord_id, cid, vatusa_json, vatsim_json)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT(discord_id) DO UPDATE SET
            cid = EXCLUDED.cid,
            vatusa_json = COALESCE(EXCLUDED.vatusa_json, members.vatusa_json),
            vatsim_json = COALESCE(EXCLUDED.vatsim_json, members.vatsim_json),
            synced_at = NOW()
        ",
    )
        .bind(discord_id as i64)
        .bind(data.cid)
        .bind(data.vatusa_json.as_ref())
        .bind(data.vatsim_json.as_ref())
        .execute(db)
        .await
        .map(|_| ())
}
