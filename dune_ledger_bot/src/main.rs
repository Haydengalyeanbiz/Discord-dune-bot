mod commands;
mod utils;

use commands::request::request;
use commands::submit::submit;
use dotenvy::dotenv;
use poise::builtins::register_in_guild;
use poise::serenity_prelude as serenity;
// use serenity::builder::{CreateInteractionResponse, CreateInteractionResponseData};
use serenity::{CreateMessage, GuildId, Message};
use std::env::var;

type BotError = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, BotError>;
struct Data {}

#[tokio::main]
async fn main() -> Result<(), BotError> {
    dotenv().ok();

    // ————————————————————————————————————————————————————————————————
    // ─── Bot startup ────────────────────────────────────────────────
    // ————————————————————————————————————————————————————————————————
    let token = var("DISCORD_TOKEN").expect("Expected DISCORD_TOKEN in env");
    let intents = serenity::GatewayIntents::non_privileged();

    let options = poise::FrameworkOptions {
        commands: vec![submit(), request()],
        event_handler: |ctx, event, framework, data| {
            Box::pin(event_handler(ctx, event, framework, data))
        },
        ..Default::default()
    };

    let framework = poise::Framework::builder()
        .options(options)
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                let http = &ctx.http;
                let guild_id: u64 = var("GUILD_ID")?.parse()?;
                let guild = GuildId::new(guild_id);
                // *For de-registering leftover global commands
                // Command::set_global_commands(&ctx.http, Vec::new()).await?;
                register_in_guild(http, &framework.options().commands, guild).await?;
                Ok(Data {})
            })
        })
        .build();

    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await?;
    client.start().await?;

    Ok(())
}

// ————————————————————————————————————————————————————————————————
//  The single async handler for *all* events.
//  Here you can watch for ComponentInteraction or any other event.
// ————————————————————————————————————————————————————————————————
async fn event_handler(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    _framework: poise::FrameworkContext<'_, Data, BotError>,
    _data: &Data,
) -> Result<(), BotError> {
    match event {
        // Login event demo
        serenity::FullEvent::Ready { data_about_bot, .. } => {
            println!("Logged in as {}", data_about_bot.user.name);
        }

        // **This** is where you catch *all* other interactions,
        // including button clicks:
        serenity::FullEvent::InteractionCreate { interaction } => {
            if let serenity::Interaction::Component(comp) = interaction.clone() {
                // dbg!("Comp:", &comp);
                if comp.data.custom_id.starts_with("request_update") {
                    let channel_id = comp.channel_id;

                    let msg_builder = CreateMessage::new().content("🔄 Update received!");
                    let _post: Message = channel_id.send_message(&ctx.http, msg_builder).await?;
                    // your existing "🔄 Update received!" logic:
                    // let _ = comp
                    //     .create_response(&ctx.http, |r: &mut CreateInteractionResponse| {
                    //         r.interaction_response_data(|d: &mut CreateInteractionResponseData| {
                    //             d.content.flags(MessageFlags::EPHEMERAL)
                    //         })
                    //     })
                    //     .await?;
                    //     })
                    // })
                    // .await?;
                }
            }
        }

        _ => {}
    }
    Ok(())
}
