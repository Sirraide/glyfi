use poise::builtins::register_application_commands;
use poise::CreateReply;
use poise::serenity_prelude::{ButtonStyle, CommandInteraction, CreateActionRow, CreateAttachment, CreateButton, CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter};
use crate::{Context, info, Res, sql};
use crate::core::{create_embed, DEFAULT_EMBED_COLOUR, file_mtime, handle_command_error, InteractionID, SUBMISSION_INTERACTION_ID_CACHE};
use crate::sql::Challenge;

/// Edit your nickname.
#[poise::command(slash_command, ephemeral, guild_only, on_error = "handle_command_error")]
pub async fn nickname(
    ctx: Context<'_>,
    name: String,
) -> Res {
    // Name must not be empty, must not include only whitespace
    // and must not be longer than 200 characters.
    let name = name.trim();
    if name.is_empty() || name.len() > 200 {
        return Err("Name must not be empty and contain at most 200 characters".into());
    }

    // Set nickname.
    sql::set_nickname(ctx.author().id, name).await?;
    ctx.say(format!("Set your nickname to ‘{}’", name)).await?;
    Ok(())
}

/// Display your user profile.
//
// Shows the specified user profile or the user that executes it. Shows
// the user’s UserID, nickname, amount of glyphs submitted, amount of
// ambigrams submitted, the highest ranking in Glyph Challenge, the
// highest ranking in ambigram challenge, & amount of 1st, 2nd, and
// 3rd place placements.
#[poise::command(slash_command, ephemeral, guild_only, on_error = "handle_command_error")]
pub async fn profile(ctx: Context<'_>) -> Res {
    const ZWSP: &str = "\u{200B}";

    let data = sql::get_user_profile(ctx.author().id).await?;
    let name: &str = data.nickname.as_ref()
        .or(ctx.author().global_name.as_ref())
        .unwrap_or(&ctx.author().name)
        .as_str();

    let mut embed = create_embed(&ctx);
    embed = embed.author(CreateEmbedAuthor::new(format!("{}’s Profile", name))
        .icon_url(ctx.author().face())
    );

    // Helper to add a field.
    fn add(embed: CreateEmbed, name: &'static str, value: i64) -> CreateEmbed {
        embed.field(
            name,
            format!(
                "{} time{}",
                value,
                if value == 1 { "" } else { "s" }
            ),
            true,
        )
    }

    let have_glyphs_rating = data.glyphs_first != 0 ||
        data.glyphs_second != 0 ||
        data.glyphs_third != 0;

    let have_ambigrams_rating = data.ambigrams_first != 0 ||
        data.ambigrams_second != 0 ||
        data.ambigrams_third != 0;

    // Add submissions.
    if data.glyphs_submissions != 0 || data.ambigrams_submissions != 0 {
        embed = embed.field("Submitted Glyphs", format!("{}", data.glyphs_submissions), true);
        embed = embed.field("Submitted Ambigrams", format!("{}", data.ambigrams_submissions), true);
        embed = embed.field(ZWSP, ZWSP, true); // Empty field.
    }

    // Add first/second/third place ratings for glyphs challenge.
    if have_glyphs_rating {
        embed = add(embed, "1st Place – G", data.glyphs_first);
        embed = add(embed, "2nd Place – G", data.glyphs_second);
        embed = add(embed, "3nd Place – G", data.glyphs_third);
    } else {
        embed = embed.field(
            "Highest ranking in Glyphs Challenge",
            format!("{}", data.highest_ranking_glyphs),
            false,
        );
    }

    // Add first/second/third place for ambigrams challenge.
    if have_ambigrams_rating {
        embed = add(embed, "1st Place – A", data.ambigrams_first);
        embed = add(embed, "2nd Place – A", data.ambigrams_second);
        embed = add(embed, "3nd Place – A", data.ambigrams_third);
    } else {
        embed = embed.field(
            "Highest ranking in Ambigrams Challenge",
            format!("{}", data.highest_ranking_ambigrams),
            false,
        );
    }

    ctx.send(CreateReply::default().embed(embed)).await?;
    Ok(())
}

/// Submit a glyph/ambigram for a challenge.
#[poise::command(slash_command, ephemeral, guild_only, on_error = "handle_command_error", default_member_permissions = "ADMINISTRATOR")]
pub async fn submit(
    ctx: Context<'_>,
    #[description = "Which challenge to set the prompt for"] challenge: Challenge,
    #[description = "The prompt for the challenge"] prompt: String,
) -> Res {
    // This is gonna take a while...
    ctx.defer_ephemeral().await?;
    let name = match challenge {
        Challenge::Glyph => "glyph_announcement",
        Challenge::Ambigram => "ambigram_announcement",
    };

    // Command for generating the image.
    let mut command = tokio::process::Command::new("./weekly_challenges.py");
    command.arg(name);
    command.arg(&prompt);
    command.kill_on_drop(true);
    command.current_dir("./weekly_challenges");
    info!("Running Shell Command {:?}", command);

    // Run it.
    let res = command.spawn()?.wait().await?;
    if !res.success() { return Err("Failed to generate image".into()); }
    let path = challenge.announcement_image_path();

    // Save prompt.
    let id = sql::add_prompt(challenge, &prompt).await?;

    // Get mtime. This is just a little sanity check.
    let mtime = file_mtime(&path)?;

    // Reply with the image.
    ctx.send(CreateReply::default()
        .attachment(CreateAttachment::path(path).await?)
        .components(vec![CreateActionRow::Buttons(vec![
            CreateButton::new(format!(
                "{}:{}:{}:{}",
                InteractionID::ConfirmAnnouncement.raw(),
                challenge.raw(),
                mtime,
                id
            )).label("Confirm").style(ButtonStyle::Success),
            CreateButton::new(format!(
                "{}:{}",
                InteractionID::CancelAnnouncement.raw(),
                id
            )).label("Cancel").style(ButtonStyle::Danger),
        ])])
    ).await?;
    Ok(())
}

/// Update bot commands.
#[poise::command(slash_command, ephemeral, guild_only, on_error = "handle_command_error", default_member_permissions = "ADMINISTRATOR")]
pub async fn update(ctx: Context<'_>) -> Res {
    register_application_commands(ctx, false).await?;
    Ok(())
}

/// Show stats for a week.
//
// Info shown are: That week’s glyph/ambigram, message link to
// that week’s announcement post, How many submissions there were
// in that week, how many people voted for that week’s submissions,
// message link to that week’s submissions post, top 3 winner names,
// message link to that week’s hall of fame, & the announcement image
// used for that week.
#[poise::command(slash_command, ephemeral, guild_only, on_error = "handle_command_error")]
pub async fn weekinfo(
    ctx: Context<'_>,
    #[description = "Which challenge to get stats for"] challenge: Challenge,
    #[description = "The week whose stats to retrieve"] week: Option<u64>,
) -> Res {
    /*let info = sql::weekinfo(week).await?;
    let mut embed = create_embed(&ctx);
    embed = embed.author(CreateEmbedAuthor::new(format!("Stats for Week {}", info.week)));
    embed = embed.field("Submissions", format!("{}", info.submissions), true);*/
    todo!();


    Ok(())
}