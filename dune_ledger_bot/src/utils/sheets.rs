use crate::BotError;
use std::{collections::HashMap, env::var};
use dotenvy::dotenv;
use google_sheets4 as sheets4;
use sheets4::{Sheets, hyper_rustls, yup_oauth2};
const SERVICE_ACCOUNT_PATH: &str = "secrets/voltaic-bridge-465115-j2-f15defee98d4.json";

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
    // println!("THIS IS THE INVENTORY => {:?}", inventory);
    Ok(inventory)
}

pub fn normalize_resource_key(s: &str) -> String {
    s.trim_matches('"') // remove leading/trailing quotes
        .replace('\u{00a0}', " ") // non-breaking space
        .to_lowercase()
        .trim()
        .to_string()
}