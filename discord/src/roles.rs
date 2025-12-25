use crate::AppState;
use crate::api_clients::vatsim::VatsimUserData;
use crate::api_clients::vatusa::VatusaUserData;
use crate::db::persist_member;
use serenity::all::{GuildId, RoleId, UserId};

#[derive(Debug)]
pub struct LookupResult {
    pub cid: Option<i32>,
    pub vatusa_data: Option<VatusaUserData>,
    pub vatsim_data: Option<VatsimUserData>,
}

#[derive(Debug, Clone)]
pub struct SyncResult {
    pub role_id: RoleId,
    pub role_name: Option<String>,
    pub role_changed: bool,
}

/// Shared lookup + role assignment logic usable by events or commands.
pub async fn sync_and_assign(
    state: &AppState,
    ctx: &serenity::prelude::Context,
    guild_id: GuildId,
    user_id: UserId,
) -> anyhow::Result<SyncResult> {
    let lookup = lookup_user(state, user_id.get()).await;

    let role_id = match lookup {
        Ok(ref found) => {
            persist_member(&state.db, user_id.get(), found).await?;
            RoleId::new(state.cfg.verified_role_id)
        }
        Err(_) => RoleId::new(state.cfg.fallback_role_id),
    };

    // Assign role and nickname; if this fails, surface error so the caller (event/command) can log/report.
    let mut member = guild_id.member(&ctx.http, user_id).await?;
    let has_role = member.roles.contains(&role_id);
    if !has_role {
        member.add_role(&ctx.http, role_id).await?;
    }

    if let Ok(ref found) = lookup
        && let Some(nick) = build_nickname(found)
    {
        let builder = serenity::builder::EditMember::new().nickname(nick.clone());
        let _ = member.edit(&ctx.http, builder).await;
    }

    let role_name = ctx
        .cache
        .guild(guild_id)
        .and_then(|g| g.roles.get(&role_id).map(|r| r.name.clone()));

    Ok(SyncResult {
        role_id,
        role_name,
        role_changed: !has_role,
    })
}

async fn lookup_user(state: &AppState, discord_id: u64) -> anyhow::Result<LookupResult> {
    let mut lookup = LookupResult {
        cid: None,
        vatusa_data: None,
        vatsim_data: None,
    };

    if let Ok(res) = state.vatusa.get_user_from_discord_id(discord_id).await {
        lookup.cid = Some(res.cid);
        lookup.vatusa_data = Some(res);
    }

    if let Ok(res) = state.vatsim.get_user_from_discord_id(discord_id).await {
        if lookup.cid.is_none() {
            lookup.cid = Some(res.id);
        }
        lookup.vatsim_data = Some(res);
    }

    if lookup.vatusa_data.is_none() && lookup.vatsim_data.is_none() {
        anyhow::bail!("no VATUSA or VATSIM match");
    }

    Ok(lookup)
}

fn build_nickname(found: &LookupResult) -> Option<String> {
    if let Some(vatusa) = &found.vatusa_data {
        let last = if vatusa.flag_nameprivacy {
            vatusa.lname.chars().next().map(|c| format!("{c}."))?
        } else {
            vatusa.lname.clone()
        };
        return Some(format!("{} {} | {}", vatusa.fname, last, vatusa.facility));
    }

    if let Some(vatsim) = &found.vatsim_data {
        return Some(format!("- {} -", vatsim.id));
    }

    None
}
