pub mod vnas;

use crate::error::ConfigError;
use figment::Figment;
use figment::providers::{Env, Format, Toml};
use serde::Deserialize;
use tokio::signal;
use tokio_util::sync::CancellationToken;

pub const DATAFEED_QUEUE_NAME: &str = "vnas_stats";
pub const ENV_VAR_PREFIX: &str = "VNAS_STATS__";
pub const SETTINGS_FILE: &str = "Settings.toml";

#[derive(Debug, Deserialize)]
pub struct Config {
    pub postgres: PostgresConfig,
}

#[derive(Debug, Deserialize)]
pub struct PostgresConfig {
    pub connection_string: String,
}

pub fn load_config() -> Result<Config, ConfigError> {
    Ok(Figment::new()
        .merge(Toml::file(SETTINGS_FILE))
        .merge(Env::prefixed(ENV_VAR_PREFIX).split("__"))
        .extract::<Config>()?)
}

pub mod error {
    use thiserror::Error;
    use tracing::dispatcher::SetGlobalDefaultError;

    #[derive(Debug, Error)]
    pub enum ConfigError {
        #[error("failed to load configuration: {0}")]
        Figment(#[from] figment::Error),
    }

    #[derive(Debug, Error)]
    pub enum InitializationError {
        #[error(transparent)]
        Tracing(#[from] SetGlobalDefaultError),
        #[error(transparent)]
        Config(#[from] ConfigError),
        #[error(transparent)]
        Migration(#[from] sqlx::migrate::MigrateError),
        #[error(transparent)]
        Db(#[from] sqlx::Error),
    }
}

pub async fn shutdown_listener(token: CancellationToken) {
    let ctrl_c = signal::ctrl_c();
    #[cfg(unix)]
    let terminate = signal::unix::signal(signal::unix::SignalKind::terminate())
        .expect("failed to install SIGTERM handler")
        .recv();
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    token.cancel();
}
