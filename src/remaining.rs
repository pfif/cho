use chrono::{Local, NaiveDate};

use crate::accounts::{get_accounts, QueriableAccount};
use crate::goals::{GoalVaultValues, Goal};
use crate::period::{PeriodsConfiguration};

type Amount = u32;
type Currency = String;

/*
TODO Design a sample output here
*/

pub struct DisplayAccount {
    period_start_balance: Amount,
    current_balance: Amount,
    currency: Currency,
}

pub struct RemainingMoneyScreen {
    overall_balance: DisplayAccount,
    individual_balance: DisplayAccount,
    predicted_income: Option<Amount>,
    overall_goal: Amount,
    goals: Vec<Goal>,
    remaining: Amount,
}

pub trait RemainingVaultParameters {
    fn predicted_income() -> Amount;
}

pub fn remaining_money(
    exchange_rate: ((Currency, f64), (Currency, f64)),
    target_currency: Currency,
    period_config: PeriodsConfiguration,
    include_predicted_income: bool,
) -> Result<RemainingMoneyScreen, String> {
    let date = Local::now().date_naive();
    let accounts = get_accounts();
    let goals; // TODO do conversion
    return _remaining_money(date, accounts, goals)
}

fn _remaining_money<A: QueriableAccount>(
    date: NaiveDate,
    target_currency: Currency,
    raw_accounts: Vec<A>,
    goals: Vec<Goal>
) -> Result<RemainingMoneyScreen, String> {
    let accounts: Vec<DisplayAccount>;
    let overall_balance = reduce_accounts(accounts, target_currency);
    let overall_goal: Amount; // TODO fold goals, add current_amount, convert to target_currency if need be

    let predicted_income = vault_values.predicted_income();

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

}
