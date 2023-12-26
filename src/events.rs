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

#[async_trait]
impl EventHandler for GlyfiEvents {
    /// Check whether a user added the submit emoji.
    async fn reaction_add(&self, ctx: Context, r: Reaction) {
        // Ignore anything that isn’t the emoji we care about.
        if !matches!(r.emoji, ReactionType::Custom {id: SUBMIT_EMOJI_ID, .. }) { return; };

        // Make sure we have all the information we need.
        let Some(user) = r.user_id else { return; };
        let Ok(message) = r.message(&ctx).await else { return; };
        let author = message.author.id;

        // Ignore this outside of the submission channels.
        let challenge = match message.channel_id {
            GLYPH_SUBMISSION_CHANNEL_ID => Challenge::Glyph,
            AMBIGRAM_SUBMISSION_CHANNEL_ID => Challenge::Ambigram,
            _ => return
        };

        // Helper to remove the reaction on error and return.
        macro_rules! remove_reaction {
            ($ctx:expr, $r:expr) => {
                if let Err(e) = $r.delete(&$ctx).await { err!("Error removing reaction: {}", e); }
                return;
            };
        }

        // If someone reacted w/ this emoji to someone else’s message, remove it.
        if user != author { remove_reaction!(ctx, r); }

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
            sql::add_submission(message.id, challenge, author, &att.url).await,
            "Error adding submission"
        );

        // Done.
        info!("Added submission from {} for challenge {:?}", user, challenge);
        if let Err(e) = message.react(ctx, ReactionType::Unicode("✅".into())).await {
            err!("Error reacting to submission: {}", e);
        }
    }

    async fn ready(&self, _ctx: Context, ready: Ready) {
        info_sync!("Glyfi running with id {}", ready.user.id);
    }
}
