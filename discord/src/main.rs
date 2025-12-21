use anyhow::{anyhow, Context};
use poise::{self, serenity_prelude as serenity};
use serenity::{GatewayIntents, GuildId, RoleId, UserId};
use sqlx::PgPool;
use crate::commands::sync_my_roles;
use crate::event_handler::handle_event;
use crate::config::Config;
use crate::vatsim::VatsimClient;
use crate::vatusa::VatusaClient;

mod vatusa;
mod vatsim;
mod config;
mod event_handler;
mod commands;
mod roles;
mod db;

type Error = anyhow::Error;
type PoiseContext<'a> = poise::Context<'a, AppState, Error>;

#[derive(Clone)]
pub struct AppState {
    cfg: Config,
    db: PgPool,
    vatusa: VatusaClient,
    vatsim: VatsimClient,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Config::load().context("load config")?;

    let db = PgPool::connect(&cfg.database_url)
        .await
        .context("connect postgres")?;

    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .context("run migrations")?;

    let http_client = reqwest::Client::new();
    let state = AppState {
        db,
        vatusa: VatusaClient::new_with_client(http_client.clone()),
        vatsim: VatsimClient::new_with_client(http_client),
        cfg: cfg.clone(),
    };

    let intents = GatewayIntents::GUILDS | GatewayIntents::GUILD_MEMBERS;

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![sync_my_roles()],
            event_handler: |ctx, event, _framework, data| {
                Box::pin(async move {
                    handle_event(ctx, &event, data).await.map_err(|e| e.into())
                })
            },
            ..Default::default()
        })
        .setup(move |ctx, _ready, _framework| {
            let state = state.clone();
            Box::pin(async move {
                println!("bot connected, listening for new members in guild {}", cfg.guild_id);
                Ok(state)
            })
        }).build();

    let client = serenity::ClientBuilder::new(cfg.discord_token, intents)
        .framework(framework)
        .await;
    client.unwrap().start().await.unwrap();

    Ok(())
}
