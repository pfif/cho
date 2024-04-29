use chrono::{Local, NaiveDate};
use rand::Error;
use std::collections::HashMap;

use crate::accounts::{get_accounts, QueriableAccount, AccountJson};
use crate::goals::{Goal, GoalVaultValues, GoalImplementation};
use crate::period::{PeriodsConfiguration, PeriodVaultValues};
use crate::vault::Vault;
use mockall_double::double;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

//////////////////
// Public types //
//////////////////

type Figure = Decimal;
type Currency = String;
type ExchangeRates = HashMap<Currency, Figure>;

#[derive(Clone)]
#[cfg_attr(test, derive(Debug, Eq, PartialEq))]
pub struct Amount {
    currency: Currency,
    figure: Figure,
}

#[cfg_attr(test, derive(Default, Debug, PartialEq, Eq, Hash))]
pub struct DisplayAccount {
    name: String,
    period_start_balance: Figure,
    current_balance: Figure,
    difference: Figure,
    currency: Currency,
}

#[cfg_attr(test, derive(Default, Debug, PartialEq, Eq))]
pub struct DisplayGoal {
    name: String,
    commited: Figure,
    to_commit_this_period: Option<Figure>,
    target: Figure,
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

    remaining: Figure,
    currency: Currency,
}

pub struct PredictedIncome {
    amount: Figure,
    currency: Currency,
}

////////////////////
// Public methods //
////////////////////

pub struct RemainingOperation<A: QueriableAccount, G: Goal<P>, P: PeriodsConfiguration> {
    rates: ExchangeRates,
    target_currency: Currency,

    date: NaiveDate,
    periods_configuration: P,

    raw_accounts: Vec<A>,
    goals: Vec<G>,

    predicted_income: Option<Amount>,
}

impl RemainingOperation<AccountJson, GoalImplementation, PeriodVaultValues>{
    pub fn FromVaultValue(
        exchange_rate: ExchangeRates,
        target_currency: Currency,
        predicted_income: Option<Amount>,
        vault: Vault,
    ) -> Result<RemainingOperation<AccountJson, GoalImplementation, PeriodVaultValues>, String> {
        return Ok(RemainingOperation {
            rates: exchange_rate,
            target_currency,

            date: Local::now().date_naive(),
            periods_configuration: vault.read_periods_configuration()?,

            raw_accounts: vec![], // TODO get from Accounts
            goals: vault.read_goals()?.goals,

            predicted_income,
        });
    }
}

impl<A: QueriableAccount, G: Goal<P>, P: PeriodsConfiguration> RemainingOperation<A, G, P> {
    pub fn execute(self) -> Result<RemainingMoneyScreen, String> {
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
                (dec!(0) as Figure, dec!(0) as Figure),
                |(acc_period_start_balance, acc_current_balance),
                 account|
                 -> Result<(Figure, Figure), String> {
                    let mut period_start_balance = account.period_start_balance;
                    let mut current_balance = account.current_balance;

                    if account.currency != self.target_currency {
                        period_start_balance =
                            self.convert(&period_start_balance, &account.currency)?;
                        current_balance = self.convert(&current_balance, &account.currency)?;
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

        let goals = self
            .goals
            .iter()
            .map(|goal| {
                Ok(DisplayGoal {
                    name: goal.name().clone(),
                    commited: goal
                        .commited()
                        .iter()
                        .fold(0, |acc, (_, amount)| acc + amount)
                        .into(),
                    to_commit_this_period: match goal
                        .to_pay_at(&self.periods_configuration, &self.date)?
                    {
                        0 => None,
                        i => Some(i.into()),
                    },
                    currency: goal.currency().clone(),
                    target: Decimal::from(*goal.target()),
                })
            })
            .collect::<Result<Vec<DisplayGoal>, String>>()?;
        let overall_goal: DisplayGoal = goals.iter().try_fold(
            DisplayGoal {
                name: "Overall Goal".into(),
                currency: self.target_currency.clone(),
                target: dec!(0),
                commited: dec!(0),
                to_commit_this_period: None,
            },
            |acc, goal| -> Result<DisplayGoal, String> {
                let mut target = goal.target;
                let mut commited = goal.commited;
                let mut to_commit_this_period = goal.to_commit_this_period;

                if goal.currency != self.target_currency {
                    target = self.convert(&target, &goal.currency)?;
                    commited = self.convert(&commited, &goal.currency)?;
                    to_commit_this_period = match to_commit_this_period {
                        None => None,
                        Some(i) => Some(self.convert(&i, &goal.currency)?),
                    }
                }

                Ok(DisplayGoal {
                    target: acc.target + target,
                    commited: acc.commited + commited,
                    to_commit_this_period: match to_commit_this_period {
                        None => acc.to_commit_this_period,
                        Some(amount) => {
                            Some(acc.to_commit_this_period.unwrap_or(0.into()) + amount)
                        }
                    },
                    ..acc
                })
            },
        )?;

        let predicted_income_in_target_currency = {
            if let Some(predicted_income) = &self.predicted_income {
                if predicted_income.currency != self.target_currency {
                    Some(Amount {
                        currency: self.target_currency.to_string(),
                        figure: self
                            .convert(&predicted_income.figure, &predicted_income.currency)?,
                    })
                } else {
                    self.predicted_income.clone()
                }
            } else {
                self.predicted_income.clone()
            }
        };

        let remaining = overall_balance.current_balance
            + match &predicted_income_in_target_currency {
                None => dec!(0),
                Some(i) => i.figure,
            }
            - overall_goal.to_commit_this_period.unwrap_or(dec!(0));

        return Ok(RemainingMoneyScreen {
            period_start: current_period.start_date,

            overall_balance,
            individual_balances: accounts,

            predicted_income: predicted_income_in_target_currency,

            overall_goal,
            goals,

            remaining,
            currency: self.target_currency,
        });
    }

    fn convert(&self, amount: &Figure, from: &Currency) -> Result<Figure, String> {
        if from == &self.target_currency {
            return Err(
                "Attempt to convert from the target currency to the target currency".into(),
            );
        }
        let exchange_rate = {
            let target_currency_value = self.rate_for_currency(&self.target_currency)?;
            let from_currency_value = self.rate_for_currency(from)?;

            target_currency_value / from_currency_value
        };

        return Ok((amount * exchange_rate)
            .round_dp_with_strategy(2, rust_decimal::RoundingStrategy::MidpointNearestEven));
    }

    fn rate_for_currency(&self, name: &Currency) -> Result<Figure, String> {
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

#[cfg(test)]
mod tests_remaining_operation {
    use super::{
        Amount, Currency, DisplayAccount, DisplayGoal, Figure as RemainingFigure,
        RemainingOperation,
    };
    use crate::accounts::{Figure as AccountFigure, FoundAmount, MockQueriableAccount};
    use crate::goals::{Figure as GoalFigure, MockGoal};
    use crate::period::{MockPeriodsConfiguration, Period};
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
    #[builder(
        pattern = "immutable",
        build_fn(skip),
        setter(into),
        name = "MockQueriableAccountBuilder"
    )]
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
    struct MockGoalB {
        commited: Vec<(NaiveDate, GoalFigure)>,
        to_pay_at: GoalFigure,
        target: GoalFigure,
        currency: String,
    }

    impl MockGoalBuilder {
        fn build(&self) -> MockGoal<MockPeriodsConfiguration> {
            let mut mock = MockGoal::new();

            mock.expect_name().return_const("Mocked goal".into());
            mock.expect_currency()
                .return_const(self.currency.clone().unwrap());
            mock.expect_target().return_const(self.target.unwrap());
            mock.expect_commited()
                .return_const(self.commited.clone().unwrap());
            mock.expect_to_pay_at()
                .return_const(Ok(self.to_pay_at.unwrap()));
            return mock;
        }
    }

    fn defaultinstance() -> RemainingOperation<
        MockQueriableAccount,
        MockGoal<MockPeriodsConfiguration>,
        MockPeriodsConfiguration,
    > {
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

            predicted_income: None,
        }
    }

    /*
    TODO Make all tests use this table test
    Once this is done, I should be able to compare the result to a
    full RemainingMoneyScreen, instead of doing partial comparison as
    I do now
     */
    pub struct TestRunner<AccountGen: Fn(MockQueriableAccountBuilder) -> Vec<MockQueriableAccount>> {
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

        expected_remaining: RemainingFigure,
    }

    impl<AccountGen: Fn(MockQueriableAccountBuilder) -> Vec<MockQueriableAccount>>
        TestRunner<AccountGen>
    {
        pub fn test(self) {
            let account_builder = MockQueriableAccountBuilder::default()
                .period_start_date(self.period_start)
                .today_date(self.today);
            let instance = RemainingOperation {
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

            let result = instance.execute().unwrap();

            assert_eq!(
                result
                    .goals
                    .iter()
                    .map(|goal| goal.commited)
                    .collect::<Vec<RemainingFigure>>(),
                self.expected_commited
            );
            assert_eq!(result.overall_goal, self.expected_overall_goal);

            assert_eq!(result.remaining, self.expected_remaining);

            assert_eq!(result.predicted_income, self.expected_predicted_income)
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

        assert_eq!(result.remaining, dec!(6))
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
        );

        assert_eq!(result.remaining, dec!(14.40))
    }

    #[test]
    fn test__account_conversion__multiple_account() {
        let account_builder = MockQueriableAccountBuilder::default()
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
        );

        assert_eq!(result.remaining, dec!(16.40))
    }

    #[test]
    fn test__goal__different_currency() {
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
                .commited(vec![(mkdate(1), 2), (mkdate(1), 3)])
                .to_pay_at(5 as GoalFigure)
                .target(15 as GoalFigure)
                .currency("CREDIT")
                .build()],

            expected_commited: vec![5.into()],
            expected_overall_goal: DisplayGoal {
                name: "Overall Goal".into(),
                commited: 12.into(),
                to_commit_this_period: Some(12.into()),
                target: 36.into(),
                currency: "EUR".into(),
            },
            expected_remaining: dec!(-12),
        }
        .test();
    }

    #[test]
    fn test__goal__nothing_to_pay() {
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
                .commited(vec![(mkdate(1), 2), (mkdate(1), 3)])
                .to_pay_at(0 as GoalFigure)
                .target(15 as GoalFigure)
                .currency("EUR")
                .build()],

            expected_commited: vec![5.into()],
            expected_overall_goal: DisplayGoal {
                name: "Overall Goal".into(),
                commited: 5.into(),
                to_commit_this_period: None,
                target: 15.into(),
                currency: "EUR".into(),
            },
            expected_remaining: dec!(0),
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
                .to_pay_at(5 as GoalFigure)
                .target(15 as GoalFigure)
                .currency("EUR")
                .build()],

            expected_commited: vec![dec!(0)],
            expected_overall_goal: DisplayGoal {
                name: "Overall Goal".into(),
                commited: dec!(0),
                to_commit_this_period: Some(dec!(5)),
                target: dec!(15),
                currency: "EUR".to_string(),
            },
            expected_remaining: dec!(-5),
        }
        .test();
    }

    #[test]
    fn test__goal__multiple_goals__different_currencies() {
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

            goals: vec![
                MockGoalBuilder::default()
                    .commited(vec![(mkdate(1), 15), (mkdate(10), 20)])
                    .to_pay_at(5 as GoalFigure)
                    .target(15 as GoalFigure)
                    .currency("EUR")
                    .build(),
                MockGoalBuilder::default()
                    .commited(vec![(mkdate(3), 5), (mkdate(17), 5)])
                    .to_pay_at(10 as GoalFigure)
                    .target(1500 as GoalFigure)
                    .currency("CREDIT")
                    .build(),
            ],

            expected_commited: vec![dec!(35), dec!(10)],
            expected_overall_goal: DisplayGoal {
                name: "Overall Goal".into(),
                commited: dec!(59),
                to_commit_this_period: Some(dec!(29)),
                target: dec!(3615),
                currency: "EUR".to_string(),
            },
            expected_remaining: dec!(-29),
        }
        .test();
    }

    #[test]
    fn test__multiple_account__multiple_goal() {
        TestRunner {
            target_currency: "EUR".into(),

            rate_credit: 1.into(),
            rate_eur: dec!(2.4),

            period_start: mkdate(1),
            period_end: mkdate(31),
            today: mkdate(3),
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
                    .to_pay_at(5 as GoalFigure)
                    .target(15 as GoalFigure)
                    .currency("EUR")
                    .build(),
                MockGoalBuilder::default()
                    .commited(vec![(mkdate(3), 5), (mkdate(17), 5)])
                    .to_pay_at(10 as GoalFigure)
                    .target(1500 as GoalFigure)
                    .currency("CREDIT")
                    .build(),
            ],
            expected_commited: vec![dec!(35), dec!(10)],
            expected_overall_goal: DisplayGoal {
                name: "Overall Goal".into(),
                commited: dec!(59),
                to_commit_this_period: Some(dec!(29)),
                target: dec!(3615),
                currency: "EUR".to_string(),
            },
            expected_remaining: dec!(4682.2),
        }
        .test()
    }

    #[test]
    fn test__multiple_account__multiple_goal__predicted_income() {
        TestRunner {
            target_currency: "EUR".into(),

            rate_credit: 1.into(),
            rate_eur: dec!(2.4),

            period_start: mkdate(1),
            period_end: mkdate(31),
            today: mkdate(3),
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
                    .to_pay_at(5 as GoalFigure)
                    .target(15 as GoalFigure)
                    .currency("EUR")
                    .build(),
                MockGoalBuilder::default()
                    .commited(vec![(mkdate(3), 5), (mkdate(17), 5)])
                    .to_pay_at(10 as GoalFigure)
                    .target(1500 as GoalFigure)
                    .currency("CREDIT")
                    .build(),
            ],
            expected_commited: vec![dec!(35), dec!(10)],
            expected_overall_goal: DisplayGoal {
                name: "Overall Goal".into(),
                commited: dec!(59),
                to_commit_this_period: Some(dec!(29)),
                target: dec!(3615),
                currency: "EUR".to_string(),
            },
            expected_remaining: dec!(7562.2),
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
            expected_remaining: dec!(1200),

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
                commited: dec!(0),
                to_commit_this_period: None,
                target: dec!(0),
                currency: "EUR".to_string(),
            },
        }
        .test()
    }
}
