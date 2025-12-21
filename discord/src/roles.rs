use serenity::all::{GuildId, RoleId, UserId};
use crate::AppState;
use crate::db::persist_member;

#[derive(Debug)]
pub enum LookupSource {
    Vatusa,
    Vatsim,
}

#[derive(Debug)]
pub struct LookupResult {
    pub source: LookupSource,
    pub cid: i32,
    pub rating: Option<i32>,
    pub facility: Option<String>,
}

/// Shared lookup + role assignment logic usable by events or commands.
pub async fn sync_and_assign(
    state: &AppState,
    ctx: &serenity::prelude::Context,
    guild_id: GuildId,
    user_id: UserId,
) -> anyhow::Result<()> {
    let lookup = lookup_user(state, user_id).await;

    let role_id = match lookup {
        Ok(ref found) => {
            persist_member(&state.db, user_id.0, found).await?;
            RoleId::new(state.cfg.verified_role_id)
        }
        Err(_) => RoleId::new(state.cfg.fallback_role_id),
    };

    // Assign role; if this fails, surface error so the caller (event/command) can log/report.
    guild_id.member(&ctx.http, user_id).await?.add_role(&ctx.http, role_id, None).await?;

    Ok(())
}

async fn lookup_user(state: &AppState, discord_id: u64) -> anyhow::Result<LookupResult> {
    let try_vatusa = || async move {
        state
            .vatusa
            .get_user_from_discord_id(discord_id)
            .await
            .map(|u| LookupResult {
                source: LookupSource::Vatusa,
                cid: u.cid,
                rating: Some(u.rating),
                facility: Some(u.facility),
            })
            .map_err(anyhow::Error::from)
    };

    let try_vatsim = || async move {
        state
            .vatsim
            .get_user_from_discord_id(discord_id)
            .await
            .map(|u| LookupResult {
                source: LookupSource::Vatsim,
                cid: u.id,
                rating: Some(i32::from(u.rating)),
                facility: Some(u.division_id),
            })
            .map_err(anyhow::Error::from)
    };

    if state.cfg.vatusa_first {
        try_vatusa().await.or_else(|_| try_vatsim().await)
    } else {
        try_vatsim().await.or_else(|_| try_vatusa().await)
    }
}
