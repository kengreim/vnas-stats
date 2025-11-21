pub mod vnas;

use figment::Figment;
use figment::providers::{Env, Format, Toml};
use rsmq_async::RsmqOptions;
use serde::Deserialize;

pub const DATAFEED_QUEUE_NAME: &str = "vnas_stats";
pub const ENV_VAR_PREFIX: &str = "VNAS_STATS_";
pub const SETTINGS_FILE: &str = "Settings.toml";

#[derive(Debug, Deserialize)]
pub struct RedisConfig {
    pub host: String,
    pub port: u16,
    pub db: u8,
    pub username: Option<String>,
    pub password: Option<String>,
    pub namespace: String,
    pub force_recreate: bool,
}

#[derive(Debug, Deserialize)]
pub struct PostgresConfig {
    pub connection_string: String,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub redis: RedisConfig,
    pub postgres: PostgresConfig,
}

pub fn load_config() -> anyhow::Result<Config> {
    Ok(Figment::new()
        .merge(Toml::file(SETTINGS_FILE))
        .merge(Env::prefixed(ENV_VAR_PREFIX).split("_"))
        .extract::<Config>()?)
}

impl From<&RedisConfig> for RsmqOptions {
    fn from(config: &RedisConfig) -> Self {
        Self {
            host: config.host.clone(),
            port: config.port,
            db: config.db,
            realtime: false,
            username: config.username.clone(),
            password: config.password.clone(),
            ns: config.namespace.clone(),
            protocol: Default::default(),
        }
    }
}
