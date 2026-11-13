//! Global presentation units, independent of device controls.
use anyhow::Result;
use clap::{Subcommand, ValueEnum};
use llcontrol_ipc::{DisplaySettings, DisplayTemperatureUnit, InterfaceLanguage};
use serde_json::json;
#[derive(Subcommand)]
pub enum Command {
    Get,
    Set {
        #[arg(long, value_enum, required_unless_present = "language")]
        temperature_unit: Option<Unit>,
        #[arg(long, value_enum, required_unless_present = "temperature_unit")]
        language: Option<Language>,
    },
}
#[derive(Clone, Copy, ValueEnum)]
pub enum Unit {
    Celsius,
    Fahrenheit,
}
#[derive(Clone, Copy, ValueEnum)]
pub enum Language {
    Russian,
    English,
}
pub async fn run(command: Command, json_mode: bool) -> Result<()> {
    let client = crate::connection::connect().await?;
    let mut settings: DisplaySettings = serde_json::from_value(
        client
            .call(llcontrol_ipc::methods::GET_DISPLAY_SETTINGS_NAME, json!({}))
            .await?,
    )?;
    settings.validate()?;
    if let Command::Set {
        temperature_unit,
        language,
    } = command
    {
        if let Some(unit) = temperature_unit {
            settings.temperature_unit = match unit {
                Unit::Celsius => DisplayTemperatureUnit::Celsius,
                Unit::Fahrenheit => DisplayTemperatureUnit::Fahrenheit,
            };
        }
        if let Some(language) = language {
            settings.interface_language = match language {
                Language::Russian => InterfaceLanguage::Russian,
                Language::English => InterfaceLanguage::English,
            };
        }
        settings = serde_json::from_value(
            client
                .call(
                    llcontrol_ipc::methods::SET_DISPLAY_SETTINGS_NAME,
                    json!({"settings":settings}),
                )
                .await?,
        )?;
        settings.validate()?;
    }
    if json_mode {
        println!("{}", serde_json::to_string(&settings)?);
    } else {
        println!(
            "Temperature: {} (revision {}), language: {:?}",
            if settings.temperature_unit == DisplayTemperatureUnit::Fahrenheit {
                "°F"
            } else {
                "°C"
            },
            settings.revision,
            settings.interface_language
        );
    }
    Ok(())
}
