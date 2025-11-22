pub mod vnas;

use crate::error::ConfigError;
use figment::Figment;
use figment::providers::{Env, Format, Toml};
use serde::Deserialize;

pub const DATAFEED_QUEUE_NAME: &str = "vnas_stats";
pub const ENV_VAR_PREFIX: &str = "VNAS_STATS_";
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
        .merge(Env::prefixed(ENV_VAR_PREFIX).split("_"))
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
        Config(#[from] crate::ConfigError),
        #[error(transparent)]
        Migration(#[from] sqlx::migrate::MigrateError),
        #[error(transparent)]
        Db(#[from] sqlx::Error),
    }
}
