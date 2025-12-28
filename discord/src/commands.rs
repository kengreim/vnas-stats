use crate::audit::send_audit_message;
use crate::roles::sync_and_assign;
use crate::{Error, PoiseContext};
use anyhow::anyhow;
use serenity::builder::CreateMessage;
use serenity::model::user::User;

/// Sync your roles based on your Discord account's links to VATSIM and VATUSA
#[poise::command(slash_command, rename = "syncmyroles")]
pub async fn sync_my_roles(ctx: PoiseContext<'_>) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or_else(|| anyhow!("This command can only be used in a server"))?;

    let pending = ctx
        .send(
            poise::CreateReply::default()
                .content("Syncing your roles...")
                .ephemeral(true),
        )
        .await?;

    let user_id = ctx.author().id;
    let data = ctx.data();

    let result = sync_and_assign(data, ctx.serenity_context(), guild_id, user_id).await?;

    if data.cfg.audit_channel_id != 0 {
        let role_label = result
            .role_name
            .clone()
            .unwrap_or_else(|| result.role_id.to_string());
        let change_label = if result.role_changed {
            "changed"
        } else {
            "unchanged"
        };
        let msg = CreateMessage::new().content(format!(
            "Role sync for <@{user_id}>: {role_label} ({change_label})"
        ));
        let _ = send_audit_message(ctx.serenity_context(), data.cfg.audit_channel_id, msg).await;
    }
    pending
        .edit(
            ctx,
            poise::CreateReply::default().content("Role sync complete"),
        )
        .await?;
    Ok(())
}

/// Admin-only command to sync roles for a User based on their account's links to VATSIM and VATUSA
#[poise::command(
    slash_command,
    rename = "syncroles",
    required_permissions = "MANAGE_ROLES"
)]
pub async fn sync_roles(
    ctx: PoiseContext<'_>,
    #[description = "user whose roles will be synced"] user: User,
) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or_else(|| anyhow!("This command can only be used in a server"))?;

    let pending = ctx
        .send(
            poise::CreateReply::default()
                .content(format!("Syncing roles for <@{}>...", user.id))
                .ephemeral(true),
        )
        .await?;

    let data = ctx.data();
    let result = sync_and_assign(data, ctx.serenity_context(), guild_id, user.id).await?;

    if data.cfg.audit_channel_id != 0 {
        let role_label = result
            .role_name
            .clone()
            .unwrap_or_else(|| result.role_id.to_string());
        let change_label = if result.role_changed {
            "changed"
        } else {
            "unchanged"
        };
        let msg = CreateMessage::new().content(format!(
            "Role synced for <@{}> by <@{}>: {} ({})",
            user.id,
            ctx.author().id,
            role_label,
            change_label
        ));
        let _ = send_audit_message(ctx.serenity_context(), data.cfg.audit_channel_id, msg).await;
    }

    pending
        .edit(
            ctx,
            poise::CreateReply::default().content(format!("Role sync complete for <@{}>", user.id)),
        )
        .await?;
    Ok(())
}
