/*
Period start: 2023/02/01
============

Accounts
========
             | ING       | Credit Mutuel     | Wise            | LINE            | ゆうちょ              | Liquide      | Total
Period start | €0        | €54775.19         | €2889.14        | ¥24796          | ¥3758343            | ¥13000       | €81518.52
Current      | €0 (-€0)  | €54000.00 (-€547) | €3000 (+€28)    | ¥22300 (-¥796)  | ¥3266780 (-¥491563) | ¥12987 (-¥13)| €79518.52 (-€2000)

(+) Predicted Income: €2000
===========================

(-) Goals
=========
                | Commited | Commited this period | Target |
Retirement fund | €15000   | … (€500)             | €60000 |
New iPhone      | €300     | ✅                   | €1000  |
Total           | €15300   | … (€2500)            | €61000 |

Remaining this period: €456
===========================
*/

use crate::remaining;
use crate::remaining::RemainingOperation;
use crate::vault::{VaultImpl, VaultReadable};
use clap::Parser;
use comfy_table::Table;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::HashMap;
use std::env::current_dir;
use std::fmt::{Display, Formatter};
use std::iter::once;
use std::path::PathBuf;
use serde_json::value::Index;

pub fn remaining_operation() {
    let result: Result<RemainingMoneyScreen, String> = (|| {
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

        Ok(screen.into())
    })();

    if let Ok(screen) = result {
        print!("{}", screen)
    } else if let Err(error) = result {
        println!("Could not compute remaining amount: {}", error)
    }
}

// TYPES AND ADAPTERS

type Figure = Decimal;
type Currency = String;
type ExchangeRate = (String, Decimal);
pub type ExchangeRates = HashMap<Currency, Figure>;

type PredictedIncome = Amount;
impl VaultReadable for PredictedIncome {
    const KEY: &'static str = "predicted_income";
}

#[derive(Deserialize)]
pub struct Amount {
    currency: Currency,
    figure: Figure,
}
impl Into<remaining::Amount> for Amount {
    fn into(self) -> remaining::Amount {
        remaining::Amount {
            currency: self.currency,
            figure: self.figure,
        }
    }
}

impl From<remaining::Amount> for Amount {
    fn from(value: remaining::Amount) -> Self {
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

trait WithCurrency {
    fn currency(&self) -> String;
    fn make_amount(&self, figure: &remaining::Figure) -> Amount {
        return Amount {
            currency: self.currency(),
            figure: figure.clone(),
        };
    }
}

struct DisplayAccount<'a> {
    account: &'a remaining::DisplayAccount,
}

impl<'a> WithCurrency for DisplayAccount<'a> {
    fn currency(&self) -> String {
        self.account.currency.clone()
    }
}

impl<'a> DisplayAccount<'a> {
    fn from(value: &'a remaining::DisplayAccount) -> DisplayAccount<'a> {
        DisplayAccount { account: value }
    }

    fn name(&self) -> String {
        self.account.name.clone()
    }

    fn period_start_balance(&self) -> Amount {
        self.make_amount(&self.account.period_start_balance)
    }

    fn current_balance(&self) -> Amount {
        self.make_amount(&self.account.current_balance)
    }

    fn difference(&self) -> Amount {
        self.make_amount(&self.account.difference)
    }
    
}

struct DisplayGoal<'a> {
    goal: &'a remaining::DisplayGoal,
}

impl<'a> WithCurrency for DisplayGoal<'a> {
    fn currency(&self) -> String {
        self.goal.currency.clone()
    }
}

impl<'a> DisplayGoal<'a> {
    fn from(value: &'a remaining::DisplayGoal) -> DisplayGoal {
        DisplayGoal { goal: value }
    }

    fn name(&self) -> String {
        self.goal.name.clone()
    }
    fn committed(&self) -> Amount {
        self.make_amount(&self.goal.commited)
    }

    fn to_commit_this_period(&self) -> Option<Amount> {
        self.goal
            .to_commit_this_period
            .map(|f| self.make_amount(&f))
    }

    fn target(&self) -> Amount {
        self.make_amount(&self.goal.target)
    }
}

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

// OUTPUT FORMATTING

struct RemainingMoneyScreen {
    screen: remaining::RemainingMoneyScreen,
}

impl From<remaining::RemainingMoneyScreen> for RemainingMoneyScreen {
    fn from(value: remaining::RemainingMoneyScreen) -> Self {
        RemainingMoneyScreen { screen: value }
    }
}

impl RemainingMoneyScreen {
    fn title(string: &str) -> String {
        let string_length = string.len();
        string.to_string() + "\n" + &"=".repeat(string_length)
    }

    fn formatted_period_start(&self) -> String {
        return Self::title(&format!("Period start: {}", self.screen.period_start));
    }

    fn formatted_account_table(&self) -> String {
        let accounts = once(&self.screen.overall_balance)
            .chain(self.screen.individual_balances.iter())
            .map(|account| DisplayAccount::from(account));

        let mut account_table = Table::new();
        account_table
            .set_header(once("".to_string()).chain(accounts.clone().map(|account| account.name())));
        account_table.add_row(
            once("Period start".to_string()).chain(
                accounts
                    .clone()
                    .map(|account| account.period_start_balance().to_string()),
            ),
        );
        account_table.add_row(
            once("Current".to_string()).chain(accounts.clone().map(|account| {
                format!("{} ({})", account.current_balance(), account.difference())
            })),
        );

        format!(
            "{}\n{}",
            RemainingMoneyScreen::title("Accounts".into()),
            account_table
        )
    }

    fn formatted_predicted_income(&self) -> String {
        let predicted_income: Option<String> = self
            .screen
            .predicted_income
            .clone()
            .map(|a| Amount::from(a).to_string());

        RemainingMoneyScreen::title(&format!(
            "(+) Predicted income: {}",
            predicted_income.unwrap_or("Not included".to_string())
        ))
    }

    fn formatted_goal_table(&self) -> String {
        let mut table = Table::new();
        table.set_header(["", "Commited", "Commited this period", "Target"]);

        let goals = once(&self.screen.overall_goal)
            .chain(self.screen.goals.iter())
            .map(|goal| DisplayGoal::from(goal));

        for goal in goals {
            table.add_row(vec![
                goal.name(),
                goal.committed().to_string(),
                match goal.to_commit_this_period() {
                    Some(amount) => amount.to_string(),
                    None => "✅".to_string(),
                },
                goal.target().to_string(),
            ]);
        }
        
        let mut formatted_uncommited = "(".to_string();
        formatted_uncommited.extend("Uncommitted:".chars());
        formatted_uncommited.extend(" ".chars());
        formatted_uncommited.extend(Amount::from(self.screen.uncommitted.clone()).to_string().chars());
        if self.screen.overcommitted {
            formatted_uncommited.extend(" ".chars());
            formatted_uncommited.extend("More money is committed to goals than is available in your accounts. Consider taking money out of one or more goals.".chars());
        }
        formatted_uncommited.extend(")".chars());

        format!(
            "{}\n{}\n{}",
            RemainingMoneyScreen::title("(-) Goals".into()),
            table,
            formatted_uncommited
        )
    }

    fn formatted_remaining(&self) -> String {
        RemainingMoneyScreen::title(&format!(
            "(=) Remaining this period: {}",
            Amount::from(self.screen.remaining.clone())
        ))
    }

    // TODO Move this to a more appropriate place once we have a better CLI
    fn format_release(&self) -> String {
        format!("Release: {}", env!("RELEASE"))
    }
}

impl Display for RemainingMoneyScreen {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}\n",
            [
                self.formatted_period_start(),
                self.formatted_account_table(),
                self.formatted_goal_table(),
                self.formatted_predicted_income(),
                self.formatted_remaining(),
                self.format_release()
            ]
            .join("\n\n")
            .as_str()
        )
    }
}
