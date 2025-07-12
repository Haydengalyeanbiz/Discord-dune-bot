use crate::{BotError, Context};

use chrono::{DateTime, Utc};
use dotenvy::dotenv;
use google_sheets4 as sheets4;
use poise::serenity_prelude::AutocompleteChoice;
use sheets4::{Sheets, api::ValueRange, hyper_rustls, yup_oauth2};
use std::env::var;

const ALL_RESOURCES: &[&str] = &[
    "Advanced Machinery",
    "Advanced Servoks",
    "Agave Seeds",
    "Aluminum Ore",
    "Armor Plating",
    "Atmospheric Filtered Fabric",
    "Ballistic Weave Fabric",
    "Basalt Stone",
    "Blade Parts",
    "Calibrated Servok",
    "Carbide Blade Parts",
    "Carbide Scraps",
    "Carbon Ore",
    "Complex Machinery",
    "Copper Ore",
    "Diamodine Blade Parts",
    "Diamondine Dust",
    "EMF Generator",
    "Erythrite Crystal",
    "Flour Sand",
    "Fluid Efficient Industrial Pump",
    "Fluted Heavy Caliber Compressor",
    "Fluted Light Caliber Compressor",
    "Fuel Cell",
    "Granite Stone",
    "Gun Parts",
    "Heavy Caliber Compressor",
    "Holtzman Actuator",
    "Hydraulic Piston",
    "Improved Holtzman Actuator",
    "Improved Watertube",
    "Industrial Pump",
    "Insulated Fabric",
    "Iron Ore",
    "Irradiated Core",
    "Irradiated Slag",
    "Jasmium Crystal",
    "Light Caliber Compressor",
    "Mechanical Parts",
    "Microsandwich Fabric",
    "Military Power Regulator",
    "Mouse Corpse",
    "Offworld Medical Supplies",
    "Opafire Gem",
    "Overclocked Power Regulator",
    "Particle Capacitor",
    "Plant Fiber",
    "Plasteel Composite Armor Plating",
    "Plasteel Composite Blade Parts",
    "Plasteel Composite Gun Parts",
    "Plasteel Microflora Fiber",
    "Plasteel Plate",
    "Precision Range Finder",
    "Range Finder",
    "Ray Amplifier",
    "Salvaged Metal",
    "Sandtrout Leathers",
    "Ship Manifest",
    "Solari",
    "Spice Residue",
    "Spice Sand",
    "Spiceinfused Aluminum Dust",
    "Spiceinfused Copper Dust",
    "Spiceinfused Duraluminum Dust",
    "Spiceinfused Iron Dust",
    "Spiceinfused Plastanium Dust",
    "Spiceinfused Steel Dust",
    "Stillsuit Tubing",
    "Stravidium Mass",
    "ThermoResponsive Ray Amplifier",
    "Thermoelectric Cooler",
    "Titanium Ore",
    "TriForged Hydraulic Piston",
    "Water",
];

/// Autocomplete handler for the `resource: String` argument.
async fn resource_autocomplete<'a>(_ctx: Context<'a>, partial: &str) -> Vec<AutocompleteChoice> {
    ALL_RESOURCES
        .iter()
        .filter(|name| name.to_lowercase().contains(&partial.to_lowercase()))
        .take(25)
        .map(|name| AutocompleteChoice::new(name.to_string(), name.to_string()))
        .collect()
}

#[poise::command(slash_command)]
pub async fn submit(
    ctx: Context<'_>,
    // #[description = "Resource to submit"]
    #[autocomplete = "resource_autocomplete"] resource: String,
    #[description = "Amount to submit"] amount: i32,
) -> Result<(), BotError> {
    dotenv().ok();

    // * Sets the service account json file for google authentication
    let service_account_key =
        yup_oauth2::read_service_account_key("secrets/voltaic-bridge-465115-j2-f15defee98d4.json")
            .await
            .expect("Can't read credential, an error occurred");

    // * Builds the authentication using the service account key
    let authenticator = yup_oauth2::ServiceAccountAuthenticator::builder(service_account_key)
        .build()
        .await
        .expect("failed to create authenticator");

    // * Constructs a HTTP client with HTTPS/HTTP support using hyper-util and Tokio
    let client = hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
        .build(
            hyper_rustls::HttpsConnectorBuilder::new()
                .with_native_roots()
                .unwrap()
                .https_or_http()
                .enable_http1()
                .build(),
        );

    // * Creates the hub (SHEET) to target columns and rows and manipulate the sheet
    let hub = Sheets::new(client, authenticator);

    // * Targets the excel spreadsheet by ID
    let inventory_spreadsheet_id = var("SPREADSHEET_ID_INVENTORY")?;
    // let request_spreadsheet_id = var("SPREADSHEET_ID_REQUEST");
    // * Sets the range of the sheet/ targeting specific rows and columns
    let range = "Sheet1!A:B";

    // * Grabs the current data inside of the spreadsheet
    let ledger_values = hub
        .spreadsheets()
        .values_get(&inventory_spreadsheet_id, range)
        .doit()
        .await?
        .1
        .values
        .unwrap_or_default();

    // * Creating variables to check the sheet and a new array for inputing in the sheet
    let mut found_in_ledger = false;
    let mut updated_ledger_values = vec![];
    let mut clone_updated_values = updated_ledger_values.clone();
    let now: DateTime<Utc> = Utc::now();
    let date = now.to_rfc3339();
    let user = ctx.author().name.clone();
    // * Loops through the sheet checking if the resource exists.
    // * Checks if the input resource matches the current resources.
    // * If it does then it takes the input value and adds the current and new value into the array.
    for row in ledger_values {
        if let Some(name_val) = row.get(0) {
            if let Some(name) = name_val.as_str() {
                let resource_key = resource.to_string().to_lowercase();
                if name.to_lowercase() == resource_key {
                    let current: i32 = row
                        .get(1)
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<i32>().ok())
                        .unwrap_or(0);
                    let new_value = current + amount;
                    updated_ledger_values
                        .push(vec![name_val.clone().into(), new_value.to_string().into()]);
                    clone_updated_values.push(vec![
                        name_val.clone().into(),
                        amount.to_string().into(),
                        date.clone().into(),
                        user.clone().into(),
                    ]);
                    found_in_ledger = true;
                } else {
                    updated_ledger_values.push(row.clone());
                }
            }
        }
    }
    // * If not then it creates a new line with the new resource and value and inputs it into the array.
    if !found_in_ledger {
        updated_ledger_values.push(vec![
            resource.clone().to_string().to_lowercase().into(),
            amount.to_string().into(),
        ]);
    }

    // After pushing to the updated values we need a clone for multisheet use

    // Send the original updated values to the inventory sheet
    hub.spreadsheets()
        .values_update(
            ValueRange {
                values: Some(updated_ledger_values),
                ..Default::default()
            },
            &inventory_spreadsheet_id,
            range,
        )
        .value_input_option("RAW")
        .doit()
        .await?;
    //
    let ledger_spreadsheet_id = var("SPREADSHEET_ID_LEDGER")?;
    let ledger_range = "Sheet1!A:D";
    let ledger_values = clone_updated_values;

    hub.spreadsheets()
        .values_append(
            ValueRange {
                values: Some(ledger_values),
                ..Default::default()
            },
            &ledger_spreadsheet_id,
            ledger_range,
        )
        .value_input_option("RAW")
        .doit()
        .await?;
    // * The bot then returns a string stating the resource and value were submitted into the sheet.
    ctx.say(format!(
        "✅ Submitted {} of {} to the sheet!",
        amount, resource
    ))
    .await?;
    Ok(())
}
