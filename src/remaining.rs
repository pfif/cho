/*use chrono::{Local, NaiveDate};

use crate::accounts::{get_accounts, QueriableAccount};
// use crate::goals::{GoalVaultValues, Goal};
use crate::period::{PeriodsConfiguration};

type Amount = u32;

pub trait AccountBalance {
    fn period_start_balance() -> Amount;
    fn current_balance() -> Amount;
    fn difference() -> Amount {
        //TODO implement
    }
    fn currency() -> String;
}

pub struct RemainingMoneyScreen {
    overall_balance: AccountSums,
    predicted_income: Option<Amount>,
//    overall_goal: Amount,
//    goals: Vec<Goals>,
    remaining: Amount,
}

pub trait RemainingVaultParameters {
    fn predicted_income() -> Amount;
}

pub fn remaining_money(
    exchange_rate: ((String, f64), (String, f64)),
    target_currency: String,
    vault_values: dyn RemainingVaultParameters,
//    goals_vault_values: dyn GoalVaultValues,
    period_config: PeriodsConfiguration,
    include_predicted_income: bool,
) -> Result<RemainingMoneyScreen, String> {
    let date = Local::now().date_naive();
    let accounts = get_accounts().map(|acc| {
        return AccountForDate {
            account: acc,
            current_date: date,
        };
    });

    let overall_balance = AccountSums::reduce_account(accounts);
//    let goals = get_goals(goals_vault_values); // TODO do conversion
//    let overall_goal: Amount; // TODO fold goals, add current_amount, convert to target_currency if need be

    let predicted_income = vault_values.predicted_income();

    let remaining = if include_predicted_income {
        predicted_income
    } else {
        0
    } - overall_balance.difference();
//        - overall_goal;

    return Ok(RemainingMoneyScreen {
        overall_balance,
//        overall_goal,
        predicted_income: if include_predicted_income {
            predicted_income
        } else {
            None
        },
//        goals,
        remaining,
    });
}

struct AccountSums {
    period_start_balance: Amount,
    current_balance: Amount,
    currency: String,
}

impl AccountSums {
    fn reduce_accounts(accounts: Vec<AccountForDate>, target_currency: String) -> AccountSums {
        // TODO actually this can be implemented with Fold
    }

    // TODO Implement
}

struct AccountForDate<T: QueriableAccount> {
    account: T,
    current_date: NaiveDate,
}

impl<T: QueriableAccount> AccountBalance for AccountForDate<T> {
    // TODO implement
}*/
