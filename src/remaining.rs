use chrono::{Local, NaiveDate};

use crate::accounts::{get_accounts, QueriableAccount};
use crate::goals::{GoalVaultValues, Goal};
use mockall_double::double;
#[double]
use crate::period::{PeriodsConfiguration};

//////////////////
// Public types //
//////////////////

type Amount = u32;
type Currency = String;

#[derive(Default)]
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

#[derive(Default)]
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

////////////////////
// Public methods //
////////////////////

/* pub fn remaining_money(
    exchange_rate: ((Currency, f64), (Currency, f64)),
    target_currency: Currency,
    predicted_income: Option<Amount>,
) -> Result<RemainingMoneyScreen, String> {
    let date = Local::now().date_naive();
    let accounts = get_accounts();
    let goals: Vec<Goal>; // TODO Implement a function to get goals from vault
    let period_configuration: PeriodsConfiguration; // TODO implement a function to get 
    return _remaining_money(date, accounts, goals)
}*/

fn _remaining_money<A: QueriableAccount>(
    exchange_rate: ((Currency, f64), (Currency, f64)),
    target_currency: Currency,

    date: &NaiveDate,
    period_configuration: &PeriodsConfiguration,

    raw_accounts: Vec<A>,
    goals: Vec<Goal>,

    predicted_income: Option<Amount>,
) -> Result<RemainingMoneyScreen, String> {
    let current_period = period_configuration.period_for_date(date)?;

    let accounts: Vec<DisplayAccount> = vec![]; // TODO go over this with account for date
    let overall_balance = reduce_accounts(&accounts, &target_currency);

    let goals: Vec<DisplayGoal> = vec![]; // TODO turn goals in to display goals
    let overall_goal: DisplayGoal = DisplayGoal::default(); // TODO fold goals, add current_amount, convert to target_currency if need be

    let remaining = match predicted_income {
        Some(i) => i,
        None => 0
    } - overall_balance.difference()
      - match overall_goal.to_commit_this_period {
        Some(i) => i,
        None => 0
    };

    return Ok(RemainingMoneyScreen {
        period_start: current_period.start_date,

        overall_balance,
        individual_balances: accounts,

        predicted_income,

        overall_goal,
        goals,

        remaining,
        currency: target_currency,
    });
}

#[cfg(test)]
mod tests_get_accounts {
    use super::{_remaining_money};
    use chrono::NaiveDate;
    use crate::period::{MockPeriodsConfiguration, Period};
    use mockall::{Predicate};
    use mockall::predicate::{eq};

    fn mkdate(day: u32) -> NaiveDate{
        return NaiveDate::from_ymd_opt(2023, 12, day).unwrap();
    }

    macro_rules! invoke {
        ($($name:ident=$value:expr);*) => {
            {
                let mut exchange_rate = (("EUR".to_string(), 1.), ("JPN".to_string(), 2.));
                let mut target_currency = "EUR".to_string();

                let mut date = mkdate(3);
                let mut period_configuration = MockPeriodsConfiguration::new();
                let period = Period{
                    start_date: mkdate(1),
                    end_date: mkdate(4)
                };
                period_configuration.expect_period_for_date().return_const(Ok(period));

                let mut raw_accounts = vec![];
                let mut goals = vec![];

                let mut predicted_income = Some(0);

                $(
                    $name = $value;
                )*

                let result = _remaining_money(
                    exchange_rate,
                    target_currency,

                    &date,
                    &period_configuration,

                    raw_accounts,
                    goals,

                    predicted_income,
                );

                result
            }
        }
    }

    fn test_period_start() {
        let today = mkdate(3);
        let mut periods_config = MockPeriodsConfiguration::new();

        let period = Period{
            start_date: mkdate(1),
            end_date: mkdate(4)
        };

        periods_config.expect_period_for_date()
            .with(eq(today))
            .return_const(Ok(period));

        let result = invoke!{date=today;period_configuration=periods_config};


    }

}

/////////////////////
// Private methods //
/////////////////////

fn reduce_accounts(accounts: &Vec<DisplayAccount>, target_currency: &Currency) -> DisplayAccount {
    DisplayAccount::default()
}

fn account_for_date<A: QueriableAccount>(account: A, current_date: NaiveDate) -> DisplayAccount {
    DisplayAccount::default()
}
