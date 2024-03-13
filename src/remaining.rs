use chrono::{Local, NaiveDate};

use crate::accounts::{get_accounts, QueriableAccount};
use crate::goals::{GoalVaultValues, Goal};
use crate::period::{PeriodsConfiguration};

//////////////////
// Public types //
//////////////////

type Amount = u32;
type Currency = String;

pub struct DisplayAccount {
    period_start_balance: Amount,
    current_balance: Amount,
    currency: Currency,
}

impl DisplayAccount {
    fn difference(&self) -> Amount {
        return self.period_start_balance - self.current_balance
    }
}

pub struct DisplayGoal {
    name: String,
    commited: Amount,
    to_commit_this_period: Option<Amount>,
    target: Amount,
    currency: Currency
}

pub struct RemainingMoneyScreen {
    period_start: NaiveDate,

    overall_balance: DisplayAccount,
    individual_balances: Vec<DisplayAccount>,

    predicted_income: Option<Amount>,

    overall_goal: DisplayGoal,
    goals: Vec<DisplayGoal>,

    remaining: Amount,
    currency: Currency,
}

pub struct PredictedIncome{
    amount: Amount,
    currency: Currency,
}

pub struct RemainingVaultParameters {
    fn predicted_income() -> Amount;
}

////////////////////
// Public methods //
////////////////////

pub fn remaining_money(
    exchange_rate: ((Currency, f64), (Currency, f64)),
    target_currency: Currency,
) -> Result<RemainingMoneyScreen, String> {
    let date = Local::now().date_naive();
    let accounts = get_accounts();
    let goals; // TODO Implement a function to get goals from vault
    let predicted_income; // TODO Implement to get pred inc. from vault
    let predicted_income_included;
    return _remaining_money(date, accounts, goals)
}

fn _remaining_money<A: QueriableAccount>(
    exchange_rate: ((Currency, f64), (Currency, f64)),
    target_currency: Currency,

    date: NaiveDate,

    raw_accounts: Vec<A>,
    goals: Vec<Goal>,

    predicted_income: Amount,
    predicted_income_included: Bool,
) -> Result<RemainingMoneyScreen, String> {
    let accounts: Vec<DisplayAccount>;
    let overall_balance = reduce_accounts(accounts, target_currency);
    let overall_goal: Amount; // TODO fold goals, add current_amount, convert to target_currency if need be

    let remaining = if include_predicted_income {
        predicted_income
    } else {
        0
    } - overall_balance.difference() - overall_goal;

    return Ok(RemainingMoneyScreen {
        overall_balance,
        overall_goal,
        predicted_income: if include_predicted_income {
            predicted_income
        } else {
            None
        },
        goals,
        remaining,
    });
}

fn reduce_accounts(accounts: Vec<DisplayAccount>, target_currency: Currency) -> DisplayAccount {
    // TODO actually this can be implemented with Fold
}

fn account_for_date<A: QueriableAccount>(account: A, current_date: NaiveDate) -> DisplayAccount {
    // TODO
}
