mod commands;
use commands::request::request;
use commands::submit::submit;

use std::env;
use dotenvy::dotenv;
use poise::serenity_prelude as serenity;

struct Data {}

type BotError = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, BotError>;


#[tokio::main]
async fn main() -> Result<(), BotError> {
    dotenv().ok();

    // ─── Bot startup ─────────────────────────────────────────
    let token = env::var("DISCORD_TOKEN")
        .expect("Expected DISCORD_TOKEN in env");
    let intents = serenity::GatewayIntents::non_privileged();

    let options = poise::FrameworkOptions {
        commands: vec![submit(), request()],
        ..Default::default()
    };

    let framework = poise::Framework::builder()
        .options(options)
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                let http = &ctx.http;
                let guild_id: u64 = env::var("GUILD_ID")?.parse()?;
                let guild = serenity::model::id::GuildId::new(guild_id);
                poise::builtins::register_in_guild(http, &framework.options().commands, guild)
                    .await?;
                Ok(Data { })
            })
        })
        .build();

    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await?;

    client.start().await?;

    Ok(())
}
