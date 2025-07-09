use crate::{BotError, Context};
use regex::Regex;
// use regex::Regex;

#[poise::command(slash_command, subcommands("start", "bulk_add"), subcommand_required)]
pub async fn request(_: Context<'_>) -> Result<(), BotError> {
    Ok(())
}

#[poise::command(slash_command)]
pub async fn start(
    ctx: Context<'_>,
    #[description = "Title for the request"] product: String,
) -> Result<(), BotError> {
    ctx.reply(format!("Request received for: {}.", product))
        .await?;
    Ok(())
}

async fn sanitize(ctx: Context<'_>, input: &str) -> Result<(), BotError> {
    let sanitized_list: &str = &input.replace(',', "").replace(" x ", " ").replace("-", "");
    let re = Regex::new(r"(?<amount>[0-9]+)(?<name>\s+([A-Za-z]+\s*)+)").unwrap();

    let mut results: Vec<(u64, String)> = Vec::new();
    for (_, [amount, resource, _]) in re.captures_iter(sanitized_list).map(|c| c.extract()) {
        results.push((amount.parse::<u64>()?, resource.trim().to_string()));
    }
    let lines: Vec<String> = results
        .iter()
        .map(|(amount, name)| format!("• {} x {},", amount, name))
        .collect();

    let body = lines.join("\n");
    let body = body.trim_end_matches(",");
    ctx.say(format!("```✅ Parsed resources:\n{}```", body))
        .await?;
    Ok(())
}

#[poise::command(slash_command)]
pub async fn bulk_add(
    ctx: Context<'_>,
    #[description = "Paste the raw resource list here"] raw_resource_list: String,
) -> Result<(), BotError> {
    sanitize(ctx, &raw_resource_list).await?;
    Ok(())
}
