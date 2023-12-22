use crate::{Context, Res};
use crate::core::handle_command_error;

/// Show your Discord ID.
#[poise::command(slash_command, ephemeral, guild_only, on_error = "handle_command_error")]
pub async fn test(ctx: Context<'_>) -> Res {
    ctx.say("Test").await?;
    Ok(())
}
