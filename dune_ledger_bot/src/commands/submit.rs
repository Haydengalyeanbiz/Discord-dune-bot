use crate::BotError;
use crate::Context;

use dotenvy::dotenv;
use google_sheets4 as sheets4;
use sheets4::{Sheets, api::ValueRange, hyper_rustls, yup_oauth2};
use std::env;

#[poise::command(slash_command)]
pub async fn submit(
    ctx: Context<'_>,
    #[description = "Resource to submit"] resource: String,
    #[description = "Amount to submit"] amount: i32,
) -> Result<(), BotError> {
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

    let ledger_spreadsheet_id = env::var("SPREADSHEET_ID_LEDGER")?;
    // let request_spreadsheet_id = env::var("SPREADSHEET_ID_REQUEST");
    let range = "Sheet1!A:B";

    let ledger_values = hub
        .spreadsheets()
        .values_get(&ledger_spreadsheet_id, range)
        .doit()
        .await?
        .1
        .values
        .unwrap_or_default();

    let mut found_in_ledger = false;
    let mut updated_ledger_values = vec![];

    for row in ledger_values {
        if let Some(name_val) = row.get(0) {
            if let Some(name) = name_val.as_str() {
                if name.to_lowercase() == resource.to_lowercase() {
                    let current: i32 = row
                        .get(1)
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<i32>().ok())
                        .unwrap_or(0);
                    let new_value = current + amount;
                    updated_ledger_values.push(vec![
                        resource.clone().into(),
                        new_value.to_string().to_lowercase().into(),
                    ]);
                    found_in_ledger = true;
                } else {
                    updated_ledger_values.push(row.clone());
                }
            }
        }
    }

    if !found_in_ledger {
        updated_ledger_values.push(vec![
            resource.clone().into(),
            amount.to_string().to_lowercase().into(),
        ]);
    }

    hub.spreadsheets()
        .values_update(
            ValueRange {
                values: Some(updated_ledger_values),
                ..Default::default()
            },
            &ledger_spreadsheet_id,
            range,
        )
        .value_input_option("RAW")
        .doit()
        .await?;

    ctx.say(format!(
        "✅ Submitted {} of {} to the sheet!",
        amount, resource
    ))
    .await?;
    Ok(())
}
