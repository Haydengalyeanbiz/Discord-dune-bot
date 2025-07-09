mod commands;
use commands::request::request;
use commands::submit::submit;

use dotenvy::dotenv;
use poise::serenity_prelude as serenity;
use std::env;

use google_sheets4 as sheets4;

use sheets4::{Sheets, api::ValueRange, hyper_rustls, yup_oauth2};
// use yup_oauth2::{InstalledFlowAuthenticator, InstalledFlowReturnMethod};
// use hyper;
// use hyper_rustls;
// use hyper_util::client::legacy::Client;

struct Data {}

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

    let hub = Sheets::new(client, authenticator);

    let req = ValueRange {
        major_dimension: None,
        range: None,
        values: Some(vec![vec![
            serde_json::value::Value::String("hello".to_string()),
            serde_json::value::Value::String("world".to_string()),
        ]]),
    };

    let result = hub
        .spreadsheets()
        .values_append(
            req,
            "1Wzp7fWqcgQNQsv7MxAj5wrPm7JrFstFP0RBSoAje8QI",
            "A1:D10",
        )
        .value_input_option("USER_ENTERED")
        .doit()
        .await;
    match result {
        Err(e) => match e {
            // The Error enum provides details about what exactly happened.
            // You can also just use its `Debug`, `Display` or `Error` traits
            sheets4::Error::HttpError(_)
            | sheets4::Error::Io(_)
            | sheets4::Error::MissingAPIKey
            | sheets4::Error::MissingToken(_)
            | sheets4::Error::Cancelled
            | sheets4::Error::UploadSizeLimitExceeded(_, _)
            | sheets4::Error::Failure(_)
            | sheets4::Error::BadRequest(_)
            | sheets4::Error::FieldClash(_)
            | sheets4::Error::JsonDecodeError(_, _) => println!("{}", e),
        },
        Ok(res) => println!("Success: {:?}", res),
    }

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
                Ok(Data {})
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
