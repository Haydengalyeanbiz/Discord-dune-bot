use crate::utils::sheets::{load_inventory_from_sheets, normalize_resource_key};
use crate::{BotError, Context};
use dashmap::DashMap;
use dotenvy::dotenv;
use google_sheets4 as sheets4;
use hyper_rustls::HttpsConnectorBuilder;
use hyper_util::client::legacy::Client as LegacyClient;
use hyper_util::rt::TokioExecutor;
use once_cell::sync::Lazy;
use poise::CreateReply;
use poise::serenity_prelude::{
    ButtonStyle, ChannelId, CreateButton, CreateEmbed, CreateMessage, CreateThread, Message,
    MessageId, UserId,
};
use regex::Regex;
use sheets4::{Sheets, api::ValueRange, hyper_rustls, yup_oauth2};
use std::collections::HashMap;
use std::env::var;
use uuid::Uuid;
use yup_oauth2::{ServiceAccountAuthenticator, read_service_account_key};
const SERVICE_ACCOUNT_PATH: &str = "secrets/voltaic-bridge-465115-j2-f15defee98d4.json";
// For storing an ongoing request in the bot's memory
struct InProgressRequest {
    product: String,
    resources: Vec<(u64, String)>,
    _sheet_row_ids: Vec<String>,
    _message_id: MessageId,
}
static IN_FLIGHT: Lazy<DashMap<UserId, InProgressRequest>> = Lazy::new(Default::default);

// Build the update embed given product / resources / live inventory.
pub fn build_update_embed(
    product: &str,
    resources: &[(u64, String)],
    inventory: &HashMap<String, u64>,
) -> CreateEmbed {
    let (completed, remaining) = compute_completed_remaining(resources, inventory);

    let comp_text = if completed.is_empty() {
        "None yet…".to_string()
    } else {
        completed
            .iter()
            .map(|(amt, nm)| format!("• {} x {}", amt, nm))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let rem_text = if remaining.is_empty() {
        "✅ All materials are now available!".to_string()
    } else {
        remaining
            .iter()
            .map(|(amt, nm)| format!("• {} x {}", amt, nm))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let mut embed = CreateEmbed::new()
        .title(format!("🔄 Update for **{}**", product))
        .field("✅ Completed", comp_text, false)
        .field("🛠️ Remaining", rem_text, false);

    if remaining.is_empty() {
        embed = embed.description(
            "All requested materials are in stock — you can now run `/request finish`!",
        );
    }

    embed
}

/// Given a list of (requested_amount, resource_name) and the live inventory
/// (a simple HashMap from name → stock), returns two Vectors:
/// - completed: items where stock ≥ requested
/// - remaining: items where stock < requested
pub fn compute_completed_remaining(
    resources: &[(u64, String)],
    inventory: &HashMap<String, u64>,
) -> (Vec<(u64, String)>, Vec<(u64, String)>) {
    let mut completed = Vec::new();
    let mut remaining = Vec::new();
    for &(req_amt, ref name) in resources {
        let key = normalize_resource_key(name);
        let stock = *inventory.get(&key).unwrap_or(&0);
        if stock >= req_amt {
            completed.push((req_amt, name.clone()));
        } else {
            remaining.push((req_amt - stock, name.clone()));
        }
    }
    (completed, remaining)
}

//* Expects raw resource list pasted from crafting calc → https://dune.geno.gg/calculator/
async fn parse_resources(ctx: &Context<'_>, input: &str) -> Result<String, BotError> {
    let re = Regex::new(r"(?<amount>[0-9]+)(?<name>\s+([A-Za-z]+\s*)+)").unwrap();

    // Sanitize input...
    let sanitized_list = input
        .replace(",", "")
        .replace(" x ", " ")
        .replace("-", "")
        .replace("•", "")
        .replace(":", "");

    // ...and parse input into "<amount>,<name>" pairs
    let mut parsed_items: Vec<(u64, String)> = Vec::new();
    for caps in re.captures_iter(&sanitized_list) {
        let amt = caps["amount"].parse::<u64>()?;
        let name = caps.name("name").unwrap().as_str().trim().to_lowercase();
        parsed_items.push((amt, name));
    }

    // Convert any water → corpse, dropping <1 corpse
    const WATER_PER_CORPSE: u64 = 45_000;
    let converted: Vec<(u64, String)> = parsed_items
        .iter()
        .filter_map(|&(amt, ref name)| {
            if name == "water" {
                let corpses = amt / WATER_PER_CORPSE;
                if corpses > 0 {
                    Some((corpses, "corpse".to_string()))
                } else {
                    None
                }
            } else {
                Some((amt, name.clone()))
            }
        })
        .collect();

    // Stash request info into the in‐flight request
    let user: UserId = ctx.author().id;
    let mut entry = IN_FLIGHT
        .remove(&user)
        .ok_or("❌ You have no active request. Start with `/request start`.")?
        .1;
    entry.resources = converted.clone();
    IN_FLIGHT.insert(user, entry);

    // Build preview text for the user
    let body = parsed_items
        .iter()
        .map(|(amount, name)| {
            if name == "water" {
                let corpses = amount / WATER_PER_CORPSE;
                format!("• Converted: {} x water → {} x corpse", amount, corpses)
            } else {
                format!("• {} x {}", amount, name)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim_end_matches(",")
        .to_string();

    Ok(body)
}

/// Pulls back all rows whose column A matches `request_id`,
/// returning (amount, name) for each.
pub async fn _query_request_rows(request_id: &str) -> Result<Vec<(u64, String)>, BotError> {
    let sa_key = read_service_account_key("secrets/voltaic-bridge-465115-j2-f15defee98d4.json")
        .await
        .map_err(|e| format!("SA key error: {}", e))?;
    let auth = ServiceAccountAuthenticator::builder(sa_key).build().await?;
    let https_connector = HttpsConnectorBuilder::new()
        .with_native_roots()?
        .https_or_http()
        .enable_http1()
        .build();
    let client = LegacyClient::builder(TokioExecutor::new()).build(https_connector);
    let hub = Sheets::new(client, auth);

    let spreadsheet_id = var("SPREADSHEET_ID_REQUEST")?;
    let raw = hub
        .spreadsheets()
        .values_get(&spreadsheet_id, "Sheet1!A:E")
        .doit()
        .await?
        .1
        .values
        .unwrap_or_default();

    // Filter rows by request_id & collect them to return
    let mut rows = Vec::new();
    for row in raw {
        if row.get(0).and_then(|v| v.as_str()) == Some(request_id) {
            if let (Some(name), Some(amt_val)) = (row.get(2), row.get(3)) {
                if let Some(amt_str) = amt_val.as_str() {
                    if let Ok(amt) = amt_str.parse::<u64>() {
                        rows.push((amt, name.to_string()));
                    }
                }
            }
        }
    }
    Ok(rows)
}

#[poise::command(
    slash_command,
    subcommands("start", "bulk_add", "update", "finish"),
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

#[poise::command(slash_command)]
pub async fn bulk_add(
    ctx: Context<'_>,
    #[description = "Paste the raw resource list here"] raw_resource_list: String,
) -> Result<(), BotError> {
    let preview: String = parse_resources(&ctx, &raw_resource_list).await?;
    let user = ctx.author().id; //? can we refactor this out? not critical...
    let _entry = IN_FLIGHT
        .get(&user) //? not sure where/how this "entry" is being used
        .ok_or("❌ Could not find in-flight request after parsing.")?;

    ctx.say(format!(
        "✅ Resources recorded.```{}```Now finalize your request with `/request finish`.",
        preview
    ))
    .await?;
    Ok(())
}

#[poise::command(slash_command)]
pub async fn update(ctx: Context<'_>) -> Result<(), BotError> {
    let inventory: HashMap<String, u64> = load_inventory_from_sheets().await?;
    let user = ctx.author().id;
    let entry = IN_FLIGHT
        .get(&user)
        .ok_or("❌ You have no active request. Start one with `/request start`.")?;

    let embed = build_update_embed(&entry.product, &entry.resources, &inventory);

    // Build and send the reply in one expression:
    let reply = CreateReply::default().embed(embed);
    ctx.send(reply).await?;

    Ok(())
}

#[poise::command(slash_command)]
pub async fn finish(ctx: Context<'_>) -> Result<(), BotError> {
    let service_account_key = read_service_account_key(SERVICE_ACCOUNT_PATH)
        .await
        .expect("Can't read credential, an error occurred");
    let authenticator = ServiceAccountAuthenticator::builder(service_account_key)
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
    let user = ctx.author().id;

    // Post in a pre-defined channel specific for request threads
    let target_channel_id: ChannelId = var("REQUESTS_CHANNEL_ID")?.parse::<u64>()?.into();

    // Access the stored request data
    let entry = IN_FLIGHT
        .remove(&user)
        .ok_or("You have no active request. Start one with `/request start`.")?
        .1;

    let resources = entry.resources.clone();
    // Unique per‐request ID
    let request_id = Uuid::new_v4().to_string();

    let mut values = vec![];
    for (req_amt, name) in &resources {
        values.push(vec![
            request_id.clone().into(),
            entry.product.clone().into(),
            name.clone().into(),
            req_amt.to_string().into(),
            "in_progress".into(),
        ]);
    }

    let request_range = "Sheet1!A:E";
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

    let request_text = resources
        .iter()
        .map(|(amt, name)| format!("• {} x {}", amt, name))
        .collect::<Vec<_>>()
        .join("\n");

    let embed = CreateEmbed::new()
        .title(format!("🔷 CRAFTING REQUEST: {}", entry.product))
        .field("🛠️ Request Materials:", request_text, false);

    let msg_builder = CreateMessage::new().embed(embed.clone()).button(
        CreateButton::new(format!("request_update:{request_id}"))
            .label("🔄 Update")
            .style(ButtonStyle::Primary),
    );

    let post: Message = target_channel_id
        .send_message(&ctx.http(), msg_builder)
        .await?;

    // Build and create the public thread from that message
    let thread_builder = CreateThread::new(format!("{} - submissions", entry.product));
    let thread = target_channel_id
        .create_thread_from_message(&ctx.http(), post.id, thread_builder)
        .await?;

    // Send static welcome message in the thread
    // TODO: Allow for adjustments to welcome message or request notes
    let info_builder = CreateMessage::new().content(
        "🛠 Please bring the materials to the Guild base for crafting. \n\n\
         Post below with what you've donated/contributed so we know the progress.\n\n\
         Let us know if you need help locating any of the resources on the list.",
    );
    let _ = thread.send_message(&ctx.http(), info_builder).await?;

    Ok(())
}
