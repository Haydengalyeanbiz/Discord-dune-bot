mod commands;
mod utils;

use utils::sheets::{complete_request, load_inventory_from_sheets, load_request_from_sheets};

use commands::request::request;
use commands::submit::submit;
use dotenvy::dotenv;
use poise::builtins::register_in_guild;
use poise::serenity_prelude as serenity;
use serenity::{CreateEmbed, CreateMessage, GuildId};
use std::env::var;

type BotError = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, BotError>;
struct Data {}

#[tokio::main]
async fn main() -> Result<(), BotError> {
    dotenv().ok();

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
                // *For de-registering leftover global commands:
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

async fn event_handler(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    _framework: poise::FrameworkContext<'_, Data, BotError>,
    _data: &Data,
) -> Result<(), BotError> {
    match event {
        // Login event demo
        // serenity::FullEvent::Ready { data_about_bot, .. } => {
        //     println!("Logged in as {}", data_about_bot.user.name);
        // }

        // *This is where you catch *all* other interactions,
        // *including button clicks:
        serenity::FullEvent::InteractionCreate { interaction } => {
            if let serenity::Interaction::Component(comp) = interaction.clone() {
                if comp.data.custom_id.starts_with("request_update") {
                    // ! THIS PREVENTS THE TIMEOUT!!!!
                    comp.defer(&ctx.http).await?;

                    let request_id = comp.data.custom_id["request_update:".len()..].to_string();
                    // println!("Request id => {}", request_id);

                    let inventory = load_inventory_from_sheets().await?;
                    let result = load_request_from_sheets(&request_id).await;
                    let (product_name, request_resources, thread_id) = result?;

                    let mut completed = Vec::new();
                    let mut remaining = Vec::new();

                    for (normalized_name, needed_amt) in &request_resources {
                        let stock_amt = inventory.get(normalized_name).copied().unwrap_or(0);

                        if stock_amt >= *needed_amt {
                            completed.push(format!("• {} x {}", needed_amt, normalized_name));
                        } else {
                            remaining.push(format!(
                                "• {} x {}",
                                needed_amt - stock_amt,
                                normalized_name
                            ));
                        }
                    }

                    let embed = CreateEmbed::new()
                        .title(format!("🔷 CRAFTING REQUEST: {}", product_name))
                        .field(
                            "✅ Completed:",
                            if completed.is_empty() {
                                "Nothing yet...".into()
                            } else {
                                completed.join("\n")
                            },
                            false,
                        )
                        .field(
                            "🛠 Remaining Materials:",
                            if remaining.is_empty() {
                                "All materials collected! 🎉".into()
                            } else {
                                remaining.join("\n")
                            },
                            false,
                        );

                    let msg = CreateMessage::new().embed(embed);

                    let response = thread_id.send_message(&ctx.http, msg).await;
                    match response {
                        Ok(_) => (),
                        Err(e) => {
                            println!("❌ Error sending to thread: {:?}", e);
                            use std::io::Write;
                            std::io::stdout().flush().unwrap();
                        }
                    }
                } else if comp.data.custom_id.starts_with("request_complete") {
                    comp.defer(&ctx.http).await?;
                    let request_id = comp.data.custom_id["request_complete:".len()..].to_string();
                    complete_request(&ctx, &comp, &request_id).await?;
                }
            }
        }

        _ => {}
    }
    Ok(())
}
