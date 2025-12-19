use crate::state::AppState;
use crate::v1::handlers::auth::{callback, login, logout, me};
use crate::v1::handlers::stats::{get_activity_timeseries, get_iron_mic_stats};
use crate::v1::middleware::auth::require_auth;
use axum::Router;
use axum::middleware::from_fn_with_state;
use axum::routing::get;

pub fn router(state: AppState) -> Router<AppState> {
    Router::<AppState>::new()
        .route("/auth/login", get(login))
        .route("/auth/callback", get(callback))
        .route("/auth/logout", get(logout))
        .route("/auth/me", get(me))
        .route("/callsigns/top", get(get_iron_mic_stats))
        .merge(protected_routes(&state))
}

pub fn protected_routes(state: &AppState) -> Router<AppState> {
    Router::<AppState>::new()
        .route("/activity/timeseries", get(get_activity_timeseries))
        .route_layer(from_fn_with_state(state.clone(), require_auth))
}
