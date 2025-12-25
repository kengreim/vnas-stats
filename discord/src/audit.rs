use poise::serenity_prelude as serenity;
use serenity::builder::CreateMessage;

/// Send a message to the configured audit channel. No-op if `channel_id` is 0.
pub async fn send_audit_message(
    ctx: &serenity::Context,
    channel_id: u64,
    message: CreateMessage,
) -> Result<(), serenity::Error> {
    if channel_id == 0 {
        return Ok(());
    }

    let channel_id = serenity::ChannelId::new(channel_id);
    channel_id.send_message(&ctx.http, message).await?;
    Ok(())
}
