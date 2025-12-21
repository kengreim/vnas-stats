use crate::{AppState};
use poise::serenity_prelude as serenity;
use crate::roles::sync_and_assign;

pub async fn handle_event(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    data: &AppState,
) -> Result<(), serenity::Error> {
    match event {
        serenity::FullEvent::GuildMemberAddition {new_member} => {
            if let Err(err) = sync_and_assign(data, ctx, new_member.guild_id, new_member.user.id).await {
                eprintln!("failed to sync new member {}: {err:?}", new_member.user.id);
            }
        }
        _ => {}
    }

    Ok(())
}
