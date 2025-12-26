use std::time::{Duration, Instant};

use poise::serenity_prelude as serenity;
use rand::{Rng, rng};

use crate::AppState;
use crate::audit::send_audit_message;
use crate::roles::sync_and_assign;
use serenity::builder::CreateMessage;

/// Spawn a background task to periodically sync all members in the guild.
pub fn spawn_periodic_sync(state: AppState, ctx: serenity::Context) {
    tokio::spawn(async move {
        // Run immediately, then every 12 hours.
        let mut interval = tokio::time::interval(Duration::from_secs(12 * 60 * 60));
        // Tick once so the first wait happens after an initial run.
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        println!("starting periodic sync loop");
        loop {
            interval.tick().await;
            if let Err(err) = sync_all_members(&state, &ctx).await {
                eprintln!("periodic sync failed: {err:?}");
            }
        }
    });
}

async fn sync_all_members(state: &AppState, ctx: &serenity::Context) -> anyhow::Result<()> {
    println!("syncing all members");
    let guild_id = serenity::GuildId::new(state.cfg.guild_id);
    let mut after = None;
    let mut changes = Vec::new();
    let started = Instant::now();

    if state.cfg.audit_channel_id != 0 {
        let msg = CreateMessage::new().content("Bulk role sync started");
        let _ = send_audit_message(ctx, state.cfg.audit_channel_id, msg).await;
    }

    loop {
        let members = guild_id.members(&ctx.http, Some(1000), after).await?;

        if members.is_empty() {
            break;
        }

        // Track the last id for pagination before consuming the vector.
        after = members.last().map(|m| m.user.id);

        for member in members {
            if member.user.bot {
                continue;
            }
            match sync_and_assign(state, ctx, guild_id, member.user.id).await {
                Ok(result) => {
                    if result.role_changed {
                        changes.push((member.user.id, result.role_id, result.role_name));
                    }
                }
                Err(err) => {
                    eprintln!("failed to sync member {}: {err:?}", member.user.id);
                }
            }

            // Small jitter between requests to avoid hammering upstream APIs.
            let jitter_ms = rng().random_range(1000..=2000);
            tokio::time::sleep(Duration::from_millis(jitter_ms)).await;
        }
    }

    let num_changes = changes.len();
    if state.cfg.audit_channel_id != 0 && !changes.is_empty() {
        let mut lines = Vec::with_capacity(changes.len());

        for (user_id, role_id, role_name) in changes {
            let role_label = role_name.unwrap_or_else(|| role_id.to_string());
            lines.push(format!("<@{user_id}> → {role_label}"));
        }

        let mut message = String::from("Bulk role sync changes:\n");
        for line in lines {
            if message.len() + line.len() + 1 > 1900 {
                let msg = CreateMessage::new().content(message);
                let _ = send_audit_message(ctx, state.cfg.audit_channel_id, msg).await;
                message = String::from("Bulk role sync changes (cont.):\n");
            }
            message.push_str(&line);
            message.push('\n');
        }

        if message.trim_end().len() > "Bulk role sync changes:\n".len() {
            let msg = CreateMessage::new().content(message);
            let _ = send_audit_message(ctx, state.cfg.audit_channel_id, msg).await;
        }
    }

    if state.cfg.audit_channel_id != 0 {
        let duration_ms = started.elapsed().as_millis();
        let msg = CreateMessage::new().content(format!(
            "Bulk role sync complete in {duration_ms} ms ({num_changes} {})",
            if num_changes == 1 {
                "change"
            } else {
                "changes"
            },
        ));
        let _ = send_audit_message(ctx, state.cfg.audit_channel_id, msg).await;
    }

    Ok(())
}
