use anyhow::anyhow;
use crate::{Error, PoiseContext};
use crate::roles::sync_and_assign;

#[poise::command(slash_command, rename = "syncmyroles")]
pub async fn sync_my_roles(ctx: PoiseContext<'_>) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or_else(|| anyhow!("This command can only be used in a server"))?;

    let user_id = ctx.author().id;
    let data = ctx.data();

    sync_and_assign(data, ctx.serenity_context(), guild_id, user_id).await?;
    ctx.say("Role sync complete").await?;
    Ok(())
}