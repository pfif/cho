/*
Period start: 2023/02/01
============

Accounts
========
             | ING       | Credit Mutuel     | Wise            | LINE            | ゆうちょ            | Liquide      | Total
Period start | €0        | €54775.19         | €2889.14        | ¥24796          | ¥3758343            | ¥13000       | €81518.52
Current      | €0 (-€0)  | €54000.00 (-€547) | €3000 (+€28)    | ¥22300 (-¥796)  | ¥3266780 (-¥491563) | ¥12987 (-¥13)| €79518.52 (-€2000)

(+) Predicted Income: €2000
====================

(-) Goals
=========
               | Commited | Commited this period | Target |
Retirment fund | €15000   | … (€500)             | €60000 |
New iPhone     | €300     | ✅                   | €1000  |
Total          | €15300   | … (€2500)            | €61000 |

Remaining this period: €456
*/

use std::collections::HashMap;
use std::env::current_dir;
use std::path::PathBuf;
use crate::remaining::{RemainingOperation, Amount as RemainingAmount};
use crate::vault::{Vault, VaultImpl, VaultReadable};
use clap::Parser;
use rust_decimal::Decimal;
use serde::Deserialize;

#[derive(Deserialize)]
struct PredictedIncome{
    currency: String,
    figure: Decimal,
}

impl VaultReadable for PredictedIncome {
    const KEY: &'static str = "predicted_income";
}

type ExchangeRate = (String, Decimal);
fn parse_exchange_rate(s: &str) -> Result<ExchangeRate, String> {
    let rate: Option<ExchangeRate> = (|| {
        let mut splitted_string = s.split(":").collect::<Vec<&str>>();
        if splitted_string.len() != 2 {
            return None
        }

        let currency = splitted_string[0];
        let raw_rate = splitted_string[1];

        let rate = Decimal::from_str_exact(raw_rate).ok()?;

        Some((currency.into(), rate))
    })();

    rate.ok_or(format!("Could not decode exchange rate {}: Format is {{CURRENCY_NAME}}:{{RATE}}. Example: EUR:0.24561", &s))
}

#[derive(Parser, Debug)]
#[command()]
struct RemainingOptions{
    #[arg(short = 'r', long = "exchange-rate", value_parser = parse_exchange_rate)]
    exchange_rates: Vec<(String, Decimal)>,

    #[arg(short = 't', long = "target-currency")]
    target_currency: String,

    #[arg(short = 'p', long = "include-predicted")]
    include_predicted_income: bool,

    #[arg(short, long)]
    vault: Option<PathBuf>
}

pub fn remaining() {
    let screen: Result<String, String> = (|| {
        let arguments = RemainingOptions::parse();
        let vault_path = match &arguments.vault{
            Some(a) => a.clone(),
            None => current_dir().map_err(|e| e.to_string())?
        };
        let vault = VaultImpl{
            path: vault_path
        };

        let mut predicted_income = None;
        if arguments.include_predicted_income {
            let raw_amount = PredictedIncome::FromVault(&vault)?;
            predicted_income = Some(RemainingAmount{
                currency: raw_amount.currency,
                figure: raw_amount.figure
            });
        }

        let predicted_income = match arguments.include_predicted_income {
            true => {
                let raw_amount = PredictedIncome::FromVault(&vault)?;
                Some(RemainingAmount{
                    currency: raw_amount.currency,
                    figure: raw_amount.figure
                })
            }
            false => None
        };

        let remaining_money = RemainingOperation::FromVaultValue(
            HashMap::from_iter(arguments.exchange_rates),
            arguments.target_currency,
            predicted_income,
            &vault,
        )?;

        let screen = remaining_money.execute()?;

        let account_table = {
            
        }
        // TODO: Display remaining money
        Ok("Hello".into())
    })();
    println!("{:?}", screen)
}
