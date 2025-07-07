use crate::Context;
use crate::BotError;

#[poise::command(slash_command)]
pub async fn submit(
    ctx: Context<'_>,
    #[description = "Resource to submit"] resource: String,
    #[description = "Amount to submit"] amount: i32,

) -> Result<(), BotError> {
    ctx.say(format!("Pong! you submitted {} of {}", amount, resource))
        .await?;
    Ok(())
}
