use crate::roles::LookupResult;
use sqlx::{Pool, Postgres};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error(transparent)]
    SqlxError(#[from] sqlx::Error),
    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),
}

pub async fn persist_member(
    db: &Pool<Postgres>,
    discord_id: u64,
    data: &LookupResult,
) -> Result<(), DbError> {
    let vatusa_json = data
        .vatusa_data
        .as_ref()
        .map(serde_json::to_value)
        .transpose()?;

    let vatsim_json = data
        .vatsim_data
        .as_ref()
        .map(serde_json::to_value)
        .transpose()?;

    let _ = sqlx::query(
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
    .bind(vatusa_json.as_ref())
    .bind(vatsim_json.as_ref())
    .execute(db)
    .await?;

    Ok(())
}
