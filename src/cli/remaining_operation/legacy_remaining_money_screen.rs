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

use std::fmt::{Display, Formatter};
use std::iter::once;

use comfy_table::Table;
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::period::Period;
use crate::remaining;

pub struct DisplayRemainingMoneyScreen {
    screen: remaining::RemainingMoneyScreen,
}

impl Display for DisplayRemainingMoneyScreen {
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

impl From<remaining::RemainingMoneyScreen> for DisplayRemainingMoneyScreen {
    fn from(value: remaining::RemainingMoneyScreen) -> Self {
        DisplayRemainingMoneyScreen { screen: value }
    }
}

impl DisplayRemainingMoneyScreen {
    fn title(string: &str) -> String {
        let string_length = string.len();
        string.to_string() + "\n" + &"=".repeat(string_length)
    }

    fn formatted_period_start(&self) -> String {
        return Self::title(&format!(
            "Current period : {}",
            DisplayPeriod::from(&self.screen.current_period)
        ));
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
            DisplayRemainingMoneyScreen::title("Accounts".into()),
            account_table
        )
    }

    fn formatted_predicted_income(&self) -> String {
        let predicted_income: Option<String> = self
            .screen
            .predicted_income
            .clone()
            .map(|a| Amount::from(a).to_string());

        DisplayRemainingMoneyScreen::title(&format!(
            "(+) Predicted income: {}",
            predicted_income.unwrap_or("Not included".to_string())
        ))
    }

    fn formatted_goal_table(&self) -> String {
        let mut table = Table::new();
        table.set_header([
            "",
            "Committed",
            "Committed this period",
            "To commit this period",
            "Target",
        ]);

        let goals = once(&self.screen.overall_goal)
            .chain(self.screen.goals.iter())
            .map(|goal| DisplayGoal::from(goal));

        for goal in goals {
            table.add_row(vec![
                goal.name(),
                goal.committed().to_string(),
                goal.committed_this_period().to_string(),
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
        formatted_uncommited.extend(
            Amount::from(self.screen.uncommitted.clone())
                .to_string()
                .chars(),
        );
        if self.screen.overcommitted {
            formatted_uncommited.extend(" ".chars());
            formatted_uncommited.extend("More money is committed to goals than is available in your accounts. Consider taking money out of one or more goals.".chars());
        }
        formatted_uncommited.extend(")".chars());

        format!(
            "{}\n{}\n{}",
            DisplayRemainingMoneyScreen::title("(-) Goals".into()),
            table,
            formatted_uncommited
        )
    }

    fn formatted_remaining(&self) -> String {
        DisplayRemainingMoneyScreen::title(&format!(
            "(=) Remaining this period: {}",
            Amount::from(self.screen.remaining.clone())
        ))
    }

    // TODO Move this to a more appropriate place once we have a better CLI
    fn format_release(&self) -> String {
        format!("Release: {}", env!("RELEASE"))
    }
}

type Figure = Decimal;
type Currency = String;

#[derive(Deserialize)]
pub struct Amount {
    currency: Currency,
    figure: Figure,
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
        self.make_amount(&self.goal.committed)
    }

    fn committed_this_period(&self) -> Amount {
        self.make_amount(&self.goal.committed_this_period)
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

trait WithCurrency {
    fn currency(&self) -> String;
    fn make_amount(&self, figure: &remaining::Figure) -> Amount {
        return Amount {
            currency: self.currency(),
            figure: figure.clone(),
        };
    }
}

struct DisplayPeriod<'a> {
    period: &'a Period,
}

impl<'a> DisplayPeriod<'a> {
    fn from(period: &'a Period) -> Self {
        DisplayPeriod { period }
    }
}

impl<'a> Display for DisplayPeriod<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} to {}", self.period.start_date, self.period.end_date)
    }
}
