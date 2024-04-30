/*
Period start: 2023/02/01
============

Accounts
========
             | ING       | Credit Mutuel     | Wise            | LINE            | ゆうちょ              | Liquide      | Total
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

use crate::remaining::{Amount as RemainingAmount, RemainingOperation, DisplayAccount as RemainingDisplayAccount, Figure as RemainingFigure};
use crate::vault::{VaultImpl, VaultReadable};
use clap::Parser;
use comfy_table::Table;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::HashMap;
use std::env::current_dir;
use std::fmt::Display;
use std::iter::once;
use std::path::PathBuf;

type Figure = Decimal;
type Currency = String;
type ExchangeRate = (String, Decimal);
pub type ExchangeRates = HashMap<Currency, Figure>;

#[derive(Deserialize)]
pub struct Amount {
    currency: Currency,
    figure: Figure,
}
impl Into<RemainingAmount> for Amount {
    fn into(self) -> RemainingAmount {
        RemainingAmount {
            currency: self.currency,
            figure: self.figure,
        }
    }
}

impl From<RemainingAmount> for Amount {
    fn from(value: RemainingAmount) -> Self {
        Self {
            currency: value.currency,
            figure: value.figure,
        }
    }
}

impl Display for Amount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO Tech debt - We probably need an actual model for currencies
        let sign = match self.currency.as_str() {
            "EUR" => "€",
            "JPY" => "¥",
            _ => &self.currency,
        };
        write!(f, "{}{}", sign, self.figure)
    }
}

struct DisplayAccount<'a>{
    account: &'a RemainingDisplayAccount
}

impl<'a> DisplayAccount<'a>{
    fn from(value: &'a RemainingDisplayAccount) -> DisplayAccount<'a> {
        DisplayAccount{
            account: value
        }
    }

    fn make_amount(&self, figure: &RemainingFigure) -> Amount {
        return Amount{
            currency: self.account.currency.clone(),
            figure: figure.clone()
        }
    }

    fn name(&self) -> String {
        self.account.name.clone()
    }

    fn period_start_balance(&self) -> Amount{
        self.make_amount(&self.account.period_start_balance)
    }

    fn current_balance(&self) -> Amount {
        self.make_amount(&self.account.current_balance)
    }

    fn difference(&self) -> Amount {
        self.make_amount(&self.account.difference)
    }
}

type PredictedIncome = Amount;
impl VaultReadable for PredictedIncome {
    const KEY: &'static str = "predicted_income";
}


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

#[derive(Parser, Debug)]
#[command()]
struct RemainingOptions {
    #[arg(short = 'r', long = "exchange-rate", value_parser = parse_exchange_rate)]
    exchange_rates: Vec<(String, Decimal)>,

    #[arg(short = 't', long = "target-currency")]
    target_currency: String,

    #[arg(short = 'p', long = "include-predicted")]
    include_predicted_income: bool,

    #[arg(short, long)]
    vault: Option<PathBuf>,
}

pub fn remaining() {
    let screen: Result<String, String> = (|| {
        let arguments = RemainingOptions::parse();
        let vault_path = match &arguments.vault {
            Some(a) => a.clone(),
            None => current_dir().map_err(|e| e.to_string())?,
        };
        let vault = VaultImpl { path: vault_path };

        let predicted_income = match arguments.include_predicted_income {
            true => {
                let raw_amount = PredictedIncome::FromVault(&vault)?;
                Some(raw_amount.into())
            }
            false => None,
        };

        let remaining_money = RemainingOperation::FromVaultValue(
            HashMap::from_iter(arguments.exchange_rates),
            arguments.target_currency,
            predicted_income,
            &vault,
        )?;

        let screen = remaining_money.execute()?;

        let accounts = once(&screen.overall_balance).chain(screen.individual_balances.iter()).map(|account| DisplayAccount::from(account));
        let mut account_table = Table::new();
        account_table
            .set_header(once("".to_string()).chain(accounts.clone().map(|account| account.name())));
        account_table.add_row(
            once("Period start".to_string()).chain(accounts.clone().map(|account| account.period_start_balance().to_string())),
        );
        account_table.add_row(
            once("Current".to_string()).chain(accounts.clone().map(|account| format!("{} ({})", account.current_balance(), account.difference())))
        );
        // TODO: Display remaining money
        Ok("Hello".into())
    })();
    println!("{:?}", screen)
}
