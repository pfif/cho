use std::collections::HashMap;
use std::env::current_dir;
use std::path::PathBuf;

use clap::Parser;
use rust_decimal::Decimal;

use crate::cli::remaining_operation::legacy_remaining_money_screen::{
    Amount, DisplayRemainingMoneyScreen,
};
use crate::remaining::RemainingOperation;
use crate::vault::{VaultImpl, VaultReadable};

pub fn remaining_operation() {
    let result: Result<String, String> = (|| {
        let arguments = RemainingOptions::parse();
        let vault_path = match &arguments.vault {
            Some(a) => a.clone(),
            None => current_dir().map_err(|e| e.to_string())?,
        };
        let vault = VaultImpl { path: vault_path };

        let predicted_income = match arguments.include_predicted_income {
            true => {
                let raw_amount = PredictedIncome::from_vault(&vault)?;
                Some(raw_amount.into())
            }
            false => None,
        };

        let remaining_money = RemainingOperation::from_vault_value(
            HashMap::from_iter(arguments.exchange_rates),
            arguments.target_currency,
            predicted_income,
            &vault,
        )?;

        let output = remaining_money.execute()?;

        let displayable_remaining_money_screen: DisplayRemainingMoneyScreen =
            output.legacy_money_screen.into();
        Ok(format!("{}", displayable_remaining_money_screen))
    })();

    if let Ok(screen) = result {
        print!("{}", screen)
    } else if let Err(error) = result {
        println!("Could not compute remaining amount: {}", error)
    }
}
type ExchangeRate = (String, Decimal);
// CLI ARGUMENTS PARSING

fn parse_exchange_rate(s: &str) -> Result<ExchangeRate, String> {
    let rate: Option<ExchangeRate> = (|| {
        let splitted_string = s.split(":").collect::<Vec<&str>>();
        if splitted_string.len() != 2 {
            return None;
        }

        let currency = splitted_string[0];
        let raw_rate = splitted_string[1];

        let rate = Decimal::from_str_exact(raw_rate).ok()?;

        Some((currency.into(), rate))
    })();

    rate.ok_or(format!(
        "Could not decode exchange rate {}: Format is {{CURRENCY_NAME}}:{{RATE}}, eg. EUR:0.24561",
        &s
    ))
}

#[derive(Parser)]
#[command()]
struct RemainingOptions {
    #[arg(short = 'r', long = "exchange-rate", value_parser = parse_exchange_rate)]
    exchange_rates: Vec<(String, Decimal)>,

    #[arg(short = 't', long = "target-currency")]
    target_currency: String,

    #[arg(short = 'p', long = "include-predicted")]
    include_predicted_income: bool,

    #[arg(short = 'V', long)]
    vault: Option<PathBuf>,
}

type PredictedIncome = Amount;
impl VaultReadable for PredictedIncome {
    const KEY: &'static str = "predicted_income";
}
