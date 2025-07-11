use crate::{BotError, Context};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use poise::serenity_prelude::{
    ChannelId, CreateEmbed, CreateMessage, CreateThread, Message, MessageId, UserId,
};
use regex::Regex;
use std::collections::HashMap;

// for storing a request in-progress, and for the bot to manipulate
struct InProgressRequest {
    product: String,
    resources: Vec<(u64, String)>,
    sheet_row_ids: Vec<String>,
    message_id: MessageId,
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
            sheet_row_ids: Vec::new(),
            message_id: confirmation.id,
        },
    );

    Ok(())
}

// *Expects raw resource list pasted from crafting calc i.e. https://dune.geno.gg/calculator/
async fn parse_resources(ctx: &Context<'_>, input: &str) -> Result<String, BotError> {
    // Sanitize input...
    let sanitized_list = input
        .replace(',', "")
        .replace(" x ", " ")
        .replace("-", "")
        .replace("•", "")
        .replace(":", "");
    let re = Regex::new(r"(?<amount>[0-9]+)(?<name>\s+([A-Za-z]+\s*)+)").unwrap();
    // ...and parse input into amount:resource pairs
    let mut results: Vec<(u64, String)> = Vec::new();
    for (_, [amount, resource, _]) in re.captures_iter(&sanitized_list).map(|c| c.extract()) {
        results.push((amount.parse::<u64>()?, resource.trim().to_string()));
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
    // Show the user a preview using your existing formatter
    let preview: String = parse_resources(&ctx, &raw_resource_list).await?;
    ctx.say(format!(
        "✅ Resources recorded.```{}```Now finalize your request with `/request finish`.",
        preview
    ))
    .await?;

    Ok(())
}

async fn load_inventory_from_sheets() -> Result<HashMap<String, u64>, BotError> {
    // TODO: replace with real Sheets lookup
    let inventory = HashMap::new();
    Ok(inventory)
}

#[poise::command(slash_command)]
pub async fn finish(ctx: Context<'_>) -> Result<(), BotError> {
    let user = ctx.author().id;

    // Post in a pre-defined channel specific for request threads
    let target_channel_id: ChannelId = std::env::var("REQUESTS_CHANNEL_ID")?.parse::<u64>()?.into();

    // Access the stored request data
    let entry = IN_FLIGHT
        .remove(&user)
        .ok_or("You have no active request. Start one with `/request start`.")?
        .1;

    // Compute required diff vs. sheet inventory for final request posting
    let inventory = load_inventory_from_sheets().await?;
    let needed: Vec<(u64, String)> = entry
        .resources
        .into_iter()
        .filter_map(|(req_amt, name)| {
            let stock = *inventory.get(&name).unwrap_or(&0);
            req_amt
                .checked_sub(stock)
                .filter(|&n| n > 0)
                .map(|n| (n, name))
        })
        .collect();

    // Build the embed
    let rem_text = needed
        .iter()
        .map(|(amt, nm)| format!("• {} x {}", amt, nm))
        .collect::<Vec<_>>()
        .join("\n");
    let embed = CreateEmbed::new()
        .title(format!("🔷 CRAFTING REQUEST: {}", entry.product))
        .field("✅ Completed:", "Nothing yet…", false)
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
