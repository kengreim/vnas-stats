use crate::AppState;
use crate::audit::send_audit_message;
use crate::roles::sync_and_assign;
use poise::serenity_prelude as serenity;

pub async fn handle_event(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    data: &AppState,
) -> Result<(), serenity::Error> {
    match event {
        serenity::FullEvent::GuildMemberAddition { new_member } => {
            match sync_and_assign(data, ctx, new_member.guild_id, new_member.user.id).await {
                Ok(result) => {
                    if data.cfg.audit_channel_id != 0 {
                        let role_label = result
                            .role_name
                            .clone()
                            .unwrap_or_else(|| result.role_id.to_string());
                        let msg = serenity::builder::CreateMessage::new().content(format!(
                            "New member joined: <@{}> → {}",
                            new_member.user.id, role_label
                        ));
                        if let Err(e) =
                            send_audit_message(ctx, data.cfg.audit_channel_id, msg).await
                        {
                            eprintln!("Error sending audit message: {e:?}");
                        }
                    }
                }
                Err(err) => {
                    eprintln!("failed to sync new member {}: {err:?}", new_member.user.id);
                }
            }
        }
        _ => {}
    }

    Ok(())
}
