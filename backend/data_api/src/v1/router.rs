use crate::state::AppState;
use crate::v1::handlers::auth::{callback, login, logout, me};
use crate::v1::handlers::stats::{get_activity_timeseries, get_iron_mic_stats};
use axum::Router;
use axum::routing::get;

pub fn router() -> Router<AppState> {
    Router::<AppState>::new()
        .route("/auth/login", get(login))
        .route("/auth/callback", get(callback))
        .route("/auth/logout", get(logout))
        .route("/auth/me", get(me))
        .route("/callsigns/top", get(get_iron_mic_stats))
        .route("/activity/timeseries", get(get_activity_timeseries))
}
