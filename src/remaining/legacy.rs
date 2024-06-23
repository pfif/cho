use std::collections::HashMap;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::Deserialize;

use crate::accounts::QueriableAccount;
use crate::goals::Goal;
use crate::period::{Period, PeriodsConfiguration};
use crate::remaining::operations::RemainingOperation;

pub type Figure = Decimal;
pub type Currency = String;
pub type ExchangeRates = HashMap<Currency, Figure>;

#[derive(Clone, Deserialize)]
#[cfg_attr(test, derive(Debug, Eq, PartialEq))]
pub struct Amount {
    pub currency: Currency,
    pub figure: Figure,
}

#[cfg_attr(test, derive(Default, Debug, PartialEq, Eq, Hash))]
pub struct DisplayAccount {
    pub name: String,
    pub period_start_balance: Figure,
    pub current_balance: Figure,
    pub difference: Figure,
    pub currency: Currency,
}

impl DisplayAccount {
    fn from_queriable_account<A: QueriableAccount>(
        raw_account: &A,
        period_start_date: &NaiveDate,
        current_date: &NaiveDate,
    ) -> Result<Self, String> {
        let name = raw_account.name();
        let instance = (|| {
            let period_start_found_amount = raw_account.amount_at(period_start_date)?;
            let current_found_amount = raw_account.amount_at(current_date)?;
            Ok(DisplayAccount::from_values(
                name.clone(),
                raw_account.currency().clone(),
                period_start_found_amount.figure.into(),
                current_found_amount.figure.into(),
            ))
        })()
        .map_err(|err: String| format!("Error when querying account \"{}\": {}", name, err));

        instance
    }

    fn from_values(
        name: String,
        currency: Currency,
        period_start_balance: Figure,
        current_balance: Figure,
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

#[cfg_attr(test, derive(Default, Debug, PartialEq, Eq))]
pub struct DisplayGoal {
    pub name: String,
    pub committed: Figure,
    pub committed_this_period: Figure,
    pub to_commit_this_period: Option<Figure>,
    pub target: Figure,
    pub currency: Currency,
}

#[cfg_attr(test, derive(Debug))]
pub struct RemainingMoneyScreen {
    pub current_period: Period,

    pub overall_balance: DisplayAccount,
    pub individual_balances: Vec<DisplayAccount>,

    pub predicted_income: Option<Amount>,

    pub overall_goal: DisplayGoal,
    pub goals: Vec<DisplayGoal>,
    pub uncommitted: Amount,
    pub overcommitted: bool,

    pub remaining: Amount,
}

pub fn compute_legacy_remaining_screen<A: QueriableAccount, G: Goal<P>, P: PeriodsConfiguration>(
    operation: &RemainingOperation<A, G, P>,
) -> Result<RemainingMoneyScreen, String> {
    let current_period: Period = operation
        .periods_configuration
        .period_for_date(&operation.date)
        .map_err(|error| "Failed to fetch Periods Configuration: ".to_string() + &error)?
        .into();

    let accounts = operation
        .raw_accounts
        .iter()
        .map(|a| {
            return DisplayAccount::from_queriable_account(
                a,
                &current_period.start_date,
                &operation.date,
            );
        })
        .collect::<Result<Vec<DisplayAccount>, String>>()?;

    let overall_balance = {
        let (period_start_balance, current_balance) = accounts.iter().try_fold(
            (dec!(0) as Figure, dec!(0) as Figure),
            |(acc_period_start_balance, acc_current_balance),
             account|
             -> Result<(Figure, Figure), String> {
                let mut period_start_balance = account.period_start_balance;
                let mut current_balance = account.current_balance;

                if account.currency != operation.target_currency {
                    period_start_balance =
                        convert(operation, &period_start_balance, &account.currency)?;
                    current_balance = convert(operation, &current_balance, &account.currency)?;
                }

                return Ok((
                    acc_period_start_balance + period_start_balance,
                    acc_current_balance + current_balance,
                ));
            },
        )?;

        DisplayAccount::from_values(
            "Overall Balance".into(),
            operation.target_currency.clone(),
            period_start_balance,
            current_balance,
        )
    };

    let goals = operation
        .goals
        .iter()
        .map(|goal| {
            Ok(DisplayGoal {
                name: goal.name().clone(),
                committed: goal
                    .committed()
                    .iter()
                    .fold(0.into(), |acc, (_, amount)| acc + amount),
                committed_this_period: goal.committed().iter().fold(
                    0.into(),
                    |acc, (date, amount)| {
                        if date >= &current_period.start_date && date <= &current_period.end_date {
                            acc + amount
                        } else {
                            acc
                        }
                    },
                ),
                to_commit_this_period: {
                    let to_commit =
                        goal.to_pay_at(&operation.periods_configuration, &operation.date)?;
                    if to_commit == 0.into() {
                        None
                    } else {
                        Some(to_commit)
                    }
                },
                currency: goal.currency().clone(),
                target: Decimal::from(*goal.target()),
            })
        })
        .collect::<Result<Vec<DisplayGoal>, String>>()?;
    let overall_goal: DisplayGoal = goals.iter().try_fold(
        DisplayGoal {
            name: "Overall Goal".into(),
            currency: operation.target_currency.clone(),
            target: dec!(0),
            committed: dec!(0),
            committed_this_period: dec!(0),
            to_commit_this_period: None,
        },
        |acc, goal| -> Result<DisplayGoal, String> {
            let mut target = goal.target;
            let mut commited = goal.committed;
            let mut committed_this_period = goal.committed_this_period;
            let mut to_commit_this_period = goal.to_commit_this_period;

            if goal.currency != operation.target_currency {
                target = convert(operation, &target, &goal.currency)?;
                commited = convert(operation, &commited, &goal.currency)?;
                committed_this_period = convert(operation, &committed_this_period, &goal.currency)?;
                to_commit_this_period = match to_commit_this_period {
                    None => None,
                    Some(i) => Some(convert(operation, &i, &goal.currency)?),
                }
            }

            Ok(DisplayGoal {
                target: acc.target + target,
                committed: acc.committed + commited,
                to_commit_this_period: match to_commit_this_period {
                    None => acc.to_commit_this_period,
                    Some(amount) => Some(acc.to_commit_this_period.unwrap_or(0.into()) + amount),
                },
                committed_this_period: acc.committed_this_period + committed_this_period,
                ..acc
            })
        },
    )?;

    let predicted_income_in_target_currency = {
        if let Some(predicted_income) = &operation.predicted_income {
            if predicted_income.currency != operation.target_currency {
                Some(Amount {
                    currency: operation.target_currency.to_string(),
                    figure: convert(
                        operation,
                        &predicted_income.figure,
                        &predicted_income.currency,
                    )?,
                })
            } else {
                operation.predicted_income.clone()
            }
        } else {
            operation.predicted_income.clone()
        }
    };

    let remaining = match &predicted_income_in_target_currency {
        None => dec!(0),
        Some(i) => i.figure,
    } + overall_balance.difference
        - overall_goal.committed_this_period
        - overall_goal.to_commit_this_period.unwrap_or(dec!(0));

    let uncommitted = Amount {
        figure: overall_balance.current_balance - overall_goal.committed,
        currency: operation.target_currency.clone(),
    };
    let overcommitted = &uncommitted.figure < &(0.into());

    return Ok(RemainingMoneyScreen {
        current_period,

        overall_balance,
        individual_balances: accounts,

        predicted_income: predicted_income_in_target_currency,

        overall_goal,
        goals,
        uncommitted,
        overcommitted,

        remaining: Amount {
            figure: remaining,
            currency: operation.target_currency.clone(),
        },
    });
}

fn convert<A: QueriableAccount, G: Goal<P>, P: PeriodsConfiguration>(
    operation: &RemainingOperation<A, G, P>,
    amount: &Figure,
    from: &Currency,
) -> Result<Figure, String> {
    if from == &operation.target_currency {
        return Err("Attempt to convert from the target currency to the target currency".into());
    }
    let exchange_rate = {
        let target_currency_value = rate_for_currency(operation, &operation.target_currency)?;
        let from_currency_value = rate_for_currency(operation, from)?;

        target_currency_value / from_currency_value
    };

    return Ok((amount * exchange_rate)
        .round_dp_with_strategy(2, rust_decimal::RoundingStrategy::MidpointNearestEven));
}

fn rate_for_currency<A: QueriableAccount, G: Goal<P>, P: PeriodsConfiguration>(
    operation: &RemainingOperation<A, G, P>,
    name: &Currency,
) -> Result<Figure, String> {
    return operation
        .rates
        .get(name)
        .ok_or(format!("Could not find currency for {}", name))
        .copied();
}

#[cfg(test)]
mod tests_remaining_operation {
    use std::collections::HashMap;
    use std::collections::HashSet;

    use chrono::NaiveDate;
    use derive_builder::Builder;
    use mockall::predicate::eq;
    use rust_decimal_macros::dec;

    use crate::accounts::{Figure as AccountFigure, FoundAmount, MockQueriableAccount};
    use crate::goals::{Figure as GoalFigure, MockGoal};
    use crate::period;
    use crate::period::MockPeriodsConfiguration;
    use crate::period::Period;

    use super::{
        compute_legacy_remaining_screen, Amount, Currency, DisplayAccount, DisplayGoal,
        Figure as RemainingFigure, RemainingOperation,
    };

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
            .return_const(Ok(period::Period {
                start_date: start_date.clone(),
                end_date: end_date.clone(),
            }));

        periods_configuration
    }

    #[derive(Builder)]
    #[builder(
        pattern = "immutable",
        build_fn(skip),
        setter(into),
        name = "MockQueriableAccountBuilder"
    )]
    #[allow(unused)] // Builder is used, not class used to create Builder
    struct MockQueriableAccountB {
        today_date: NaiveDate,
        period_start_date: NaiveDate,

        name: String,
        currency: Currency,

        today_figure: AccountFigure,
        period_start_figure: AccountFigure,
    }

    impl MockQueriableAccountBuilder {
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

    #[derive(Builder)]
    #[builder(
        pattern = "immutable",
        build_fn(skip),
        setter(into),
        name = "MockGoalBuilder"
    )]
    #[allow(unused)] // MockGoalBuilder is used, not MockGoalB
    struct MockGoalB {
        commited: Vec<(NaiveDate, i32)>,
        to_pay_at: i32,
        target: i32,
        currency: String,
    }

    impl MockGoalBuilder {
        fn build(&self) -> MockGoal<MockPeriodsConfiguration> {
            let mut mock = MockGoal::new();

            mock.expect_name().return_const("Mocked goal".into());
            mock.expect_currency()
                .return_const(self.currency.clone().unwrap());
            mock.expect_target()
                .return_const(self.target.unwrap().into());
            mock.expect_committed().return_const(
                self.commited
                    .clone()
                    .unwrap()
                    .into_iter()
                    .map(|(date, figure)| (date, GoalFigure::from(figure)))
                    .collect(),
            );
            mock.expect_to_pay_at()
                .return_const(Ok(self.to_pay_at.unwrap().into()));
            return mock;
        }
    }

    fn defaultinstance() -> RemainingOperation<
        MockQueriableAccount,
        MockGoal<MockPeriodsConfiguration>,
        MockPeriodsConfiguration,
    > {
        let mut period_configuration = MockPeriodsConfiguration::new();

        let period = period::Period {
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

            predicted_income: None,
        }
    }

    /*
    TODO Make all tests use this table test
    Once this is done, I should be able to compare the result to a
    full RemainingMoneyScreen, instead of doing partial comparison as
    I do now
     */
    struct TestRunner<AccountGen: Fn(MockQueriableAccountBuilder) -> Vec<MockQueriableAccount>> {
        target_currency: String,

        rate_credit: RemainingFigure,
        rate_eur: RemainingFigure,

        period_start: NaiveDate,
        period_end: NaiveDate,
        today: NaiveDate,

        accounts: AccountGen,
        predicted_income: Option<Amount>,

        goals: Vec<MockGoal<MockPeriodsConfiguration>>,
        expected_commited: Vec<RemainingFigure>,
        expected_overall_goal: DisplayGoal,
        expected_predicted_income: Option<Amount>,
        expected_uncommitted: Amount,
        expected_overcommitted: bool,

        expected_remaining: Amount,
    }

    impl<AccountGen: Fn(MockQueriableAccountBuilder) -> Vec<MockQueriableAccount>>
        TestRunner<AccountGen>
    {
        pub fn test(self) {
            let account_builder = MockQueriableAccountBuilder::default()
                .period_start_date(self.period_start)
                .today_date(self.today);
            let operation = RemainingOperation {
                target_currency: self.target_currency,
                rates: HashMap::from([
                    ("CREDIT".to_string(), self.rate_credit),
                    ("EUR".to_string(), self.rate_eur),
                ]),
                periods_configuration: mkperiodsconfig(
                    &self.period_start,
                    &self.period_end,
                    &self.today,
                ),
                date: self.today,

                raw_accounts: (self.accounts)(account_builder),
                predicted_income: self.predicted_income,

                goals: self.goals,
            };

            let result = compute_legacy_remaining_screen(&operation).unwrap();

            assert_eq!(
                result
                    .goals
                    .iter()
                    .map(|goal| goal.committed)
                    .collect::<Vec<RemainingFigure>>(),
                self.expected_commited
            );
            assert_eq!(result.overall_goal, self.expected_overall_goal);

            assert_eq!(result.remaining, self.expected_remaining);

            assert_eq!(result.predicted_income, self.expected_predicted_income);

            assert_eq!(result.uncommitted, self.expected_uncommitted);

            assert_eq!(result.overcommitted, self.expected_overcommitted);
        }
    }

    #[test]
    fn test_period() {
        let today = mkdate(3);
        let periods_configuration = mkperiodsconfig(&mkdate(1), &mkdate(4), &today);

        let operation = RemainingOperation {
            date: today,
            periods_configuration,
            ..defaultinstance()
        };
        let result = compute_legacy_remaining_screen(&operation);

        assert_eq!(
            result.unwrap().current_period,
            Period {
                start_date: mkdate(1),
                end_date: mkdate(4)
            }
        )
    }

    #[test]
    fn test_fails__period_config_initialization() {
        let today = mkdate(3);
        let mut periods_configuration = MockPeriodsConfiguration::new();

        periods_configuration
            .expect_period_for_date()
            .with(eq(today))
            .return_const(Err("inner error".to_string()));

        let operation = RemainingOperation {
            date: today,
            periods_configuration,
            ..defaultinstance()
        };
        let result = compute_legacy_remaining_screen(&operation);

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

        let operation = RemainingOperation {
            periods_configuration: mkperiodsconfig(
                &mkdate(1), // Period start
                &mkdate(4),
                &mkdate(3), // Today
            ),
            raw_accounts: vec![raw_account],
            ..defaultinstance()
        };
        let result = compute_legacy_remaining_screen(&operation);
        assert_eq!(
            result.unwrap_err(),
            "Error when querying account \"Failing account\": some error".to_string()
        )
    }

    #[test]
    fn test__single_account__same_currency() {
        let operation = RemainingOperation {
            date: mkdate(3),
            periods_configuration: mkperiodsconfig(
                &mkdate(1), // Period start
                &mkdate(4),
                &mkdate(3), // Today
            ),

            target_currency: "CREDIT".to_string(),

            raw_accounts: vec![MockQueriableAccountBuilder::default()
                .period_start_date(mkdate(1))
                .today_date(mkdate(3))
                .name("Galactic bank")
                .currency("CREDIT")
                .period_start_figure(10 as u32)
                .today_figure(6 as u32)
                .build()],
            ..defaultinstance()
        };
        let result = compute_legacy_remaining_screen(&operation).unwrap();

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

        assert_eq!(
            result.remaining,
            Amount {
                figure: dec!(-4),
                currency: "CREDIT".to_string(),
            }
        )
    }

    #[test]
    fn test__single_account__different_currency() {
        let operation = RemainingOperation {
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

            raw_accounts: vec![MockQueriableAccountBuilder::default()
                .period_start_date(mkdate(1))
                .today_date(mkdate(3))
                .name("Galactic bank")
                .currency("CREDIT")
                .period_start_figure(10 as u32)
                .today_figure(6 as u32)
                .build()],
            ..defaultinstance()
        };
        let result = compute_legacy_remaining_screen(&operation).unwrap();

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
        );

        assert_eq!(
            result.remaining,
            Amount {
                figure: dec!(-9.60),
                currency: "EUR".to_string()
            }
        )
    }

    #[test]
    fn test__account_conversion__multiple_account() {
        let account_builder = MockQueriableAccountBuilder::default()
            .period_start_date(mkdate(1))
            .today_date(mkdate(3));

        let operation = RemainingOperation {
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
        let result = compute_legacy_remaining_screen(&operation).unwrap();

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
                    difference: dec!(-19),
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
                difference: dec!(-28.6),
            }
        );

        assert_eq!(
            result.remaining,
            Amount {
                figure: dec!(-28.6),
                currency: "EUR".to_string()
            }
        )
    }

    #[test]
    fn test__goal__different_currencies__committed_this_period() {
        TestRunner {
            target_currency: "EUR".into(),
            rate_credit: dec!(1.0),
            rate_eur: dec!(2.4),

            period_start: mkdate(16),
            period_end: mkdate(31),
            today: mkdate(17),
            accounts: |_| vec![],
            predicted_income: None,
            expected_predicted_income: None,

            goals: vec![MockGoalBuilder::default()
                .commited(
                    vec![
                        (mkdate(1), 2), // Outside of period
                        (mkdate(16), 3),
                    ], // In Period
                )
                .to_pay_at(0)
                .target(15)
                .currency("CREDIT")
                .build()],

            expected_commited: vec![5.into()],
            expected_overall_goal: DisplayGoal {
                name: "Overall Goal".into(),
                committed: 12.into(),
                committed_this_period: dec!(7.20),
                to_commit_this_period: None,
                target: 36.into(),
                currency: "EUR".into(),
            },
            expected_uncommitted: Amount {
                figure: (-12).into(),
                currency: "EUR".into(),
            },
            expected_overcommitted: true,
            expected_remaining: Amount {
                figure: dec!(-7.2),
                currency: "EUR".to_string(),
            },
        }
        .test();
    }

    #[test]
    fn test__goal__different_currencies__not_committed_this_period() {
        TestRunner {
            target_currency: "EUR".into(),
            rate_credit: dec!(1.0),
            rate_eur: dec!(2.4),

            period_start: mkdate(16),
            period_end: mkdate(31),
            today: mkdate(17),
            accounts: |_| vec![],
            predicted_income: None,
            expected_predicted_income: None,

            goals: vec![MockGoalBuilder::default()
                .commited(vec![(mkdate(1), 2), (mkdate(1), 3)])
                .to_pay_at(5)
                .target(15)
                .currency("CREDIT")
                .build()],

            expected_commited: vec![5.into()],
            expected_overall_goal: DisplayGoal {
                name: "Overall Goal".into(),
                committed: 12.into(),
                committed_this_period: 0.into(),
                to_commit_this_period: Some(12.into()),
                target: 36.into(),
                currency: "EUR".into(),
            },
            expected_uncommitted: Amount {
                figure: (-12).into(),
                currency: "EUR".into(),
            },
            expected_overcommitted: true,
            expected_remaining: Amount {
                figure: dec!(-12),
                currency: "EUR".to_string(),
            },
        }
        .test();
    }

    #[test]
    fn test__goal__nothing_to_pay__committed_this_period() {
        TestRunner {
            target_currency: "EUR".into(),
            rate_credit: dec!(1.0),
            rate_eur: dec!(2.4),

            period_start: mkdate(16),
            period_end: mkdate(31),
            today: mkdate(17),
            accounts: |_| vec![],
            predicted_income: None,
            expected_predicted_income: None,

            goals: vec![MockGoalBuilder::default()
                .commited(vec![(mkdate(1), 2), (mkdate(16), 3)])
                .to_pay_at(0)
                .target(15)
                .currency("EUR")
                .build()],

            expected_commited: vec![5.into()],
            expected_overall_goal: DisplayGoal {
                name: "Overall Goal".into(),
                committed: 5.into(),
                committed_this_period: 3.into(),
                to_commit_this_period: None,
                target: 15.into(),
                currency: "EUR".into(),
            },
            expected_uncommitted: Amount {
                figure: dec!(-5),
                currency: "EUR".to_string(),
            },
            expected_overcommitted: true,
            expected_remaining: Amount {
                figure: dec!(-3),
                currency: "EUR".to_string(),
            },
        }
        .test();
    }

    #[test]
    fn test__goal__committed_this_period__have_to_pay_more() {
        TestRunner {
            target_currency: "EUR".into(),
            rate_credit: dec!(1.0),
            rate_eur: dec!(2.4),

            period_start: mkdate(16),
            period_end: mkdate(31),
            today: mkdate(17),
            accounts: |_| vec![],
            predicted_income: None,
            expected_predicted_income: None,

            goals: vec![MockGoalBuilder::default()
                .commited(vec![(mkdate(1), 2), (mkdate(16), 3)])
                .to_pay_at(5)
                .target(15)
                .currency("EUR")
                .build()],

            expected_commited: vec![5.into()],
            expected_overall_goal: DisplayGoal {
                name: "Overall Goal".into(),
                committed: 5.into(),
                committed_this_period: 3.into(),
                to_commit_this_period: Some(5.into()),
                target: 15.into(),
                currency: "EUR".into(),
            },
            expected_uncommitted: Amount {
                figure: dec!(-5),
                currency: "EUR".to_string(),
            },
            expected_overcommitted: true,
            expected_remaining: Amount {
                figure: dec!(-8),
                currency: "EUR".to_string(),
            },
        }
        .test();
    }

    #[test]
    fn test__goal__nothing_to_pay__not_committed_this_period() {
        TestRunner {
            target_currency: "EUR".into(),
            rate_credit: dec!(1.0),
            rate_eur: dec!(2.4),

            period_start: mkdate(16),
            period_end: mkdate(31),
            today: mkdate(17),
            accounts: |_| vec![],
            predicted_income: None,
            expected_predicted_income: None,

            goals: vec![MockGoalBuilder::default()
                .commited(vec![(mkdate(1), 2), (mkdate(1), 3)])
                .to_pay_at(0)
                .target(15)
                .currency("EUR")
                .build()],

            expected_commited: vec![5.into()],
            expected_overall_goal: DisplayGoal {
                name: "Overall Goal".into(),
                committed: 5.into(),
                committed_this_period: 0.into(),
                to_commit_this_period: None,
                target: 15.into(),
                currency: "EUR".into(),
            },
            expected_uncommitted: Amount {
                figure: dec!(-5),
                currency: "EUR".to_string(),
            },
            expected_overcommitted: true,
            expected_remaining: Amount {
                figure: dec!(0),
                currency: "EUR".to_string(),
            },
        }
        .test();
    }

    #[test]
    fn test__goal__nothing_commited() {
        TestRunner {
            target_currency: "EUR".into(),
            rate_credit: dec!(1.0),
            rate_eur: dec!(2.4),

            period_start: mkdate(1),
            period_end: mkdate(31),
            today: mkdate(3),
            accounts: |_| vec![],
            predicted_income: None,
            expected_predicted_income: None,

            goals: vec![MockGoalBuilder::default()
                .commited(vec![])
                .to_pay_at(5)
                .target(15)
                .currency("EUR")
                .build()],

            expected_commited: vec![dec!(0)],
            expected_overall_goal: DisplayGoal {
                name: "Overall Goal".into(),
                committed: dec!(0),
                committed_this_period: 0.into(),
                to_commit_this_period: Some(dec!(5)),
                target: dec!(15),
                currency: "EUR".to_string(),
            },
            expected_uncommitted: Amount {
                figure: dec!(0),
                currency: "EUR".to_string(),
            },
            expected_overcommitted: false,
            expected_remaining: Amount {
                figure: dec!(-5),
                currency: "EUR".to_string(),
            },
        }
        .test();
    }

    #[test]
    fn test__goal__multiple_goals__different_currencies__committed_this_period() {
        TestRunner {
            target_currency: "EUR".into(),
            rate_credit: dec!(1.0),
            rate_eur: dec!(2.4),

            period_start: mkdate(16),
            period_end: mkdate(31),
            today: mkdate(17),
            accounts: |_| vec![],
            predicted_income: None,
            expected_predicted_income: None,

            goals: vec![
                MockGoalBuilder::default()
                    .commited(vec![(mkdate(1), 15), (mkdate(16), 20)])
                    .to_pay_at(0)
                    .target(15)
                    .currency("EUR")
                    .build(),
                MockGoalBuilder::default()
                    .commited(vec![(mkdate(3), 5), (mkdate(17), 5)])
                    .to_pay_at(0)
                    .target(1500)
                    .currency("CREDIT")
                    .build(),
            ],

            expected_commited: vec![dec!(35), dec!(10)],
            expected_overall_goal: DisplayGoal {
                name: "Overall Goal".into(),
                committed: dec!(59),
                committed_this_period: 32.into(),
                to_commit_this_period: None,
                target: dec!(3615),
                currency: "EUR".to_string(),
            },
            expected_uncommitted: Amount {
                figure: dec!(-59),
                currency: "EUR".to_string(),
            },
            expected_overcommitted: true,
            expected_remaining: Amount {
                figure: dec!(-32),
                currency: "EUR".to_string(),
            },
        }
        .test();
    }

    #[test]
    fn test__goal__multiple_goals__different_currencies__not_committed_this_period() {
        TestRunner {
            target_currency: "EUR".into(),
            rate_credit: dec!(1.0),
            rate_eur: dec!(2.4),

            period_start: mkdate(16),
            period_end: mkdate(31),
            today: mkdate(17),
            accounts: |_| vec![],
            predicted_income: None,
            expected_predicted_income: None,

            goals: vec![
                MockGoalBuilder::default()
                    .commited(vec![(mkdate(1), 15), (mkdate(10), 20)])
                    .to_pay_at(5)
                    .target(15)
                    .currency("EUR")
                    .build(),
                MockGoalBuilder::default()
                    .commited(vec![(mkdate(3), 5), (mkdate(13), 5)])
                    .to_pay_at(10)
                    .target(1500)
                    .currency("CREDIT")
                    .build(),
            ],

            expected_commited: vec![dec!(35), dec!(10)],
            expected_overall_goal: DisplayGoal {
                name: "Overall Goal".into(),
                committed: dec!(59),
                committed_this_period: 0.into(),
                to_commit_this_period: Some(dec!(29)),
                target: dec!(3615),
                currency: "EUR".to_string(),
            },
            expected_uncommitted: Amount {
                figure: dec!(-59),
                currency: "EUR".to_string(),
            },
            expected_overcommitted: true,
            expected_remaining: Amount {
                figure: dec!(-29),
                currency: "EUR".to_string(),
            },
        }
        .test();
    }

    #[test]
    fn test__multiple_account__multiple_goal() {
        TestRunner {
            target_currency: "EUR".into(),

            rate_credit: 1.into(),
            rate_eur: dec!(2.4),

            period_start: mkdate(16),
            period_end: mkdate(31),
            today: mkdate(17),
            predicted_income: None,
            expected_predicted_income: None,

            accounts: |account_builder| {
                return vec![
                    account_builder
                        .name("European bank")
                        .currency("EUR")
                        .today_figure(1500 as AccountFigure)
                        .period_start_figure(1630 as AccountFigure)
                        .build(),
                    account_builder
                        .name("Galactic bank")
                        .currency("CREDIT")
                        .today_figure(1338 as AccountFigure)
                        .period_start_figure(1400 as AccountFigure)
                        .build(),
                ];
            },

            goals: vec![
                MockGoalBuilder::default()
                    .commited(vec![(mkdate(1), 15), (mkdate(10), 20)])
                    .to_pay_at(5)
                    .target(15)
                    .currency("EUR")
                    .build(),
                MockGoalBuilder::default()
                    .commited(vec![(mkdate(3), 5), (mkdate(17), 5)])
                    .to_pay_at(0)
                    .target(1500)
                    .currency("CREDIT")
                    .build(),
            ],
            expected_commited: vec![dec!(35), dec!(10)],
            expected_overall_goal: DisplayGoal {
                name: "Overall Goal".into(),
                committed: dec!(59),
                committed_this_period: 12.into(),
                to_commit_this_period: Some(dec!(5)),
                target: dec!(3615),
                currency: "EUR".to_string(),
            },
            expected_uncommitted: Amount {
                figure: dec!(4652.2),
                currency: "EUR".to_string(),
            },
            expected_overcommitted: false,
            expected_remaining: Amount {
                figure: dec!(-295.8),
                currency: "EUR".to_string(),
            },
        }
        .test()
    }

    #[test]
    fn test__multiple_account__multiple_goal__predicted_income() {
        TestRunner {
            target_currency: "EUR".into(),

            rate_credit: 1.into(),
            rate_eur: dec!(2.4),

            period_start: mkdate(16),
            period_end: mkdate(31),
            today: mkdate(17),
            predicted_income: Some(Amount {
                currency: "CREDIT".into(),
                figure: 1200.into(),
            }),
            expected_predicted_income: Some(Amount {
                currency: "EUR".into(),
                figure: 2880.into(),
            }),

            accounts: |account_builder| {
                return vec![
                    account_builder
                        .name("European bank")
                        .currency("EUR")
                        .today_figure(1500 as AccountFigure)
                        .period_start_figure(1630 as AccountFigure)
                        .build(),
                    account_builder
                        .name("Galactic bank")
                        .currency("CREDIT")
                        .today_figure(1338 as AccountFigure)
                        .period_start_figure(1400 as AccountFigure)
                        .build(),
                ];
            },

            goals: vec![
                MockGoalBuilder::default()
                    .commited(vec![(mkdate(1), 15), (mkdate(10), 20)])
                    .to_pay_at(5)
                    .target(15)
                    .currency("EUR")
                    .build(),
                MockGoalBuilder::default()
                    .commited(vec![(mkdate(3), 5), (mkdate(17), 5)])
                    .to_pay_at(0)
                    .target(1500)
                    .currency("CREDIT")
                    .build(),
            ],
            expected_commited: vec![dec!(35), dec!(10)],
            expected_overall_goal: DisplayGoal {
                name: "Overall Goal".into(),
                committed: dec!(59),
                committed_this_period: 12.into(),
                to_commit_this_period: Some(dec!(5)),
                target: dec!(3615),
                currency: "EUR".to_string(),
            },
            expected_uncommitted: Amount {
                figure: dec!(4652.2),
                currency: "EUR".to_string(),
            },
            expected_overcommitted: false,
            expected_remaining: Amount {
                figure: dec!(2584.2),
                currency: "EUR".to_string(),
            },
        }
        .test()
    }

    #[test]
    fn test__predicted_amount__same_currency() {
        TestRunner {
            target_currency: "EUR".into(),
            predicted_income: Some(Amount {
                currency: "EUR".into(),
                figure: 1200.into(),
            }),

            expected_predicted_income: Some(Amount {
                currency: "EUR".into(),
                figure: 1200.into(),
            }),
            expected_remaining: Amount {
                figure: dec!(1200),
                currency: "EUR".to_string(),
            },

            rate_credit: 1.into(),
            rate_eur: dec!(2.4),
            period_start: mkdate(1),
            period_end: mkdate(31),
            today: mkdate(3),
            accounts: |_| vec![],
            goals: vec![],
            expected_commited: vec![],
            expected_overall_goal: DisplayGoal {
                name: "Overall Goal".into(),
                committed: dec!(0),
                committed_this_period: 0.into(),
                to_commit_this_period: None,
                target: dec!(0),
                currency: "EUR".to_string(),
            },
            expected_overcommitted: false,
            expected_uncommitted: Amount {
                figure: 0.into(),
                currency: "EUR".into(),
            },
        }
        .test()
    }
}
