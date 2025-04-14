use crate::database::models::ControllerSession;
use sqlx::{Error, Pool, Postgres};

pub async fn get_all_controller_sessions(
    pool: &Pool<Postgres>,
) -> Result<Vec<ControllerSession>, Error> {
    sqlx::query_as("SELECT * FROM controller_sessions")
        .fetch_all(pool)
        .await
}
