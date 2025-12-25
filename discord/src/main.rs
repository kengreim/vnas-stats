#![warn(clippy::pedantic)]

use crate::api_clients::{VatsimClient, VatusaClient};
use crate::commands::sync_my_roles;
use crate::config::Config;
use crate::event_handler::handle_event;
use crate::health::{HealthState, serve_health};
use crate::jobs::spawn_periodic_sync;
use anyhow::Context;
use poise::{self, serenity_prelude as serenity};
use serenity::GatewayIntents;
use sqlx::PgPool;
use std::sync::OnceLock;

mod api_clients;
mod audit;
mod commands;
mod config;
mod db;
mod event_handler;
mod health;
mod jobs;
mod roles;

type Error = anyhow::Error;
type PoiseContext<'a> = poise::Context<'a, AppState, Error>;

static PERIODIC_SYNC_STARTED: OnceLock<()> = OnceLock::new();

#[derive(Clone)]
pub struct AppState {
    cfg: Config,
    db: PgPool,
    vatusa: VatusaClient,
    vatsim: VatsimClient,
    health: HealthState,
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
    let health = HealthState::new(cfg.guild_id);
    let state = AppState {
        db,
        vatusa: VatusaClient::new_with_client(http_client.clone()),
        vatsim: VatsimClient::new_with_client(http_client),
        cfg: cfg.clone(),
        health: health.clone(),
    };

    let health_addr = cfg.health_addr.parse().context("parse health addr")?;

    tokio::spawn(async move {
        if let Err(err) = serve_health(health, health_addr).await {
            eprintln!("health server stopped: {err:?}");
        }
    });

    let intents = GatewayIntents::GUILDS | GatewayIntents::GUILD_MEMBERS;

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![sync_my_roles(), commands::sync_roles()],
            event_handler: |ctx, event, _framework, data| {
                Box::pin(async move { handle_event(ctx, event, data).await.map_err(Into::into) })
            },
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {
            let state = state.clone();
            Box::pin(async move {
                state.health.set_cache(ctx.cache.clone()).await;
                println!(
                    "bot connected, listening for new members in guild {}",
                    cfg.guild_id
                );
                // Register commands (guild-scoped if configured, otherwise global).
                if let Some(guild_id) = state.cfg.command_guild_id {
                    poise::builtins::register_in_guild(
                        ctx,
                        &framework.options().commands,
                        guild_id.into(),
                    )
                    .await?;
                } else {
                    poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                }
                if PERIODIC_SYNC_STARTED.set(()).is_ok() {
                    spawn_periodic_sync(state.clone(), ctx.clone());
                }
                Ok(state)
            })
        })
        .build();

    let mut client = serenity::ClientBuilder::new(cfg.discord_token, intents)
        .framework(framework)
        .await?;
    client.start().await?;

    Ok(())
}
