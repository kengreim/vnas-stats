use sqlx::{Pool, Postgres};
use crate::roles::{LookupResult, LookupSource};
pub async fn persist_member(
    db: &Pool<Postgres>,
    discord_id: u64,
    data: &LookupResult,
) -> Result<(), sqlx::Error> {
    let source = match data.source {
        LookupSource::Vatusa => "vatusa",
        LookupSource::Vatsim => "vatsim",
    };

    sqlx::query(
        r"
        INSERT INTO members (discord_id, source, cid, rating, facility)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT(discord_id) DO UPDATE SET
            source = EXCLUDED.source,
            cid = EXCLUDED.cid,
            rating = EXCLUDED.rating,
            facility = EXCLUDED.facility,
            synced_at = NOW()
        ",
    )
        .bind(discord_id as i64)
        .bind(source)
        .bind(data.cid)
        .bind(data.rating)
        .bind(data.facility.clone())
        .execute(db)
        .await
        .map(|_| ())
}
