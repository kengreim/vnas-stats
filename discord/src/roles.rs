use serenity::all::{CreateMessage, GuildId, RoleId, UserId};
use crate::AppState;
use crate::db::persist_member;
use crate::audit::send_audit_message;
use crate::vatusa::VatusaUserData;
use crate::vatsim::VatsimUserData;
use serde_json::Value;

#[derive(Debug)]
pub enum LookupSource {
    Vatusa,
    Vatsim,
}

#[derive(Debug)]
pub struct LookupResult {
    pub source: LookupSource,
    pub cid: Option<i32>,
    pub vatusa_data: Option<VatusaUserData>,
    pub vatsim_data: Option<VatsimUserData>,
    pub vatusa_json: Option<Value>,
    pub vatsim_json: Option<Value>,
}

/// Shared lookup + role assignment logic usable by events or commands.
pub async fn sync_and_assign(
    state: &AppState,
    ctx: &serenity::prelude::Context,
    guild_id: GuildId,
    user_id: UserId,
) -> anyhow::Result<()> {
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
    member.add_role(&ctx.http, role_id).await?;

    if let Ok(ref found) = lookup {
        if let Some(nick) = build_nickname(found) {
            let builder = serenity::builder::EditMember::new().nickname(nick.clone());
            let _ = member.edit(&ctx.http, builder).await;
        }
    }

    // Fire-and-forget audit message if an audit channel is configured.
    if state.cfg.audit_channel_id != 0 {
        let msg = CreateMessage::new().content(format!("Synced roles for <@{}> using {}", user_id, match role_id {
            r if r == RoleId::new(state.cfg.verified_role_id) => "verified role",
            _ => "fallback role",
        }));

        let _ = send_audit_message(
            ctx,
            state.cfg.audit_channel_id,
            msg,
        )
        .await;
    }

    Ok(())
}

async fn lookup_user(state: &AppState, discord_id: u64) -> anyhow::Result<LookupResult> {
    let try_vatusa = async {
        state
            .vatusa
            .get_user_from_discord_id(discord_id)
            .await
            .map(|u| LookupResult {
                source: LookupSource::Vatusa,
                cid: Some(u.cid),
                vatusa_json: Some(serde_json::to_value(&u).unwrap()),
                vatsim_json: None,
                vatusa_data: Some(u),
                vatsim_data: None,
            })
            .map_err(anyhow::Error::from)
    };

    let try_vatsim = async {
        state
            .vatsim
            .get_user_from_discord_id(discord_id)
            .await
            .map(|u| LookupResult {
                source: LookupSource::Vatsim,
                cid: Some(u.id),
                vatusa_json: None,
                vatsim_json: Some(serde_json::to_value(&u).unwrap()),
                vatusa_data: None,
                vatsim_data: Some(u),
            })
            .map_err(anyhow::Error::from)
    };

    if state.cfg.vatusa_first {
        match try_vatusa.await {
            Ok(res) => Ok(res),
            Err(_) => try_vatsim.await,
        }
    } else {
        match try_vatsim.await {
            Ok(res) => Ok(res),
            Err(_) => try_vatusa.await,
        }
    }
}

fn build_nickname(found: &LookupResult) -> Option<String> {
    if let Some(vatusa) = &found.vatusa_data {
        let last = if vatusa.flag_nameprivacy {
            vatusa.lname.chars().next().map(|c| format!("{}.", c))?
        } else {
            vatusa.lname.clone()
        };
        return Some(format!("{} {} | {}", vatusa.fname, last, vatusa.facility));
    }

    if let Some(vatsim) = &found.vatsim_data {
        return Some(format!("{}", vatsim.id));
    }

    None
}
