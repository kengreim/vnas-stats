use anyhow::anyhow;
use crate::audit::send_audit_message;
use crate::roles::sync_and_assign;
use crate::{Error, PoiseContext};
use serenity::builder::CreateMessage;
use serenity::model::user::User;
use serenity::model::id::RoleId;

#[poise::command(slash_command, rename = "syncmyroles")]
pub async fn sync_my_roles(ctx: PoiseContext<'_>) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or_else(|| anyhow!("This command can only be used in a server"))?;

    let user_id = ctx.author().id;
    let data = ctx.data();

    let result = sync_and_assign(data, ctx.serenity_context(), guild_id, user_id).await?;

    if data.cfg.audit_channel_id != 0 {
        let role_label = if result.role_id == RoleId::new(data.cfg.verified_role_id) {
            "verified role"
        } else {
            "fallback role"
        };
        let change_label = if result.role_changed {
            "changed"
        } else {
            "unchanged"
        };
        let msg = CreateMessage::new().content(format!(
            "Role sync for <@{}>: {} ({})",
            user_id, role_label, change_label
        ));
        let _ = send_audit_message(ctx.serenity_context(), data.cfg.audit_channel_id, msg).await;
    }
    ctx.say("Role sync complete").await?;
    Ok(())
}

#[poise::command(slash_command, rename = "syncroles", required_permissions = "MANAGE_ROLES")]
pub async fn sync_roles(ctx: PoiseContext<'_>, user: User) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or_else(|| anyhow!("This command can only be used in a server"))?;

    let data = ctx.data();
    let result = sync_and_assign(data, ctx.serenity_context(), guild_id, user.id).await?;

    if data.cfg.audit_channel_id != 0 {
        let role_label = if result.role_id == RoleId::new(data.cfg.verified_role_id) {
            "verified role"
        } else {
            "fallback role"
        };
        let change_label = if result.role_changed {
            "changed"
        } else {
            "unchanged"
        };
        let msg = CreateMessage::new().content(format!(
            "Role sync for <@{}> by <@{}>: {} ({})",
            user.id, ctx.author().id, role_label, change_label
        ));
        let _ = send_audit_message(ctx.serenity_context(), data.cfg.audit_channel_id, msg).await;
    }

    ctx.say(format!("Role sync complete for <@{}>", user.id)).await?;
    Ok(())
}
