use figment::Figment;
use figment::providers::{Env, Serialized};

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Config {
    pub discord_token: String,
    pub guild_id: u64,
    pub verified_role_id: u64,
    pub fallback_role_id: u64,
    pub audit_channel_id: u64,
    pub command_guild_id: Option<u64>,
    pub database_url: String,
    pub health_addr: String,
}

impl Config {
    pub fn load() -> Result<Self, figment::Error> {
        Figment::new()
            .merge(Serialized::defaults(Config {
                discord_token: String::new(),
                guild_id: 0,
                verified_role_id: 0,
                fallback_role_id: 0,
                audit_channel_id: 0,
                command_guild_id: None,
                database_url: "postgres://user:pass@localhost:5432/discord".to_string(),
                health_addr: "127.0.0.1:3000".to_string(),
            }))
            .merge(Env::prefixed("DISCORD__").split("__"))
            .extract()
    }
}
