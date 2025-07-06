mod commands;
use commands::request::request;
use commands::submit::submit;

use dotenvy::dotenv;
use std::env;

use poise::serenity_prelude as serenity;

struct Data {}

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

#[poise::command(slash_command)]
async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("Pong!").await?;
    Ok(())
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    //  Login with a bot token from the environment
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    let intents = serenity::GatewayIntents::non_privileged();

    let options = poise::FrameworkOptions {
        commands: vec![ping(), submit(), request()],
        ..Default::default()
    };
    // Set gateway intents, which decides what events the bot will be notified about
    let framework = poise::Framework::builder()
        .options(options)
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                let http = ctx.http.clone(); // ensure `http` is in scope
                let guild_id: u64 = std::env::var("GUILD_ID")?.parse()?;
                let guild = serenity::model::id::GuildId::new(guild_id);
                poise::builtins::register_in_guild(&http, &framework.options().commands, guild)
                    .await?;
                // poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {})
            })
        })
        .build();

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await;
    client.unwrap().start().await.unwrap();
}
