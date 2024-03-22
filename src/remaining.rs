use chrono::{Local, NaiveDate};

use crate::accounts::{get_accounts, QueriableAccount};
use crate::goals::{Goal, GoalVaultValues};
#[double]
use crate::period::PeriodsConfiguration;
use mockall_double::double;

//////////////////
// Public types //
//////////////////

type Amount = u32;
type Currency = String;

#[derive(Default, Debug, PartialEq, Eq)]
pub struct DisplayAccount {
    name: String,
    period_start_balance: Amount,
    current_balance: Amount,
    difference: Amount,
    currency: Currency,
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct DisplayGoal {
    name: String,
    commited: Amount,
    to_commit_this_period: Option<Amount>,
    target: Amount,
    currency: Currency,
}

#[derive(Debug)]
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

pub struct PredictedIncome {
    amount: Amount,
    currency: Currency,
}

////////////////////
// Public methods //
////////////////////

pub struct RemainingOperation<A: QueriableAccount> {
    exchange_rate: ((Currency, f64), (Currency, f64)),
    target_currency: Currency,

    date: NaiveDate,
    periods_configuration: PeriodsConfiguration,

    raw_accounts: Vec<A>,
    goals: Vec<Goal>,

    predicted_income: Option<Amount>,
}

impl<A: QueriableAccount> RemainingOperation<A> {
    fn FromVaultValue(
        exchange_rate: ((Currency, f64), (Currency, f64)),
        target_currency: Currency,
        predicted_income: Option<Amount>,
    ) -> Result<RemainingOperation<A>, String> {
        return Ok(RemainingOperation {
            exchange_rate,
            target_currency,

            date: Local::now().date_naive(),
            periods_configuration: PeriodsConfiguration::default(), // TODO get from Vault, remove the "default" implementation

            raw_accounts: vec![],
            goals: vec![],

            predicted_income: Some(0),
        });
    }

    fn execute(&self) -> Result<RemainingMoneyScreen, String> {
        let current_period = self
            .periods_configuration
            .period_for_date(&self.date)
            .map_err(|error| "Failed to fetch Periods Configuration: ".to_string() + &error)?;

        let accounts: Vec<DisplayAccount> = vec![]; // TODO go over this with account for date
        let overall_balance = reduce_accounts(&accounts, &self.target_currency);

        let goals: Vec<DisplayGoal> = vec![]; // TODO turn goals in to display goals
        let overall_goal: DisplayGoal = DisplayGoal::default(); // TODO fold goals, add current_amount, convert to target_currency if need be

        let remaining = match self.predicted_income {
            Some(i) => i,
            None => 0,
        } - overall_balance.difference
            - match overall_goal.to_commit_this_period {
                Some(i) => i,
                None => 0,
            };

        return Ok(RemainingMoneyScreen {
            period_start: current_period.start_date,

            overall_balance,
            individual_balances: accounts,

            predicted_income: self.predicted_income,

            overall_goal,
            goals,

            remaining,
            currency: self.target_currency.clone(),
        });
    }
}

#[cfg(test)]
mod tests_get_accounts {
    use super::RemainingOperation;
    use crate::accounts::{FoundAmount, MockQueriableAccount};
    use crate::period::{MockPeriodsConfiguration, Period};
    use crate::remaining::DisplayAccount;
    use chrono::NaiveDate;
    use mockall::predicate::eq;

    fn mkdate(day: u32) -> NaiveDate {
        return NaiveDate::from_ymd_opt(2023, 12, day).unwrap();
    }

    fn mkperiodsconfig(start_date: &NaiveDate, end_date: &NaiveDate, today: &NaiveDate) -> MockPeriodsConfiguration {
        let mut periods_configuration = MockPeriodsConfiguration::new();

        periods_configuration
            .expect_period_for_date()
            .with(eq(today.clone()))
            .return_const(Ok(Period {start_date: start_date.clone(), end_date: end_date.clone()}));

        periods_configuration
    }

    fn defaultinstance() -> RemainingOperation<MockQueriableAccount> {
        let mut period_configuration = MockPeriodsConfiguration::new();

        let period = Period {
            start_date: mkdate(1),
            end_date: mkdate(4),
        };
        period_configuration
            .expect_period_for_date()
            .return_const(Ok(period));

        RemainingOperation {
            exchange_rate: (("EUR".to_string(), 1.), ("JPN".to_string(), 2.)),
            target_currency: "EUR".to_string(),

            date: mkdate(3),
            periods_configuration: period_configuration,

            raw_accounts: vec![],
            goals: vec![],

            predicted_income: Some(0),
        }
    }

    #[test]
    fn test_period_start() {
        let today = mkdate(3);
        let mut periods_configuration = mkperiodsconfig(&mkdate(1), &mkdate(4), &today);

        let instance = RemainingOperation {
            date: today,
            periods_configuration,
            ..defaultinstance()
        };
        let result = instance.execute();

        assert_eq!(result.unwrap().period_start, mkdate(1))
    }

    #[test]
    fn test_period_config_fails_initialization() {
        let today = mkdate(3);
        let mut periods_configuration = MockPeriodsConfiguration::new();

        periods_configuration
            .expect_period_for_date()
            .with(eq(today))
            .return_const(Err("inner error".to_string()));

        let instance = RemainingOperation {
            date: today,
            periods_configuration,
            ..defaultinstance()
        };
        let result = instance.execute();

        assert_eq!(
            result.unwrap_err(),
            "Failed to fetch Periods Configuration: inner error"
        )
    }

    #[test]
    fn test__account_conversion__same_currency(){
        let today = mkdate(3);
        let period_start = mkdate(1);

        let periodsconfig = mkperiodsconfig(&period_start, &mkdate(4), &today);

        let mut raw_account = MockQueriableAccount::new();
        raw_account.expect_name().return_const("Galactic bank".to_string());
        raw_account.expect_currency().return_const("CREDITS".to_string());
        raw_account.expect_amount_at().with(eq(today)).return_const(Ok(FoundAmount{
            estimated: false,
            figure: 6
        }));

        raw_account.expect_amount_at().with(eq(period_start)).return_const(Ok(FoundAmount{
            estimated: false,
            figure: 10
        }));

        let instance = RemainingOperation {
            date: today,
            raw_accounts: vec![raw_account],
            target_currency: "CREDITS".to_string(),
            periods_configuration: periodsconfig,
            ..defaultinstance()
        };
        let result = instance.execute();

        assert_eq!(
            result.unwrap().individual_balances,
            vec![
                DisplayAccount{
                    name: "Galactic bank".to_string(),
                    period_start_balance: 10,
                    current_balance: 6,
                    difference: 4,
                    currency: "CREDITS".to_string(),
                }
            ]
        )
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
