mod commands;
use commands::request::request;
use commands::submit::submit;

use dotenvy::dotenv;
use poise::serenity_prelude as serenity;
use std::env;
use std::sync::Arc;

use google_sheets4 as sheets4;

use hyper_rustls::HttpsConnector;
use sheets4::{Sheets, hyper_rustls, yup_oauth2};
// use hyper::Body;
// use hyper::client::HttpConnector;

// use yup_oauth2::{InstalledFlowAuthenticator, InstalledFlowReturnMethod};
// use hyper;
// use hyper_rustls;
// use hyper_util::client::legacy::Client;

type SheetsClient = sheets4::hyper_util::client::legacy::Client<
    hyper_rustls::HttpsConnector<hyper::client::HttpConnector>,
    hyper::Body,
>;
struct Data {
    hub: Arc<Sheets<SheetsClient>>,
}

type BotError = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, BotError>;

#[tokio::main]
async fn main() -> Result<(), BotError> {
    dotenv().ok();

    let service_account_key =
        yup_oauth2::read_service_account_key("secrets/voltaic-bridge-465115-j2-f15defee98d4.json")
            .await
            .expect("Can't read credential, an error occurred");

    let authenticator = yup_oauth2::ServiceAccountAuthenticator::builder(service_account_key)
        .build()
        .await
        .expect("failed to create authenticator");

    let client = hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
        .build(
            hyper_rustls::HttpsConnectorBuilder::new()
                .with_native_roots()
                .unwrap()
                .https_or_http()
                .enable_http1()
                .build(),
        );

    let hub = Arc::new(Sheets::new(client, authenticator));

    // ─── Bot startup ─────────────────────────────────────────
    let token = env::var("DISCORD_TOKEN").expect("Expected DISCORD_TOKEN in env");
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
                Ok(Data { hub })
            })
        })
        .build();

    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await?;

    // This `.start().await` will run your bot until it shuts down,
    // and then return a `Result<(), serenity::Error>`.
    client.start().await?;

    // If we get here, the bot has cleanly shut down:
    Ok(())
}
