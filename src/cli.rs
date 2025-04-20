use std::collections::HashMap;
use crate::vault::{VaultImpl, VaultReadable};
use clap::Parser;
use comfy_table::Table;
use rust_decimal::Decimal;
use serde::{forward_to_deserialize_any, Deserialize};
use std::env::current_dir;
use std::fmt::{Display, Formatter};
use std::iter::once;
use std::path::PathBuf;
use crate::remaining_operation::core_types::{IllustrationValue, RemainingOperation, RemainingOperationScreen};
use crate::remaining_operation::amounts::exchange_rates::ExchangeRates;

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

        Ok(format_remaining_operation_screen(&screen))
    })();

    if let Ok(screen) = result {
        print!("{}", screen)
    } else if let Err(error) = result {
        println!("Could not compute remaining amount: {}", error)
    }
}

fn format_remaining_operation_screen(screen: &RemainingOperationScreen) -> String {
    let mut components = vec![title(&format!(
        "Current period : {} to {}", screen.period.start_date, screen.period.end_date,
    ))];

    for group in screen.groups.iter() {
        let mut table = Table::new();
        let group_title = title(group.name());

        // TODO - do we need a column that shows the number used for the math?
        let mut illustration_fields = vec![String::from("Name")];
        illustration_fields.extend(group.illustration_fields());
        table.set_header(illustration_fields);

        for operand in group.operands() {
            let mut illustration_values = vec![operand.name.clone()];

            let raw_illustration_value = operand.illustration.clone()
                .into_iter().map(|(_, value)| value)
                .map(|illustration_value| {
                    match illustration_value {
                        IllustrationValue::Amount(amount) => format!("{}", amount),
                        IllustrationValue::Bool(bool) => (if bool { "✅" } else { "" }).into()
                    }
                });

            illustration_values.extend(raw_illustration_value);
            table.add_row(illustration_values);
        }

        components.push(format!("{}\n{}", group_title, table.to_string()));
    }

    components.push(title(&format!("Remaining this period: {}", screen.remaining)));

    components.join("\n\n")
}


fn title(string: &str) -> String {
    let string_length = string.len();
    string.to_string() + "\n" + &"=".repeat(string_length)
}

// CLI ARGUMENTS PARSING

fn parse_exchange_rate(s: &str) -> Result<(String, Decimal), String> {
    let rate: Option<(String, Decimal)> = (|| {
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