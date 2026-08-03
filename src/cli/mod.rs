use crate::vault::{Vault, VaultImpl, VaultReadable};
use clap::Parser;
use serde::Deserialize;
use std::env::current_dir;
use std::fmt::Display;
use argument_parsing::RemainingOptions;
use crate::remaining_operation::core_types::{RemainingOperation, RemainingOperationScreen};
use crate::amounts::exchange_rates::ExchangeRates;
use crate::period::{CalendarMonthPeriodConfiguration, PeriodConfigurationVaultValue, PeriodsConfiguration};

pub mod formatting;
mod argument_parsing;
mod tests;

pub fn remaining_operation() {
    let result: Result<String, String> = (|| {
        let arguments = RemainingOptions::parse();
        let vault_path = match &arguments.vault {
            Some(a) => a.clone(),
            None => current_dir().map_err(|e| e.to_string())?,
        };
        let vault = VaultImpl { path: vault_path };

        let exchange_rates = ExchangeRates::from_indent_and_rates(arguments.exchange_rates.clone())?;

        let screen = match PeriodConfigurationVaultValue::from_vault(&vault)? {
            PeriodConfigurationVaultValue::CalendarMonth => remaining_screen::<CalendarMonthPeriodConfiguration>(arguments, &vault, exchange_rates)?
        };

        Ok(formatting::format_remaining_operation_screen(&screen))
    })();

    if let Ok(screen) = result {
        print!("{}", screen)
    } else if let Err(error) = result {
        println!("Could not compute remaining amount: {}", error)
    }
}

fn remaining_screen<P: PeriodsConfiguration>(arguments: RemainingOptions, vault: &VaultImpl, exchange_rates: ExchangeRates) -> Result<RemainingOperationScreen, String> {
    let remaining_money = RemainingOperation::<P>::from_vault_values(
        arguments.include_predicted_income,
        vault,
        exchange_rates,
    )?;

    let screen = remaining_money.execute(
        &arguments.target_currency,
    )?;
    Ok(screen)
}