use chrono::{Local, NaiveDate};
use std::collections::HashMap;

use crate::accounts::{get_accounts, QueriableAccount};
use crate::goals::{Goal, GoalVaultValues};
#[double]
use crate::period::PeriodsConfiguration;
use mockall_double::double;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

//////////////////
// Public types //
//////////////////

type Amount = Decimal;
type Currency = String;
type ExchangeRates = HashMap<Currency, Amount>;

#[cfg_attr(test, derive(Default, Debug, PartialEq, Eq, Hash))]
pub struct DisplayAccount {
    name: String,
    period_start_balance: Amount,
    current_balance: Amount,
    difference: Amount,
    currency: Currency,
}

#[cfg_attr(test, derive(Default, Debug, PartialEq, Eq))]
pub struct DisplayGoal {
    name: String,
    commited: Amount,
    to_commit_this_period: Option<Amount>,
    target: Amount,
    currency: Currency,
}

#[cfg_attr(test, derive(Debug))]
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
    rates: ExchangeRates,
    target_currency: Currency,

    date: NaiveDate,
    periods_configuration: PeriodsConfiguration,

    raw_accounts: Vec<A>,
    goals: Vec<Goal>,

    predicted_income: Option<Amount>,
}

impl<A: QueriableAccount> RemainingOperation<A> {
    pub fn FromVaultValue(
        exchange_rate: ExchangeRates,
        target_currency: Currency,
        predicted_income: Option<Amount>,
    ) -> Result<RemainingOperation<A>, String> {
        return Ok(RemainingOperation {
            rates: exchange_rate,
            target_currency,

            date: Local::now().date_naive(),
            periods_configuration: PeriodsConfiguration::default(), // TODO get from Vault, remove the "default" implementation

            raw_accounts: vec![],
            goals: vec![],

            predicted_income: Some(dec!(0)),
        });
    }

    pub fn execute(&self) -> Result<RemainingMoneyScreen, String> {
        let current_period = self
            .periods_configuration
            .period_for_date(&self.date)
            .map_err(|error| "Failed to fetch Periods Configuration: ".to_string() + &error)?;

        let accounts = self
            .raw_accounts
            .iter()
            .map(|a| {
                return DisplayAccount::FromQueriableAccount(
                    a,
                    &current_period.start_date,
                    &self.date,
                );
            })
            .collect::<Result<Vec<DisplayAccount>, String>>()?;
        let overall_balance = {
            let (period_start_balance, current_balance) = accounts.iter().try_fold(
                (dec!(0) as Amount, dec!(0) as Amount),
                |(acc_period_start_balance, acc_current_balance),
                 account|
                 -> Result<(Amount, Amount), String> {
                    let mut period_start_balance = account.period_start_balance;
                    let mut current_balance = account.current_balance;

                    if account.currency != self.target_currency {
                        let exchange_rate = {
                            let target_currency_value =
                                self.rate_for_currency(&self.target_currency)?;
                            let account_currency_value =
                                self.rate_for_currency(&account.currency)?;

                            target_currency_value / account_currency_value
                        };

                        period_start_balance = (period_start_balance * exchange_rate).round_dp_with_strategy(2, rust_decimal::RoundingStrategy::MidpointNearestEven);
                        current_balance = (current_balance * exchange_rate).round_dp_with_strategy(2, rust_decimal::RoundingStrategy::MidpointNearestEven);
                    }

                    return Ok((
                        acc_period_start_balance + period_start_balance,
                        acc_current_balance + current_balance,
                    ));
                },
            )?;

            DisplayAccount::FromValues(
                "Overall Balance".into(),
                self.target_currency.clone(),
                period_start_balance,
                current_balance,
            )
        };

        let goals: Vec<DisplayGoal> = vec![]; // TODO turn goals in to display goals
        let overall_goal: DisplayGoal = DisplayGoal::default(); // TODO fold goals, add current_amount, convert to target_currency if need be

        let remaining = match self.predicted_income {
            Some(i) => i,
            None => dec!(0),
        } - overall_balance.difference
            - match overall_goal.to_commit_this_period {
                Some(i) => i,
                None => dec!(0),
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

    fn rate_for_currency(&self, name: &Currency) -> Result<Amount, String> {
        return self
            .rates
            .get(name)
            .ok_or(format!("Could not find currency for {}", name))
            .copied();
    }
}

impl DisplayAccount {
    fn FromQueriableAccount<A: QueriableAccount>(
        raw_account: &A,
        period_start_date: &NaiveDate,
        current_date: &NaiveDate,
    ) -> Result<Self, String> {
        let name = raw_account.name();
        let instance = (|| {
            let period_start_found_amount = raw_account.amount_at(period_start_date)?;
            let current_found_amount = raw_account.amount_at(current_date)?;
            Ok(DisplayAccount::FromValues(
                name.clone(),
                raw_account.currency().clone(),
                period_start_found_amount.figure.into(),
                current_found_amount.figure.into(),
            ))
        })()
        .map_err(|err: String| format!("Error when querying account \"{}\": {}", name, err));

        instance
    }

    fn FromValues(
        name: String,
        currency: Currency,
        period_start_balance: Amount,
        current_balance: Amount,
    ) -> Self {
        DisplayAccount {
            name,
            currency,
            period_start_balance,
            current_balance,
            difference: current_balance - period_start_balance,
        }
    }
}

#[cfg(test)]
mod tests_get_accounts {
    use super::{Amount as RemainingAmount, Currency, RemainingOperation};
    use crate::accounts::{Amount as AccountAmount, FoundAmount, MockQueriableAccount};
    use crate::period::{MockPeriodsConfiguration, Period};
    use crate::remaining::DisplayAccount;
    use chrono::NaiveDate;
    use derive_builder::Builder;
    use mockall::predicate::eq;
    use rust_decimal_macros::dec;
    use std::collections::HashMap;
    use std::collections::HashSet;

    fn mkdate(day: u32) -> NaiveDate {
        return NaiveDate::from_ymd_opt(2023, 12, day).unwrap();
    }

    fn mkperiodsconfig(
        start_date: &NaiveDate,
        end_date: &NaiveDate,
        today: &NaiveDate,
    ) -> MockPeriodsConfiguration {
        let mut periods_configuration = MockPeriodsConfiguration::new();

        periods_configuration
            .expect_period_for_date()
            .with(eq(today.clone()))
            .return_const(Ok(Period {
                start_date: start_date.clone(),
                end_date: end_date.clone(),
            }));

        periods_configuration
    }

    #[derive(Builder)]
    #[builder(pattern = "immutable", build_fn(skip), setter(into), name = "MockAccountBuilder")]
    struct MockAccount {
        today_date: NaiveDate,
        period_start_date: NaiveDate,

        name: String,
        currency: Currency,

        today_figure: AccountAmount,
        period_start_figure: AccountAmount,
    }

    impl MockAccountBuilder {
        fn build(&self) -> MockQueriableAccount {
            let mut raw_account = MockQueriableAccount::new();
            raw_account
                .expect_name()
                .return_const(self.name.clone().unwrap());
            raw_account
                .expect_currency()
                .return_const(self.currency.clone().unwrap());
            raw_account
                .expect_amount_at()
                .with(eq(self.today_date.unwrap()))
                .return_const(Ok(FoundAmount {
                    estimated: false,
                    figure: self.today_figure.unwrap(),
                }));
            raw_account
                .expect_amount_at()
                .with(eq(self.period_start_date.unwrap()))
                .return_const(Ok(FoundAmount {
                    estimated: false,
                    figure: self.period_start_figure.unwrap(),
                }));

            return raw_account;
        }
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
            rates: HashMap::from([("EUR".to_string(), dec!(1)), ("JPN".to_string(), dec!(2))]),
            target_currency: "EUR".to_string(),

            date: mkdate(3),
            periods_configuration: period_configuration,

            raw_accounts: vec![],
            goals: vec![],

            predicted_income: Some(dec!(0)),
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
    fn test_fails__period_config_initialization() {
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
    fn test__fails__account_parsing() {
        let mut raw_account = MockQueriableAccount::new();
        raw_account
            .expect_name()
            .return_const("Failing account".into());
        raw_account.expect_currency().return_const("EUR".into());
        raw_account
            .expect_amount_at()
            .with(eq(mkdate(1)))
            .return_const(Ok(FoundAmount {
                estimated: false,
                figure: 5,
            }));
        raw_account
            .expect_amount_at()
            .with(eq(mkdate(3)))
            .return_const(Err("some error".into()));

        let instance = RemainingOperation {
            periods_configuration: mkperiodsconfig(
                &mkdate(1), // Period start
                &mkdate(4),
                &mkdate(3), // Today
            ),
            raw_accounts: vec![raw_account],
            ..defaultinstance()
        };
        let result = instance.execute();
        assert_eq!(
            result.unwrap_err(),
            "Error when querying account \"Failing account\": some error".to_string()
        )
    }

    #[test]
    fn test__single_account__same_currency() {
        let instance = RemainingOperation {
            date: mkdate(3),
            periods_configuration: mkperiodsconfig(
                &mkdate(1), // Period start
                &mkdate(4),
                &mkdate(3), // Today
            ),

            target_currency: "CREDIT".to_string(),

            raw_accounts: vec![MockAccountBuilder::default()
                .period_start_date(mkdate(1))
                .today_date(mkdate(3))
                .name("Galactic bank")
                .currency("CREDIT")
                .period_start_figure(10 as u32)
                .today_figure(6 as u32)
                .build()],
            ..defaultinstance()
        };
        let result = instance.execute().unwrap();

        assert_eq!(
            result.individual_balances,
            vec![DisplayAccount {
                name: "Galactic bank".to_string(),
                period_start_balance: dec!(10),
                current_balance: dec!(6),
                difference: dec!(-4),
                currency: "CREDIT".to_string(),
            }]
        );

        assert_eq!(
            result.overall_balance,
            DisplayAccount {
                name: "Overall Balance".to_string(),
                period_start_balance: dec!(10),
                current_balance: dec!(6),
                difference: dec!(-4),
                currency: "CREDIT".to_string(),
            }
        );
    }

    #[test]
    fn test__single_account__different_currency() {
        let instance = RemainingOperation {
            date: mkdate(3),
            periods_configuration: mkperiodsconfig(
                &mkdate(1), // Period start
                &mkdate(4),
                &mkdate(3), // Today
            ),

            target_currency: "EUR".to_string(),
            rates: HashMap::from([
                ("CREDIT".to_string(), dec!(1.0)),
                ("EUR".to_string(), dec!(2.4)),
            ]),

            raw_accounts: vec![MockAccountBuilder::default()
                .period_start_date(mkdate(1))
                .today_date(mkdate(3))
                .name("Galactic bank")
                .currency("CREDIT")
                .period_start_figure(10 as u32)
                .today_figure(6 as u32)
                .build()],
            ..defaultinstance()
        };
        let result = instance.execute().unwrap();

        assert_eq!(
            result.individual_balances,
            vec![DisplayAccount {
                name: "Galactic bank".to_string(),
                period_start_balance: dec!(10),
                current_balance: dec!(6),
                difference: dec!(-4),
                currency: "CREDIT".to_string(),
            }]
        );

        assert_eq!(
            result.overall_balance,
            DisplayAccount {
                name: "Overall Balance".to_string(),
                period_start_balance: dec!(24.00),
                current_balance: dec!(14.40),
                difference: dec!(-9.60),
                currency: "EUR".to_string(),
            }
        )
    }

    #[test]
    fn test__account_conversion__multiple_account() {
        let account_builder = MockAccountBuilder::default()
            .period_start_date(mkdate(1))
            .today_date(mkdate(3));

        let instance = RemainingOperation {
            date: mkdate(3),
            periods_configuration: mkperiodsconfig(
                &mkdate(1), // Period start
                &mkdate(4),
                &mkdate(3), // Today
            ),

            target_currency: "EUR".to_string(),
            rates: HashMap::from([
                ("CREDIT".to_string(), dec!(1.0)),
                ("EUR".to_string(), dec!(2.4)),
            ]),

            raw_accounts: vec![
                account_builder
                    .name("Galactic bank")
                    .currency("CREDIT")
                    .period_start_figure(10 as u32)
                    .today_figure(6 as u32)
                    .build(),
                account_builder
                    .name("European bank")
                    .currency("EUR")
                    .period_start_figure(21 as u32)
                    .today_figure(2 as u32)
                    .build(),
            ],
            ..defaultinstance()
        };
        let result = instance.execute().unwrap();

        assert_eq!(
            HashSet::<DisplayAccount>::from_iter(result.individual_balances),
            HashSet::from_iter([
                DisplayAccount {
                    name: "Galactic bank".to_string(),
                    period_start_balance: dec!(10),
                    current_balance: dec!(6),
                    difference: dec!(-4),
                    currency: "CREDIT".to_string(),
                },
                DisplayAccount {
                    name: "European bank".to_string(),
                    currency: "EUR".to_string(),
                    period_start_balance: dec!(21),
                    current_balance: dec!(2),
                    difference: dec!(-19)
                }
            ])
        );

        assert_eq!(
            result.overall_balance,
            DisplayAccount {
                name: "Overall Balance".to_string(),
                currency: "EUR".to_string(),
                period_start_balance: dec!(45),
                current_balance: dec!(16.40),
                difference: dec!(-28.6)
            }
        )
    }

    #[test]
    fn test__goal_conversion__different_currency() {
        panic!("Next test to implement. Only test the overall DisplayGoal, the rest is just plumbing")
    }

    #[test]
    fn test__goal_conversion__multiple_goals() {
        panic!("Next test to implement. Only test the overall DisplayGoal, the rest is just plumbing")
    }
}
