use crate::vault::{VaultImpl, VaultReadable};
use clap::Parser;
use serde::Deserialize;
use std::env::current_dir;
use std::fmt::Display;
use argument_parsing::RemainingOptions;
use crate::remaining_operation::core_types::RemainingOperation;
use crate::remaining_operation::amounts::exchange_rates::ExchangeRates;

mod formatting;
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

        let exchange_rates = ExchangeRates::from_indent_and_rates(arguments.exchange_rates)?;

        // TODO call from vault value from the new module...
        let remaining_money = RemainingOperation::from_vault_values(
            arguments.include_predicted_income,
            &vault,
        )?;

        let screen = remaining_money.execute(
            &arguments.target_currency,
            &exchange_rates,
        )?;

        Ok(formatting::format_remaining_operation_screen(&screen))
    })();

    if let Ok(screen) = result {
        print!("{}", screen)
    } else if let Err(error) = result {
        println!("Could not compute remaining amount: {}", error)
    }
}
