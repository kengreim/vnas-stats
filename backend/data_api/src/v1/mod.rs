mod api_models;
mod db;
mod handlers;
mod traits;
mod utils;

use crate::v1::handlers::stats::get_iron_mic_stats;
use axum::{Router, routing::get};
use sqlx::{Pool, Postgres};

pub fn router(pool: Pool<Postgres>) -> Router {
    // Router::new()
    //     .route("/controllers/active", get(get_active_controllers))
    //     .route("/controllers", get(get_controller_sessions))
    //     .route("/callsigns/active", get(get_active_callsigns))
    //     .route("/callsigns", get(get_callsign_sessions))
    //     .route("/positions/active", get(get_active_positions))
    //     .with_state(pool)

    Router::new()
        .route("/callsigns/top", get(get_iron_mic_stats))
        .with_state(pool)
}
