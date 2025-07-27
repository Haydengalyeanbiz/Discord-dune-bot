use crate::BotError;
use dotenvy::dotenv;
use google_sheets4 as sheets4;
use sheets4::{Sheets, api::ValueRange, hyper_rustls, yup_oauth2};
use serde_json::Value;use poise::serenity_prelude as serenity;
use serenity::{
    Context, CreateMessage, ComponentInteraction, CreateEmbed
};
use std::{collections::HashMap, env::var};
const SERVICE_ACCOUNT_PATH: &str = "secrets/voltaic-bridge-465115-j2-f15defee98d4.json";

struct Data;

pub async fn load_inventory_from_sheets() -> Result<HashMap<String, u64>, BotError> {
    dotenv().ok();
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
    Ok(inventory)
}

pub async fn load_request_from_sheets(request_id: &str) -> Result<(String, HashMap<String, u64>), BotError> {
    dotenvy::dotenv().ok();
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

    let request_sheet_id = var("SPREADSHEET_ID_REQUEST")?;
    let sheet_range = "Sheet1!A:E";

    let result = hub
        .spreadsheets()
        .values_get(&request_sheet_id, sheet_range)
        .doit()
        .await?;

    let values = result.1.values.unwrap_or_default();
    let mut product_name = String::new();
    let mut resource_map = HashMap::new();

    for row in values {
        if row.len() < 5 || row[0] != request_id {
            continue;
        }

        if product_name.is_empty() {
            product_name = row[1].to_string().clone();
        }

        let name_raw = row[2].to_string();
        let normalized = normalize_resource_key(&name_raw);
        let amount = row[3]
            .as_str()
            .unwrap_or("0")
            .trim()
            .parse::<u64>()
            .unwrap_or(0);

        resource_map.insert(normalized, amount);
    }

    Ok((product_name, resource_map))
}

pub fn normalize_resource_key(s: &str) -> String {
    s.trim_matches('"')
        .replace('\u{00a0}', " ")
        .to_lowercase()
        .trim()
        .to_string()
}

pub async fn complete_request(
    ctx: &serenity::Context,
    comp: &ComponentInteraction, 
    request_id: &str,
) -> Result<(), BotError> {
    dotenvy::dotenv().ok();
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

    let ledger_sheet_id = std::env::var("SPREADSHEET_ID_INVENTORY")?;
    let request_sheet_id = std::env::var("SPREADSHEET_ID_REQUEST")?;
    let ledger_range = "Sheet1!A:B";
    let request_range = "Sheet1!A:E";

    let mut inventory = load_inventory_from_sheets().await?;
    let (product_name, request_resources) = load_request_from_sheets(request_id).await?;


    let all_satisfied = request_resources.iter().all(|(name, amt)| {
        inventory.get(name).copied().unwrap_or(0) >= *amt
    });

    if !all_satisfied {
        comp.channel_id.send_message(&ctx.http, serenity::builder::CreateMessage::new()
            .content("❌ Not enough resources in inventory to complete this request.")).await?;
        return Ok(());
    }

    for (name, amt) in &request_resources {
        let normalized = normalize_resource_key(name);
        if let Some(stock) = inventory.get_mut(&normalized) {
            *stock -= *amt;
        }
    }

    let new_inventory_values: Vec<Vec<Value>> = inventory.into_iter()
    .map(|(name, amt)| vec![Value::String(name), Value::String(amt.to_string())])
    .collect();

    hub.spreadsheets()
        .values_update(ValueRange {
            range: Some(ledger_range.to_string()),
            values: Some(new_inventory_values),
            ..Default::default()
        }, &ledger_sheet_id, ledger_range)
        .value_input_option("RAW")
        .doit()
        .await?;

    let sheet_data = hub.spreadsheets().values_get(&request_sheet_id, request_range)
        .doit().await?.1.values.unwrap_or_default();

    let mut updated_rows = Vec::new();

    for mut row in sheet_data {
        if row.len() >= 5 && row[0] == request_id {
            row[4] = Value::String("completed".to_string()); // status column
        }
        updated_rows.push(row);
    }

    hub.spreadsheets()
        .values_update(ValueRange {
            range: Some(request_range.to_string()),
            values: Some(updated_rows),
            ..Default::default()
        }, &request_sheet_id, request_range)
        .value_input_option("RAW")
        .doit()
        .await?;

    let embed = CreateEmbed::new()
        .title("✅ CRAFTING COMPLETE")
        .description(format!(
            "{} is complete. All materials have been submitted.",
            product_name,
        ))
        .color(0x00ff00);

    comp.channel_id
    .send_message(&ctx.http, poise::serenity_prelude::CreateMessage::new().embed(embed))
    .await?;
    Ok(())
}
