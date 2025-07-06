use crate::{Context, Error};
use regex::Regex;

#[poise::command(
    slash_command,
    subcommands("start", "bulk_add"), // TODO: bulk add functionality
    subcommand_required
)]
pub async fn request(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command)]
pub async fn start(
    ctx: Context<'_>,
    #[description = "Title for the request"] product: String,
) -> Result<(), Error> {
    ctx.reply(format!("Request recieved for: {}.", product))
        .await?;
    Ok(())
}

#[poise::command(slash_command)]
pub async fn bulk_add(
    ctx: Context<'_>,
    #[description = "Paste the raw resouce list here"] raw_resource_list: String,
) -> Result<(), Error> {
    regex_parse(ctx, &raw_resource_list).await?;
    Ok(())
}

async fn regex_parse(ctx: Context<'_>, input: &String) -> Result<(), Error> {
    let reg: Regex = Regex::new(r"(?i)^\s*([\d,]+)\s*x\s+(.+?)\s*$").unwrap();
    let mut parsed_lines: Vec<String> = Vec::new();

    for line in input.lines() {
        if let Some(caps) = reg.captures(line) {
            let raw_amount: &str = &caps[1];
            let name: &str = &caps[2];
            let amount: i32 = raw_amount.replace(",", "").parse().unwrap();
            println!("{} x {}", amount, name);
            parsed_lines.push(format!("{} x {}", amount, name))
        }
    }

    if parsed_lines.is_empty() {
        ctx.say("⚠️ No valid resources found.").await?;
    } else {
        let body: String = parsed_lines.join("\n");
        let output: String = format!(
            "✅ Parsed the following resources:\n```\n{}\n```",
            body,
        );
        ctx.say(output).await?;
    }

    Ok(())
}
