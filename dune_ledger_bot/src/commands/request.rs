use crate::{Context, Error};

#[poise::command(
    slash_command,
    subcommands("start"/*,"bulk-add"*/), // TODO: bulk add functionality
    subcommand_required
)]
pub async fn request(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command)]
pub async fn start(
    ctx: Context<'_>,
    #[description = "Title for the request"] product: String,
) -> Result<(), Error> {
    ctx.reply(format!("Request recieved for: {}.", product)).await?;
    Ok(())
}