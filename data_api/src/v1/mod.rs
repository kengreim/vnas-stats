mod api_models;
mod db_helpers;
mod handlers;

use axum::{Router, routing::get};

use crate::v1::handlers::active_sessions::{
    get_active_callsigns, get_active_controllers, get_active_positions, get_callsign_sessions,
    get_controller_sessions,
};
use sqlx::{Pool, Postgres};

pub fn router(pool: Pool<Postgres>) -> Router {
    Router::new()
        .route("/controllers/active", get(get_active_controllers))
        .route("/controllers", get(get_controller_sessions))
        .route("/callsigns/active", get(get_active_callsigns))
        .route("/callsigns", get(get_callsign_sessions))
        .route("/positions/active", get(get_active_positions))
        .with_state(pool)
}
