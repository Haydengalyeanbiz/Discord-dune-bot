use crate::{BotError, Context};
use dashmap::DashMap;
use dotenvy::dotenv;
use google_sheets4 as sheets4;
use hyper_rustls::HttpsConnectorBuilder;
use hyper_util::client::legacy::Client as LegacyClient;
use hyper_util::rt::TokioExecutor;
use once_cell::sync::Lazy;
use poise::serenity_prelude::{
    ChannelId, CreateEmbed, CreateMessage, CreateThread, Message, MessageId, UserId,
};
use regex::Regex;
use sheets4::{Sheets, api::ValueRange, hyper_rustls, yup_oauth2};
use std::{collections::HashMap, env::var};
// for storing a request in-progress, and for the bot to manipulate
const SERVICE_ACCOUNT_PATH: &str = "secrets/voltaic-bridge-465115-j2-f15defee98d4.json";
struct InProgressRequest {
    product: String,
    resources: Vec<(u64, String)>,
    _sheet_row_ids: Vec<String>,
    _message_id: MessageId,
}
static IN_FLIGHT: Lazy<DashMap<UserId, InProgressRequest>> = Lazy::new(Default::default);

#[poise::command(
    slash_command,
    subcommands("start", "bulk_add", "finish"),
    subcommand_required
)]
pub async fn request(_: Context<'_>) -> Result<(), BotError> {
    Ok(())
}

#[poise::command(slash_command)]
pub async fn start(
    ctx: Context<'_>,
    #[description = "Title for the request"] product: String,
) -> Result<(), BotError> {
    dotenv().ok();
    let user = ctx.author().id;

    // Prevent overlapping requests
    if IN_FLIGHT.contains_key(&user) {
        ctx.say("❌ You already have a pending request. Please finish it with `/request finish` before starting a new one.")
            .await?;
        return Ok(());
    }

    let confirmation_builder = CreateMessage::new().content(format!(
        "✅ Request started for **{}**.\n\
             Now add resources with `/request bulk_add`, then finalize with `/request finish`.",
        product
    ));

    // Send confirmation and capture the message ID
    let confirmation: Message = ctx
        .channel_id()
        .send_message(&ctx.http(), confirmation_builder)
        .await?;

    // Store in-flight state
    IN_FLIGHT.insert(
        user,
        InProgressRequest {
            product: product.clone(),
            resources: Vec::new(),
            _sheet_row_ids: Vec::new(),
            _message_id: confirmation.id,
        },
    );

    Ok(())
}

// *Expects raw resource list pasted from crafting calc i.e. https://dune.geno.gg/calculator/
async fn parse_resources(ctx: &Context<'_>, input: &str) -> Result<String, BotError> {
    let re = Regex::new(r"(?<amount>[0-9]+)(?<name>\s+([A-Za-z]+\s*)+)").unwrap();
    // Sanitize input...
    let sanitized_list = input
        .replace(",", "")
        .replace(" x ", " ")
        .replace("-", "")
        .replace("•", "")
        .replace(":", "");
    // ...and parse input into amount:resource pairs
    let mut results: Vec<(u64, String)> = Vec::new();
    for (_, [amount, resource, _]) in re.captures_iter(&sanitized_list).map(|c| c.extract()) {
        results.push((
            amount.parse::<u64>()?,
            resource.trim().to_string().to_lowercase(),
        ));
    }

    // Pull out the in‐flight request, update its resources, and re‐insert
    let user: UserId = ctx.author().id;
    let mut entry = IN_FLIGHT
        .remove(&user)
        .ok_or("❌ You have no active request. Start with `/request start`.")?
        .1;
    entry.resources = results.clone();
    IN_FLIGHT.insert(user, entry);

    // Build human‐readable summary of resource list
    let lines: Vec<String> = results
        .iter()
        .map(|(amount, name)| format!("• {} x {},", amount, name))
        .collect();
    let body = lines.join("\n");
    Ok(body.trim_end_matches(",").to_string())
}

#[poise::command(slash_command)]
pub async fn bulk_add(
    ctx: Context<'_>,
    #[description = "Paste the raw resource list here"] raw_resource_list: String,
) -> Result<(), BotError> {
    let service_account_key = yup_oauth2::read_service_account_key(SERVICE_ACCOUNT_PATH)
        .await
        .expect("Can't read credential, an error occurred");
    let authenticator = yup_oauth2::ServiceAccountAuthenticator::builder(service_account_key)
        .build()
        .await
        .expect("failed to create authenticator");
    let executor = TokioExecutor::new();
    let https_connector = HttpsConnectorBuilder::new()
        .with_native_roots()
        .unwrap()
        .https_or_http()
        .enable_http1()
        .build();
    let client = LegacyClient::builder(executor).build(https_connector);
    let hub = Sheets::new(client, authenticator);
    // Show the user a preview using your existing formatter
    let preview: String = parse_resources(&ctx, &raw_resource_list).await?;
    let user = ctx.author().id;
    let entry = IN_FLIGHT
        .get(&user)
        .ok_or("❌ Could not find in-flight request after parsing.")?;

    // ✅ Build rows to append
    let mut values = vec![];
    for (amount, resource) in entry.resources.iter() {
        values.push(vec![
            entry.product.clone().into(), // Product
            resource.clone().into(),      // Resource name
            amount.to_string().into(),    // Amount
            "in_progress".into(),         // Status (optional)
        ]);
    }

    // ✅ Append to sheet
    let request_range = "Sheet1!A:D";
    let request_spreadsheet_id = var("SPREADSHEET_ID_REQUEST")?;

    hub.spreadsheets()
        .values_append(
            ValueRange {
                range: Some(request_range.to_string()),
                values: Some(values),
                ..Default::default()
            },
            &request_spreadsheet_id,
            request_range,
        )
        .value_input_option("RAW")
        .doit()
        .await?;

    ctx.say(format!(
        "✅ Resources recorded.```{}```Now finalize your request with `/request finish`.",
        preview
    ))
    .await?;
    Ok(())
}

fn normalize_resource_key(s: &str) -> String {
    s.trim_matches('"') // removes leading/trailing `"` if present
        .replace('\u{00a0}', " ") // replace non-breaking space
        .to_lowercase()
        .trim()
        .to_string()
}

async fn load_inventory_from_sheets() -> Result<HashMap<String, u64>, BotError> {
    let service_account_key = yup_oauth2::read_service_account_key(SERVICE_ACCOUNT_PATH)
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
    let inventory_spreadsheet_id = var("SPREADSHEET_ID_INVENTORY")?;
    let range = "Sheet1!A:B";

    let result = hub
        .spreadsheets()
        .values_get(&inventory_spreadsheet_id, &range)
        .doit()
        .await?;

    let values = result.1.values.unwrap_or_default();
    let mut inventory = HashMap::new();
    for row in values {
        if row.len() < 2 {
            continue;
        }

        let name = normalize_resource_key(&row[0].to_string());
        let amount = row[1]
            .as_str()
            .unwrap_or("0")
            .trim()
            .parse::<u64>()
            .unwrap_or(0);
        inventory.insert(name, amount);
    }
    // println!("THIS IS THE INVENTORY => {:?}", inventory);
    Ok(inventory)
}

#[poise::command(slash_command)]
pub async fn finish(ctx: Context<'_>) -> Result<(), BotError> {
    let user = ctx.author().id;

    // Post in a pre-defined channel specific for request threads
    let target_channel_id: ChannelId = var("REQUESTS_CHANNEL_ID")?.parse::<u64>()?.into();

    // Access the stored request data
    let entry = IN_FLIGHT
        .remove(&user)
        .ok_or("You have no active request. Start one with `/request start`.")?
        .1;

    // Compute required diff vs. sheet inventory for final request posting
    let inventory = load_inventory_from_sheets().await?;
    let mut needed: Vec<(u64, String)> = Vec::new();
    let mut completed: Vec<(u64, String)> = Vec::new();

    for (req_amt, name) in entry.resources {
        let normalized_name = normalize_resource_key(&name);
        let stock = *inventory.get(&normalized_name).unwrap_or(&0);
        if stock >= req_amt {
            completed.push((req_amt, name));
        } else if let Some(diff) = req_amt.checked_sub(stock) {
            needed.push((diff, name));
        }
    }
    // println!("THIS IS THE NEEDED => {:?}", needed);
    // Build the embed
    let comp_text = if completed.is_empty() {
    "Nothing yet…".to_string()
    } else {
        completed
            .iter()
            .map(|(amt, nm)| format!("• {} x {}", amt, nm))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let rem_text = if needed.is_empty() {
        "✅ All resources are available in inventory!".to_string()
    } else {
        needed
            .iter()
            .map(|(amt, nm)| format!("• {} x {}", amt, nm))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let embed = CreateEmbed::new()
        .title(format!("🔷 CRAFTING REQUEST: {}", entry.product))
        .field("✅ Completed:", comp_text, false)
        .field("🛠️ Remaining Materials:", rem_text, false);

    let msg_builder = CreateMessage::new().embed(embed.clone());
    let post: Message = target_channel_id
        .send_message(&ctx.http(), msg_builder)
        .await?;

    // Build and create the public thread from that message
    let thread_builder = CreateThread::new(format!("{} - submissions", entry.product));
    let thread = target_channel_id
        .create_thread_from_message(&ctx.http(), post.id, thread_builder)
        .await?;

    // Send your info message in the thread
    let info_builder = CreateMessage::new().content(
        "🛠 Please bring the materials to the Guild base for crafting. \n\n\
         Post below with what you've donated/contributed so we know the progress.\n\n\
         Let us know if you need help locating any of the resources on the list.",
    );
    let _ = thread.send_message(&ctx.http(), info_builder).await?;

    Ok(())
}
