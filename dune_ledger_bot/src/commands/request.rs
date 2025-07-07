use crate::{Context, BotError};
// use regex::Regex;

#[poise::command(slash_command, subcommands("start", "bulk_add"), subcommand_required)]
pub async fn request(_: Context<'_>) -> Result<(), BotError> {
    Ok(())
}

#[poise::command(slash_command)]
pub async fn start(
    ctx: Context<'_>,
    #[description = "Title for the request"] product: String,
) -> Result<(), BotError> {
    ctx.reply(format!("Request recieved for: {}.", product))
        .await?;
    Ok(())
}

async fn sanitize(ctx: Context<'_>, input: &str) -> Result<(), BotError> {
    let sanitized_list = input.replace(',', "").replace(" x ", " ").replace("-", "");
    let output: String = format!(
        "✅ Sanitized the following resources:\n```\n{}\n```",
        sanitized_list,
    );
    ctx.say(output).await?;
    Ok(())
}

#[poise::command(slash_command)]
pub async fn bulk_add(
    ctx: Context<'_>,
    #[description = "Paste the raw resouce list here"] raw_resource_list: String,
) -> Result<(), BotError> {
    sanitize(ctx, &raw_resource_list).await?;
    Ok(())
}
