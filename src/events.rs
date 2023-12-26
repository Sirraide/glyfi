use poise::serenity_prelude::*;
use crate::{err, info, info_sync, sql};
use crate::core::report_user_error;
use crate::server_data::{AMBIGRAM_SUBMISSION_CHANNEL_ID, GLYPH_SUBMISSION_CHANNEL_ID, SUBMIT_EMOJI_ID};
use crate::sql::Challenge;

pub struct GlyfiEvents;

/// Execute code and notify the user if execution fails.
macro_rules! run {
    ($ctx:expr, $user:expr, $code:expr, $msg:expr) => {
        if let Err(e) = $code {
            err!("{}: {}", $msg, e);
            report_user_error(
                $ctx,
                $user,
                &format!("Sorry, an internal error occurred: {}: {}", $msg, e)
            ).await;
            return;
        }
    }
}

fn confirm_reaction() -> ReactionType { return ReactionType::Unicode("✅".into()); }

/// Check if we care about a reaction event.
async fn match_relevant_reaction_event(ctx: &Context, r: &Reaction) -> Option<(
    UserId,
    Message,
    Challenge,
)> {
    // Ignore anything that isn’t the emoji we care about.
    if !matches!(r.emoji, ReactionType::Custom {id: SUBMIT_EMOJI_ID, .. }) { return None; };

    // Make sure we have all the information we need.
    let Some(user) = r.user_id else { return None; };
    let Ok(message) = r.message(&ctx).await else { return None; };

    // Ignore this outside of the submission channels.
    let challenge = match message.channel_id {
        GLYPH_SUBMISSION_CHANNEL_ID => Challenge::Glyph,
        AMBIGRAM_SUBMISSION_CHANNEL_ID => Challenge::Ambigram,
        _ => return None
    };

    return Some((user, message, challenge));
}

#[async_trait]
impl EventHandler for GlyfiEvents {
    /// Check whether a user added the submit emoji.
    async fn reaction_add(&self, ctx: Context, r: Reaction) {
        let Some((user, message, challenge)) =
            match_relevant_reaction_event(&ctx, &r).await else { return; };

        // Helper to remove the reaction on error and return.
        macro_rules! remove_reaction {
            ($ctx:expr, $r:expr) => {
                if let Err(e) = $r.delete(&$ctx).await { err!("Error removing reaction: {}", e); }
                return;
            };
        }

        // If someone reacted w/ this emoji to someone else’s message, remove it.
        if user != message.author.id { remove_reaction!(ctx, r); }

        // Check the message for attachments.
        if message.attachments.len() != 1 {
            report_user_error(&ctx, user, "Submissions must contain exactly one image").await;
            remove_reaction!(ctx, r);
        }

        // Safe because we just checked that that is an attachment.
        let att = message.attachments.first().unwrap();

        // Error if the attachment is not an image.
        //
        // There doesn’t really seem to be a way of checking what an attachment
        // actually is (excepting checking the mime type, which I’m not willing
        // to do), so checking whether the height exists, which it only should
        // for images, will have to do.
        if att.height.is_none() {
            report_user_error(&ctx, user, "Submissions must contain only images").await;
            remove_reaction!(ctx, r);
        }

        // Add the submission.
        run!(
            ctx, user,
            sql::add_submission(message.id, challenge, user, &att.url).await,
            "Error adding submission"
        );

        // Done.
        info!("Added submission {} from {} for challenge {:?}", message.id, user, challenge);
        if let Err(e) = message.react(ctx, confirm_reaction()).await {
            err!("Error reacting to submission: {}", e);
        }
    }

    async fn reaction_remove(&self, ctx: Context, r: Reaction) {
        // Check if we care about this.
        let Some((user, message, challenge)) =
            match_relevant_reaction_event(&ctx, &r).await else { return; };

        // If the reaction that was removed is not the reaction of the
        // user that sent the message (which I guess can happen if there
        // is ever some amount of downtime on our part?) then ignore it.
        if user != message.author.id { return; };

        // Remove the submission.
        run!(
            ctx, user,
            sql::remove_submission(message.id, challenge).await,
            "Error removing submission"
        );

        // Done.
        info!("Removed submission {} from {} for challenge {:?}", message.id, user, challenge);

        // Remove our confirmation reaction. This is allowed to fail in case
        // it was already removed somehow.
        let me = ctx.cache.current_user().id;
        let _ = message.delete_reaction(ctx, Some(me), confirm_reaction()).await;
    }

    async fn ready(&self, _ctx: Context, ready: Ready) {
        info_sync!("Glyfi running with id {}", ready.user.id);
    }
}
