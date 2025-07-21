use crate::{BotError, Context};
use crate::utils::sheets::{load_inventory_from_sheets, normalize_resource_key};

use chrono::{DateTime, Utc};
use dotenvy::dotenv;
use google_sheets4 as sheets4;
use poise::serenity_prelude::AutocompleteChoice;
use sheets4::{Sheets, api::ValueRange, hyper_rustls, yup_oauth2};
use poise::serenity_prelude::{CreateEmbed};
use std::env::var;
use std::collections::HashMap;

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
    "Corpse",
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
    if !ALL_RESOURCES
        .iter()
        .any(|&r| r.eq_ignore_ascii_case(&resource))
    {
        ctx.say(format!(
            "❌ '{}' is not a recognized resource. Please choose from the autocompleted options.",
            resource
        ))
        .await?;
        return Ok(()); // Exit early
    }
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
                        .push(vec![name_val.clone().into(), new_value.into()]);
                    clone_updated_values.push(vec![
                        name_val.clone().into(),
                        amount.into(),
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
            amount.into(),
        ]);
    }

    
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

    let request_spreadsheet_id = var("SPREADSHEET_ID_REQUEST")?;
    let request_range = "Sheet1!A:D";
    let mut inventory = load_inventory_from_sheets().await?; // refresh after update
    let request_result = hub
        .spreadsheets()
        .values_get(&request_spreadsheet_id, request_range)
        .doit()
        .await?;

    let mut request_values = request_result.1.values.unwrap_or_default();
    let mut updated_request_rows = Vec::new();
    let mut completed_by_product: HashMap<String, Vec<(u64, String)>> = HashMap::new();
    let mut needed_by_product: HashMap<String, Vec<(u64, String)>> = HashMap::new();

    for row in request_values.iter_mut() {
        if row.len() < 4 {
            continue;
        }

        let req_resource = normalize_resource_key(row[1].as_str().unwrap_or(""));
        let req_amount: u64 = row[2].as_str().unwrap_or("0").parse().unwrap_or(0);
        let status = row[3].as_str().unwrap_or("");
        let product = row[0].as_str().unwrap_or("").to_string();

        if req_resource == normalize_resource_key(&resource) && status == "in_progress" {
            let stock = *inventory.get(&req_resource).unwrap_or(&0);
            if stock >= req_amount {
                completed_by_product
                    .entry(product.clone())
                    .or_default()
                    .push((req_amount, row[1].to_string().clone()));
                row[2] = "0".into();
                inventory.insert(req_resource.clone(), stock - req_amount);
            } else {
                let remaining = req_amount - stock;
                needed_by_product
                    .entry(product.clone())
                    .or_default()
                    .push((remaining, row[1].to_string().clone()));
                row[2] = remaining.to_string().into();
                inventory.insert(req_resource.clone(), 0);
            }

            updated_request_rows.push(row.clone());
        }
    }

    if !updated_request_rows.is_empty() {
        hub.spreadsheets()
            .values_update(
                ValueRange {
                    range: Some(request_range.to_string()),
                    values: Some(request_values),
                    ..Default::default()
                },
                &request_spreadsheet_id,
                request_range,
            )
            .value_input_option("RAW")
            .doit()
            .await?;
    }

    for (product, completed) in completed_by_product.iter() {
        let empty = Vec::new();
        let needed = needed_by_product.get(product).unwrap_or(&empty);

        let comp_text = if completed.is_empty() {
            "Nothing completed yet.".to_string()
        } else {
            completed
                .iter()
                .map(|(amt, res)| format!("• {} x {}", amt, res))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let rem_text = if needed.is_empty() {
            "✅ All materials fulfilled!".to_string()
        } else {
            needed
                .iter()
                .map(|(amt, res)| format!("• {} x {}", amt, res))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let embed = CreateEmbed::new()
            .title(format!("📦 Updated Request: {}", product))
            .field("✅ Completed:", comp_text, false)
            .field("🛠️ Still Needed:", rem_text, false);

        ctx.send(poise::CreateReply::default().embed(embed)).await?;
    }

    ctx.say(format!(
        "✅ Submitted {} of {} to the sheet!",
        amount, resource
    ))
    .await?;
    Ok(())
}
