use std::time::Duration;

use poise::serenity_prelude as serenity;
use rand::{thread_rng, Rng};

use crate::roles::sync_and_assign;
use crate::AppState;

/// Spawn a background task to periodically sync all members in the guild.
pub fn spawn_periodic_sync(state: AppState, ctx: serenity::Context) {
    tokio::spawn(async move {
        // Run immediately, then every 12 hours.
        let mut interval = tokio::time::interval(Duration::from_secs(12 * 60 * 60));
        // Tick once so the first wait happens after an initial run.
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            if let Err(err) = sync_all_members(&state, &ctx).await {
                eprintln!("periodic sync failed: {err:?}");
            }
            interval.tick().await;
        }
    });
}

async fn sync_all_members(state: &AppState, ctx: &serenity::Context) -> anyhow::Result<()> {
    let guild_id = serenity::GuildId::new(state.cfg.guild_id);
    let mut after = None;

    loop {
        let members = guild_id
            .members(&ctx.http, Some(1000), after)
            .await?;

        if members.is_empty() {
            break;
        }

        // Track the last id for pagination before consuming the vector.
        after = members.last().map(|m| m.user.id);

        for member in members {
            if let Err(err) = sync_and_assign(state, ctx, guild_id, member.user.id).await {
                eprintln!("failed to sync member {}: {err:?}", member.user.id);
            }

            // Small jitter between requests to avoid hammering upstream APIs.
            let jitter_ms = thread_rng().gen_range(200..=1200);
            tokio::time::sleep(Duration::from_millis(jitter_ms)).await;
        }
    }

    Ok(())
}
