use std::net::SocketAddr;
use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Router};
use poise::serenity_prelude as serenity;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct HealthState {
    guild_id: u64,
    cache: Arc<RwLock<Option<Arc<serenity::Cache>>>>,
}

impl HealthState {
    pub fn new(guild_id: u64) -> Self {
        Self {
            guild_id,
            cache: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn set_cache(&self, cache: Arc<serenity::Cache>) {
        *self.cache.write().await = Some(cache);
    }

    async fn is_connected(&self) -> bool {
        let cache_guard = self.cache.read().await;
        let Some(cache) = cache_guard.as_ref() else {
            return false;
        };

        let guild_id = serenity::GuildId::new(self.guild_id);
        cache.guild(guild_id).is_some()
    }
}

pub async fn serve_health(state: HealthState, addr: SocketAddr) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/health", get(health_check))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health_check(State(state): State<HealthState>) -> impl IntoResponse {
    if state.is_connected().await {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}
