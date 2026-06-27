pub mod aggregated_amounts;

use crate::amounts::exchange_rates::ExchangeRates;
use crate::amounts::{Amount, Figure, RawAmount};
use crate::buckets::aggregated_amounts::AggregatedAmounts;
use crate::chrono_stack::ChronoStack;
use crate::line::LineWithDateVisitor;
use crate::period::{
    ErrorPeriodsBetween, Period, PeriodConfigurationVaultValue, PeriodsConfiguration,
};
use crate::remaining_operation::core_types::{
    GroupBuilder, IllustrationValue, Operand, OperandBuilder,
};
use crate::vault::VaultReadable;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::de::{Error, Visitor};
use serde::{Deserialize, Deserializer};
use std::cmp::max;
use std::fmt::Formatter;
use std::str::{Split, SplitWhitespace};

pub type BucketsVaultValue = Vec<Bucket>;
impl VaultReadable for BucketsVaultValue {
    const KEY: &'static str = "buckets";
}

impl GroupBuilder<Bucket> for BucketsVaultValue {
    fn build(self) -> Result<(String, Vec<Bucket>), String> {
        Ok(("Buckets".into(), self.into_iter().collect()))
    }
}

#[derive(Deserialize, Debug, Eq, PartialEq)]
pub struct Bucket {
    name: String,
    lines: Vec<Line>,
    #[serde(default)]
    archived_since: Option<NaiveDate>,
}

#[derive(Debug, Eq, PartialEq, Clone)]
struct Line((NaiveDate, Action));

impl<'de> Deserialize<'de> for Line {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ActionVisitor;
        impl ActionVisitor {
            fn parse_amount<E: Error>(line: &mut SplitWhitespace) -> Result<RawAmount, E> {
                let raw_amount_str = line.next().ok_or(Error::custom("No amounts specified"))?;

                let raw_amount_str_itr: &str = raw_amount_str.into();

                Ok(RawAmount::try_from(raw_amount_str_itr).map_err(|s| {
                    Error::custom(format!(
                        "Failed to parse amount: {}. Error: {}",
                        raw_amount_str_itr, s
                    ))
                })?)
            }
        }

        impl<'de> Visitor<'de> for ActionVisitor {
            type Value = Action;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("an action")
            }

            fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
                let mut line = v.split_whitespace();
                let tag = line.next().ok_or(Error::custom("No tag specified"))?;
                let action = match tag {
                    "TARG" => {
                        let raw_amount = ActionVisitor::parse_amount(&mut line)?;

                        let raw_target_date = line
                            .next()
                            .ok_or(Error::custom("No target date specified"))?;
                        let target_date = NaiveDate::parse_from_str(raw_target_date, "%Y/%m/%d")
                            .map_err(|err| {
                                Error::custom(format!(
                                    "Failed to parse date: {}. Error: {}",
                                    raw_target_date, err
                                ))
                            })?;

                        Ok(Action::SetTarget {
                            amount: raw_amount,
                            target_date,
                        })
                    }
                    "DEPO" => Ok(Action::Deposit(ActionVisitor::parse_amount(&mut line)?)),
                    "DEPO-" => Ok(Action::DepositCancellation(ActionVisitor::parse_amount(
                        &mut line,
                    )?)),
                    "WITH" => Ok(Action::Withdrawal(ActionVisitor::parse_amount(&mut line)?)),
                    "WITH-" => Ok(Action::WithdrawalCancellation(ActionVisitor::parse_amount(
                        &mut line,
                    )?)),
                    _ => Err(Error::custom("Unknown tag")),
                }?;

                Ok(action)
            }
        }

        let line_visitor = LineWithDateVisitor::new(ActionVisitor);

        Ok(Line(deserializer.deserialize_str(line_visitor)?))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Action {
    Deposit(RawAmount),
    DepositCancellation(RawAmount),
    Withdrawal(RawAmount),
    WithdrawalCancellation(RawAmount),
    SetTarget {
        amount: RawAmount,
        target_date: NaiveDate,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub struct BucketAtDate {
    recommended_or_actual_change: Amount,
    current_recommended_deposit: Option<Amount>,
    current_actual_deposit: Option<Amount>,
    current_withdrawal: Option<Amount>,
    total_deposit: Amount,
    total_withdrawal: Amount,
    total: Amount,
}

impl Bucket {
    fn for_period<P: PeriodsConfiguration>(
        &self,
        period_config: &P,
        date: &NaiveDate,
        ex: &ExchangeRates,
    ) -> Result<BucketAtDate, String> {
        let stack = ChronoStack::new(
            &self
                .lines
                .clone()
                .into_iter()
                .map(|line| line.0)
                .collect::<Vec<_>>(),
        )?;
        let period = period_config.period_for_date(date)?;

        let (actions_before_period, actions_in_period_before_date, actions_after_date) =
            stack.iters_for_periods_and_date(&period, date);

        let mut aggregated_amounts = AggregatedAmounts::new(ex)?;

        actions_before_period
            .clone()
            .try_for_each(|action| aggregated_amounts.apply(action))?;
        let aggregated_amounts_before_period = aggregated_amounts.clone();

        actions_in_period_before_date
            .clone()
            .try_for_each(|action| aggregated_amounts.apply(&action))?;
        let aggregated_amounts_until_date = aggregated_amounts.clone();

        actions_after_date.clone().try_for_each(|action| aggregated_amounts.apply(&action))?;

        let seen_deposit_this_period =
            actions_in_period_before_date
                .clone()
                .any(|action| match action {
                    Action::Deposit(_) | Action::DepositCancellation(_) => true,
                    _ => false,
                });

        let seen_withdrawal_this_period = actions_in_period_before_date
            .clone()
            .any(|action| match action {
                Action::Withdrawal(_) | Action::WithdrawalCancellation(_) => true,
                _ => false,
            });

        let aggregated_amounts_for_this_period =
            aggregated_amounts_until_date.clone() - aggregated_amounts_before_period.clone();

        let deposit_this_period = if seen_deposit_this_period {
            Some(aggregated_amounts_for_this_period.deposited())
        } else {
            if aggregated_amounts_for_this_period.deposited() != ex.zero(&"JPY".to_string())? {
                return Err("No deposit in this period, but deposit is not zero".to_string());
            }
            None
        };

        let withdrawal_this_period = if seen_withdrawal_this_period {
            Some(aggregated_amounts_for_this_period.withdrawn())
        } else {
            if aggregated_amounts_for_this_period.withdrawn() != ex.zero(&"JPY".to_string())? {
                return Err("No withdrawal in this period, but withdrawl is not zero".to_string());
            }
            None
        };

        let total_this_period = if seen_withdrawal_this_period || seen_deposit_this_period {
            Some(aggregated_amounts_for_this_period.total())
        } else {
            if aggregated_amounts_for_this_period.total() != ex.zero(&"JPY".to_string())? {
                return Err(
                    "No withdrawal or deposit in this period, but total is not zero".to_string(),
                );
            }
            None
        };

        let target = actions_before_period
            .chain(actions_in_period_before_date)
            .filter_map(|action| match action {
                    Action::SetTarget {
                    amount,
                    target_date,
                } => Some(ex.new_amount_from_raw_amount(amount).map(|amount| (amount, target_date.clone()))),
                _ => None,
            })
            .collect::<Result<Vec<(Amount, NaiveDate)>, String>>()?
            .into_iter()
            .last();

        let recommended_deposit_figure = if let Some((target_amount, target_date)) = target {
            let number_of_periods = match period_config.periods_between_nb(date, &target_date) {
                Ok(nb) => nb,
                Err(ErrorPeriodsBetween::EndBeforeStart) => 1,
                any => any?,
            };

            let recommended_deposit_figure = max(
                (target_amount - aggregated_amounts_before_period.deposited()),
                ex.zero(&"JPY".to_string())?,
            ) / Decimal::from(number_of_periods);

            Some(recommended_deposit_figure)
        } else {
            None
        };

        Ok(BucketAtDate {
            recommended_or_actual_change: total_this_period.clone().unwrap_or(
                recommended_deposit_figure
                    .clone()
                    .unwrap_or(ex.zero(&"JPY".to_string())?),
            ),
            current_recommended_deposit: recommended_deposit_figure,
            current_actual_deposit: deposit_this_period,
            current_withdrawal: withdrawal_this_period,
            total_deposit: aggregated_amounts_until_date.deposited(),
            total_withdrawal: aggregated_amounts_until_date.withdrawn(),
            total: aggregated_amounts_until_date.total(),
        })
    }
}

impl OperandBuilder for Bucket {
    fn build<P: PeriodsConfiguration>(
        self,
        period_configuration: &P,
        today: &NaiveDate,
        exchange_rates: &ExchangeRates,
    ) -> Result<Vec<Operand>, String> {
        let period = self.for_period(period_configuration, today, exchange_rates)?;
        Ok(vec![Operand {
            name: self.name,
            amount: period.recommended_or_actual_change.flip_sign(),
            illustration: vec![
                (
                    "This period - recommended deposit".to_string(),
                    period.current_recommended_deposit.into(),
                ),
                (
                    "This period - actual deposit".to_string(),
                    period.current_actual_deposit.into(),
                ),
                (
                    "This period - actual withdrawal".to_string(),
                    period.current_withdrawal.into(),
                ),
                ("Deposited".to_string(), period.total_deposit.into()),
                ("Withdrawn".to_string(), period.total_withdrawal.into()),
                ("Total".to_string(), period.total.into()),
            ],
            archived_from: self.archived_since,
        }])
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::period::CalendarMonthPeriodConfiguration;
    fn mkdate(month: u32, date: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2025, month, date).expect("Can create date")
    }

    mod for_period {
        use super::*;
        use crate::period::CalendarMonthPeriodConfiguration;
        use pretty_assertions::assert_eq;

        type TestResult = Result<BucketAtDate, String>;
        type ExpectedFn = Box<dyn Fn(&ExchangeRates) -> TestResult>;

        struct Test {
            executed: bool,
            lines: Vec<Line>,
            expected: ExpectedFn,
        }

        impl Default for Test {
            fn default() -> Self {
                Test {
                    executed: false,
                    lines: Vec::new(),
                    expected: Box::new(|_| Err("Please setup the test".to_string())),
                }
            }
        }

        impl Test {
            pub fn add_line(mut self, date: NaiveDate, line: Action) -> Self {
                self.lines.push(Line((date, line)));
                self
            }

            pub fn target_set_in_current_period_one_hundred_thousand_in_four_months(
                mut self,
            ) -> Self {
                self.add_line(
                    mkdate(9, 1),
                    Action::SetTarget {
                        amount: RawAmount::yen("100000"),
                        target_date: mkdate(12, 31),
                    },
                )
            }

            pub fn target_set_last_period_one_hundred_thousand_in_five_months(mut self) -> Self {
                self.add_line(
                    mkdate(8, 1),
                    Action::SetTarget {
                        amount: RawAmount::yen("100000"),
                        target_date: mkdate(12, 31),
                    },
                )
            }

            pub fn target_set_many_periods_ago_twelve_hundred_in_twelve_months(mut self) -> Self {
                self.add_line(
                    mkdate(1, 1),
                    Action::SetTarget {
                        amount: RawAmount::yen("1200"),
                        target_date: mkdate(12, 30),
                    },
                )
            }

            pub fn expect_error(mut self, error: &str) -> Self {
                let error_string = error.to_string();
                self.expected = Box::new(move |_| Err(error_string.clone()));
                self
            }

            pub fn expect_bucket(
                mut self,
                bucket_builder: impl Fn(&ExchangeRates) -> BucketAtDate + 'static,
            ) -> Self {
                self.expected = Box::new(move |ex| Ok(bucket_builder(ex)));
                self
            }

            pub fn expect_bucket_no_commits_one_hundred_thousand_in_four_months(mut self) -> Self {
                self.expect_bucket(|ex| BucketAtDate {
                    recommended_or_actual_change: ex.yen("25000"),
                    current_recommended_deposit: Some(ex.yen("25000")),
                    current_actual_deposit: None,
                    current_withdrawal: None,
                    total_deposit: ex.yen("0"),
                    total_withdrawal: ex.yen("0"),
                    total: ex.yen("0"),
                })
            }

            pub fn expect_bucket_recommended_commit_one_hundred_thousand_in_four_months(
                self,
            ) -> Self {
                self.expect_bucket(|ex| BucketAtDate {
                    recommended_or_actual_change: ex.yen("25000"),
                    current_recommended_deposit: Some(ex.yen("25000")),
                    current_actual_deposit: Some(ex.yen("25000")),
                    current_withdrawal: None,
                    total_deposit: ex.yen("25000"),
                    total_withdrawal: ex.yen("0"),
                    total: ex.yen("25000"),
                })
            }

            pub fn expect_bucket_recommended_commit_one_hundred_thousand_in_four_months_five_thousand_withdrawn(
                self,
            ) -> Self {
                self.expect_bucket(|ex| BucketAtDate {
                    recommended_or_actual_change: ex.yen("20000"),
                    current_recommended_deposit: Some(ex.yen("25000")),
                    current_actual_deposit: Some(ex.yen("25000")),
                    current_withdrawal: Some(ex.yen("5000")),
                    total_deposit: ex.yen("25000"),
                    total_withdrawal: ex.yen("5000"),
                    total: ex.yen("20000"),
                })
            }
        }

        impl Test {
            fn execute(&mut self) -> () {
                self.executed = true;
                let ex = ExchangeRates::for_tests();
                let period_configuration = PeriodConfigurationVaultValue::CalendarMonth(
                    CalendarMonthPeriodConfiguration {},
                );
                let today = mkdate(9, 15);

                let bucket = Bucket {
                    name: "test bucket inner".to_string(),
                    lines: self.lines.clone(),
                    archived_since: None,
                };

                assert_eq!(
                    (self.expected)(&ex),
                    bucket.for_period(&period_configuration, &today, &ex),
                );
            }
        }

        impl Drop for Test {
            fn drop(&mut self) {
                if !self.executed {
                    panic!("This test was not executed")
                }
            }
        }

        mod target_setting {
            use super::*;

            #[test]
            fn no_lines() {
                Test::default()
                    .expect_bucket(|ex| BucketAtDate {
                        recommended_or_actual_change: ex.yen("0"),
                        current_recommended_deposit: None,
                        current_actual_deposit: None,
                        current_withdrawal: None,
                        total_deposit: ex.yen("0"),
                        total_withdrawal: ex.yen("0"),
                        total: ex.yen("0"),
                    })
                    .execute()
            }

            #[test]
            fn last_period() {
                Test::default()
                    .add_line(
                        mkdate(9, 15),
                        Action::SetTarget {
                            amount: RawAmount::yen("100000"),
                            target_date: mkdate(8, 31),
                        },
                    )
                    .expect_bucket(|ex| BucketAtDate {
                        recommended_or_actual_change: ex.yen("100000"),
                        current_recommended_deposit: Some(ex.yen("100000")),
                        current_actual_deposit: None,
                        current_withdrawal: None,
                        total_deposit: ex.yen("0"),
                        total_withdrawal: ex.yen("0"),
                        total: ex.yen("0"),
                    })
                    .execute()
            }

            #[test]
            fn this_period() {
                Test::default()
                    .add_line(
                        mkdate(9, 15),
                        Action::SetTarget {
                            amount: RawAmount::yen("100000"),
                            target_date: mkdate(9, 30),
                        },
                    )
                    .expect_bucket(|ex| BucketAtDate {
                        recommended_or_actual_change: ex.yen("100000"),
                        current_recommended_deposit: Some(ex.yen("100000")),
                        current_actual_deposit: None,
                        current_withdrawal: None,
                        total_deposit: ex.yen("0"),
                        total_withdrawal: ex.yen("0"),
                        total: ex.yen("0"),
                    })
                    .execute()
            }

            #[test]
            fn next_period() {
                Test::default()
                    .add_line(
                        mkdate(9, 15),
                        Action::SetTarget {
                            amount: RawAmount::yen("100000"),
                            target_date: mkdate(10, 31),
                        },
                    )
                    .expect_bucket(|ex| BucketAtDate {
                        recommended_or_actual_change: ex.yen("50000"),
                        current_recommended_deposit: Some(ex.yen("50000")),
                        current_actual_deposit: None,
                        current_withdrawal: None,
                        total_deposit: ex.yen("0"),
                        total_withdrawal: ex.yen("0"),
                        total: ex.yen("0"),
                    })
                    .execute()
            }

            #[test]
            fn next_next_period() {
                Test::default()
                    .add_line(
                        mkdate(9, 15),
                        Action::SetTarget {
                            amount: RawAmount::yen("100000"),
                            target_date: mkdate(11, 30),
                        },
                    )
                    .expect_bucket(|ex| BucketAtDate {
                        recommended_or_actual_change: ex.yen("33333.33"),
                        current_recommended_deposit: Some(ex.yen("33333.33")),
                        current_actual_deposit: None,
                        current_withdrawal: None,
                        total_deposit: ex.yen("0"),
                        total_withdrawal: ex.yen("0"),
                        total: ex.yen("0"),
                    })
                    .execute()
            }

            #[test]
            fn two_set() {
                Test::default()
                    .add_line(
                        mkdate(9, 13),
                        Action::SetTarget {
                            amount: RawAmount::yen("100000"),
                            target_date: mkdate(10, 31),
                        },
                    )
                    .add_line(
                        mkdate(9, 15),
                        Action::SetTarget {
                            amount: RawAmount::yen("100000"),
                            target_date: mkdate(11, 30),
                        },
                    )
                    .expect_bucket(|ex| BucketAtDate {
                        recommended_or_actual_change: ex.yen("33333.33"),
                        current_recommended_deposit: Some(ex.yen("33333.33")),
                        current_actual_deposit: None,
                        current_withdrawal: None,
                        total_deposit: ex.yen("0"),
                        total_withdrawal: ex.yen("0"),
                        total: ex.yen("0"),
                    })
                    .execute()
            }

            #[test]
            fn set_in_the_future() {
                Test::default()
                    .add_line(mkdate(9, 14), Action::Withdrawal(RawAmount::yen("5000")))
                    .add_line(mkdate(9, 15), Action::Deposit(RawAmount::yen("10000")))
                    .add_line(
                        mkdate(11, 1),
                        Action::SetTarget {
                            amount: RawAmount::yen("20000"),
                            target_date: mkdate(12, 31),
                        },
                    )
                    .expect_bucket(|ex| BucketAtDate {
                        recommended_or_actual_change: ex.yen("5000"),
                        current_recommended_deposit: None,
                        current_actual_deposit: Some(ex.yen("10000")),
                        current_withdrawal: Some(ex.yen("5000")),
                        total_deposit: ex.yen("10000"),
                        total_withdrawal: ex.yen("5000"),
                        total: ex.yen("5000"),
                    })
                    .execute();
            }

            #[test]
            fn set_this_period_changed_in_the_future() {
                Test::default()
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(9, 10), Action::Deposit(RawAmount::yen("25000")))
                    .add_line(
                        mkdate(11, 1),
                        Action::SetTarget {
                            amount: RawAmount::yen("50000"),
                            target_date: mkdate(12, 31),
                        },
                    )
                    .expect_bucket_recommended_commit_one_hundred_thousand_in_four_months()
                    .execute();
            }
        }

        mod deposits {
            use super::*;

            mod this_period_until_today {
                use super::*;

                mod one_deposit_today {
                    use super::*;

                    #[test]
                    fn no_target() {
                        Test::default()
                            .add_line(mkdate(9, 15), Action::Deposit(RawAmount::yen("10000")))
                            .expect_bucket(|ex| BucketAtDate {
                                recommended_or_actual_change: ex.yen("10000"),
                                current_recommended_deposit: None,
                                current_actual_deposit: Some(ex.yen("10000")),
                                current_withdrawal: None,
                                total_deposit: ex.yen("10000"),
                                total_withdrawal: ex.yen("0"),
                                total: ex.yen("10000"),
                            })
                            .execute()
                    }

                    #[test]
                    fn partial() {
                        Test::default()
                            .target_set_in_current_period_one_hundred_thousand_in_four_months()
                            .add_line(mkdate(9, 15), Action::Deposit(RawAmount::yen("10000")))
                            .expect_bucket(|ex| BucketAtDate {
                                recommended_or_actual_change: ex.yen("10000"),
                                current_recommended_deposit: Some(ex.yen("25000")),
                                current_actual_deposit: Some(ex.yen("10000")),
                                current_withdrawal: None,
                                total_deposit: ex.yen("10000"),
                                total_withdrawal: ex.yen("0"),
                                total: ex.yen("10000"),
                            })
                            .execute();
                    }

                    #[test]
                    fn zero() {
                        Test::default()
                            .target_set_in_current_period_one_hundred_thousand_in_four_months()
                            .add_line(mkdate(9, 15), Action::Deposit(RawAmount::yen("0")))
                            .expect_bucket(|ex| BucketAtDate {
                                recommended_or_actual_change: ex.yen("0"),
                                current_recommended_deposit: Some(ex.yen("25000")),
                                current_actual_deposit: Some(ex.yen("0")),
                                current_withdrawal: None,
                                total_deposit: ex.yen("0"),
                                total_withdrawal: ex.yen("0"),
                                total: ex.yen("0"),
                            })
                            .execute();
                    }
                }

                mod two_deposits {
                    use super::*;

                    #[test]
                    fn recommended() {
                        Test::default()
                            .target_set_in_current_period_one_hundred_thousand_in_four_months()
                            .add_line(mkdate(9, 3), Action::Deposit(RawAmount::yen("10000")))
                            .add_line(mkdate(9, 5), Action::Deposit(RawAmount::yen("15000")))
                            .expect_bucket(|ex| BucketAtDate {
                                recommended_or_actual_change: ex.yen("25000"),
                                current_recommended_deposit: Some(ex.yen("25000")),
                                current_actual_deposit: Some(ex.yen("25000")),
                                current_withdrawal: None,
                                total_deposit: ex.yen("25000"),
                                total_withdrawal: ex.yen("0"),
                                total: ex.yen("25000"),
                            })
                            .execute();
                    }

                    #[test]
                    fn same_day() {
                        Test::default()
                            .target_set_in_current_period_one_hundred_thousand_in_four_months()
                            .add_line(mkdate(9, 5), Action::Deposit(RawAmount::yen("10000")))
                            .add_line(mkdate(9, 5), Action::Deposit(RawAmount::yen("15000")))
                            .expect_bucket(|ex| BucketAtDate {
                                recommended_or_actual_change: ex.yen("25000"),
                                current_recommended_deposit: Some(ex.yen("25000")),
                                current_actual_deposit: Some(ex.yen("25000")),
                                current_withdrawal: None,
                                total_deposit: ex.yen("25000"),
                                total_withdrawal: ex.yen("0"),
                                total: ex.yen("25000"),
                            })
                            .execute();
                    }
                }

                mod one_deposit_period_start {
                    use super::*;

                    #[test]
                    fn partial() {
                        Test::default()
                            .target_set_in_current_period_one_hundred_thousand_in_four_months()
                            .add_line(mkdate(9, 1), Action::Deposit(RawAmount::yen("10000")))
                            .expect_bucket(|ex| BucketAtDate {
                                recommended_or_actual_change: ex.yen("10000"),
                                current_recommended_deposit: Some(ex.yen("25000")),
                                current_actual_deposit: Some(ex.yen("10000")),
                                current_withdrawal: None,
                                total_deposit: ex.yen("10000"),
                                total_withdrawal: ex.yen("0"),
                                total: ex.yen("10000"),
                            })
                            .execute();
                    }
                }

                mod one_deposit_before_today {
                    use super::*;

                    #[test]
                    fn partial() {
                        Test::default()
                            .target_set_last_period_one_hundred_thousand_in_five_months()
                            .add_line(mkdate(9, 3), Action::Deposit(RawAmount::yen("10000")))
                            .expect_bucket(|ex| BucketAtDate {
                                recommended_or_actual_change: ex.yen("10000"),
                                current_recommended_deposit: Some(ex.yen("25000")), // This is correct. Even if the target was set for five months, there was no deposit last month
                                current_actual_deposit: Some(ex.yen("10000")),
                                current_withdrawal: None,
                                total_deposit: ex.yen("10000"),
                                total_withdrawal: ex.yen("0"),
                                total: ex.yen("10000"),
                            })
                            .execute();
                    }

                    #[test]
                    fn over() {
                        Test::default()
                            .target_set_in_current_period_one_hundred_thousand_in_four_months()
                            .add_line(mkdate(9, 3), Action::Deposit(RawAmount::yen("30000")))
                            .expect_bucket(|ex| BucketAtDate {
                                recommended_or_actual_change: ex.yen("30000"),
                                current_recommended_deposit: Some(ex.yen("25000")),
                                current_actual_deposit: Some(ex.yen("30000")),
                                current_withdrawal: None,
                                total_deposit: ex.yen("30000"),
                                total_withdrawal: ex.yen("0"),
                                total: ex.yen("30000"),
                            })
                            .execute();
                    }
                }
            }
            mod before_current_period {
                use super::*;

                mod one_deposit {
                    use super::*;

                    #[test]
                    fn recommended() {
                        Test::default()
                            .target_set_last_period_one_hundred_thousand_in_five_months()
                            .add_line(mkdate(8, 31), Action::Deposit(RawAmount::yen("20000")))
                            .expect_bucket(|ex| BucketAtDate {
                                recommended_or_actual_change: ex.yen("20000"),
                                current_recommended_deposit: Some(ex.yen("20000")),
                                current_actual_deposit: None,
                                current_withdrawal: None,
                                total_deposit: ex.yen("20000"),
                                total_withdrawal: ex.yen("0"),
                                total: ex.yen("20000"),
                            })
                            .execute();
                    }

                    #[test]
                    fn over_recommendation() {
                        Test::default()
                            .target_set_last_period_one_hundred_thousand_in_five_months()
                            .add_line(mkdate(8, 31), Action::Deposit(RawAmount::yen("60000")))
                            .expect_bucket(|ex| BucketAtDate {
                                recommended_or_actual_change: ex.yen("10000"),
                                current_recommended_deposit: Some(ex.yen("10000")),
                                current_actual_deposit: None,
                                current_withdrawal: None,
                                total_deposit: ex.yen("60000"),
                                total_withdrawal: ex.yen("0"),
                                total: ex.yen("60000"),
                            })
                            .execute();
                    }

                    #[test]
                    fn over_target() {
                        Test::default()
                            .target_set_last_period_one_hundred_thousand_in_five_months()
                            .add_line(mkdate(8, 31), Action::Deposit(RawAmount::yen("200000")))
                            .expect_bucket(|ex| BucketAtDate {
                                recommended_or_actual_change: ex.yen("0"),
                                current_recommended_deposit: Some(ex.yen("0")),
                                current_actual_deposit: None,
                                current_withdrawal: None,
                                total_deposit: ex.yen("200000"),
                                total_withdrawal: ex.yen("0"),
                                total: ex.yen("200000"),
                            })
                            .execute();
                    }
                }

                mod two_deposits {
                    use super::*;

                    #[test]
                    fn partial() {
                        Test::default()
                            .target_set_last_period_one_hundred_thousand_in_five_months()
                            .add_line(mkdate(8, 15), Action::Deposit(RawAmount::yen("5000")))
                            .add_line(mkdate(8, 31), Action::Deposit(RawAmount::yen("5000")))
                            .expect_bucket(|ex| BucketAtDate {
                                recommended_or_actual_change: ex.yen("22500"),
                                current_recommended_deposit: Some(ex.yen("22500")),
                                current_actual_deposit: None,
                                current_withdrawal: None,
                                total_deposit: ex.yen("10000"),
                                total_withdrawal: ex.yen("0"),
                                total: ex.yen("10000"),
                            })
                            .execute();
                    }

                    #[test]
                    fn same_day() {
                        Test::default()
                            .target_set_last_period_one_hundred_thousand_in_five_months()
                            .add_line(mkdate(8, 31), Action::Deposit(RawAmount::yen("5000")))
                            .add_line(mkdate(8, 31), Action::Deposit(RawAmount::yen("5000")))
                            .expect_bucket(|ex| BucketAtDate {
                                recommended_or_actual_change: ex.yen("22500"),
                                current_recommended_deposit: Some(ex.yen("22500")),
                                current_actual_deposit: None,
                                current_withdrawal: None,
                                total_deposit: ex.yen("10000"),
                                total_withdrawal: ex.yen("0"),
                                total: ex.yen("10000"),
                            })
                            .execute();
                    }
                }

                #[test]
                fn many_periods_ago() {
                    Test::default()
                        .target_set_many_periods_ago_twelve_hundred_in_twelve_months()
                        .add_line(mkdate(1, 15), Action::Deposit(RawAmount::yen("100")))
                        .add_line(mkdate(2, 28), Action::Deposit(RawAmount::yen("100")))
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("250"),
                            current_recommended_deposit: Some(ex.yen("250")),
                            current_actual_deposit: None,
                            current_withdrawal: None,
                            total_deposit: ex.yen("200"),
                            total_withdrawal: ex.yen("0"),
                            total: ex.yen("200"),
                        })
                        .execute();
                }
            }
            mod after_current_period {
                use super::*;
                use crate::amounts::RawAmount;
                use crate::buckets::Action;

                #[test]
                fn one_deposit_tomorrow() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 16), Action::Deposit(RawAmount::yen("25000")))
                        .expect_bucket_no_commits_one_hundred_thousand_in_four_months()
                        .execute();
                }

                #[test]
                fn one_deposit_this_period_after_tomorrow() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 17), Action::Deposit(RawAmount::yen("25000")))
                        .expect_bucket_no_commits_one_hundred_thousand_in_four_months()
                        .execute();
                }

                #[test]
                fn one_deposit_next_period() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(10, 18), Action::Deposit(RawAmount::yen("25000")))
                        .expect_bucket_no_commits_one_hundred_thousand_in_four_months()
                        .execute();
                }

                #[test]
                fn one_deposit_many_period_after() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(12, 18), Action::Deposit(RawAmount::yen("25000")))
                        .expect_bucket_no_commits_one_hundred_thousand_in_four_months()
                        .execute();
                }

                #[test]
                fn many_deposits() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 16), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(mkdate(9, 17), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(mkdate(10, 18), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(mkdate(12, 18), Action::Deposit(RawAmount::yen("25000")))
                        .expect_bucket_no_commits_one_hundred_thousand_in_four_months()
                        .execute();
                }
            }
            mod across_periods {
                use super::*;

                #[test]
                fn one_deposit_this_period_next_period() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 10), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(mkdate(10, 18), Action::Deposit(RawAmount::yen("25000")))
                        .expect_bucket(
                            (|ex| BucketAtDate {
                                recommended_or_actual_change: ex.yen("25000"),
                                current_recommended_deposit: Some(ex.yen("25000")),
                                current_actual_deposit: Some(ex.yen("25000")),
                                current_withdrawal: None,
                                total_deposit: ex.yen("25000"),
                                total_withdrawal: ex.yen("0"),
                                total: ex.yen("25000"),
                            }),
                        )
                        .execute();
                }

                #[test]
                fn all_periods() {
                    Test::default()
                        .target_set_many_periods_ago_twelve_hundred_in_twelve_months()
                        .add_line(mkdate(1, 15), Action::Deposit(RawAmount::yen("50")))
                        .add_line(mkdate(2, 1), Action::Deposit(RawAmount::yen("55")))
                        .add_line(mkdate(8, 31), Action::Deposit(RawAmount::yen("55")))
                        .add_line(mkdate(9, 1), Action::Deposit(RawAmount::yen("200")))
                        .add_line(mkdate(9, 15), Action::Deposit(RawAmount::yen("50")))
                        .add_line(mkdate(9, 20), Action::Deposit(RawAmount::yen("10")))
                        .add_line(mkdate(10, 25), Action::Deposit(RawAmount::yen("260")))
                        .add_line(mkdate(12, 31), Action::Deposit(RawAmount::yen("260")))
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("250"),
                            current_recommended_deposit: Some(ex.yen("260")),
                            current_actual_deposit: Some(ex.yen("250")),
                            current_withdrawal: None,
                            total_deposit: ex.yen("410"),
                            total_withdrawal: ex.yen("0"),
                            total: ex.yen("410"),
                        })
                        .execute();
                }

                #[test]
                fn all_periods_multiple_deposits() {
                    Test::default()
                        .target_set_many_periods_ago_twelve_hundred_in_twelve_months()
                        .add_line(mkdate(1, 15), Action::Deposit(RawAmount::yen("25")))
                        .add_line(mkdate(1, 16), Action::Deposit(RawAmount::yen("25")))
                        .add_line(mkdate(2, 1), Action::Deposit(RawAmount::yen("50")))
                        .add_line(mkdate(2, 2), Action::Deposit(RawAmount::yen("5")))
                        .add_line(mkdate(8, 30), Action::Deposit(RawAmount::yen("25")))
                        .add_line(mkdate(8, 31), Action::Deposit(RawAmount::yen("30")))
                        .add_line(mkdate(9, 1), Action::Deposit(RawAmount::yen("200")))
                        .add_line(mkdate(9, 15), Action::Deposit(RawAmount::yen("50")))
                        .add_line(mkdate(9, 20), Action::Deposit(RawAmount::yen("10")))
                        .add_line(mkdate(10, 25), Action::Deposit(RawAmount::yen("200")))
                        .add_line(mkdate(10, 26), Action::Deposit(RawAmount::yen("60")))
                        .add_line(mkdate(12, 30), Action::Deposit(RawAmount::yen("200")))
                        .add_line(mkdate(12, 31), Action::Deposit(RawAmount::yen("60")))
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("250"),
                            current_recommended_deposit: Some(ex.yen("260")),
                            current_actual_deposit: Some(ex.yen("250")),
                            current_withdrawal: None,
                            total_deposit: ex.yen("410"),
                            total_withdrawal: ex.yen("0"),
                            total: ex.yen("410"),
                        })
                        .execute();
                }
            }
        }

        mod deposits_cancellation {
            use super::*;

            mod this_period_until_today {
                use super::*;

                #[test]
                fn one_today() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 8), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(
                            mkdate(9, 15),
                            Action::DepositCancellation(RawAmount::yen("10000")),
                        )
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("15000"),
                            current_recommended_deposit: Some(ex.yen("25000")),
                            current_actual_deposit: Some(ex.yen("15000")),
                            current_withdrawal: None,
                            total_deposit: ex.yen("15000"),
                            total_withdrawal: ex.yen("0"),
                            total: ex.yen("15000"),
                        })
                        .execute();
                }

                #[test]
                fn one_today_deposit_last_period() {
                    Test::default()
                        .target_set_last_period_one_hundred_thousand_in_five_months()
                        .add_line(mkdate(8, 8), Action::Deposit(RawAmount::yen("20000")))
                        .add_line(
                            mkdate(9, 15),
                            Action::DepositCancellation(RawAmount::yen("10000")),
                        )
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("-10000"),
                            current_recommended_deposit: Some(ex.yen("20000")),
                            current_actual_deposit: Some(ex.yen("-10000")),
                            current_withdrawal: None,
                            total_deposit: ex.yen("10000"),
                            total_withdrawal: ex.yen("0"),
                            total: ex.yen("10000"),
                        })
                        .execute();
                }

                #[test]
                fn two_this_period_deposit_last_period() {
                    Test::default()
                        .target_set_last_period_one_hundred_thousand_in_five_months()
                        .add_line(mkdate(8, 8), Action::Deposit(RawAmount::yen("20000")))
                        .add_line(
                            mkdate(9, 14),
                            Action::DepositCancellation(RawAmount::yen("5000")),
                        )
                        .add_line(
                            mkdate(9, 15),
                            Action::DepositCancellation(RawAmount::yen("5000")),
                        )
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("-10000"),
                            current_recommended_deposit: Some(ex.yen("20000")),
                            current_actual_deposit: Some(ex.yen("-10000")),
                            current_withdrawal: None,
                            total_deposit: ex.yen("10000"),
                            total_withdrawal: ex.yen("0"),
                            total: ex.yen("10000"),
                        })
                        .execute();
                }

                #[test]
                fn two_this_period() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 8), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(
                            mkdate(9, 14),
                            Action::DepositCancellation(RawAmount::yen("5000")),
                        )
                        .add_line(
                            mkdate(9, 15),
                            Action::DepositCancellation(RawAmount::yen("5000")),
                        )
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("15000"),
                            current_recommended_deposit: Some(ex.yen("25000")),
                            current_actual_deposit: Some(ex.yen("15000")),
                            current_withdrawal: None,
                            total_deposit: ex.yen("15000"),
                            total_withdrawal: ex.yen("0"),
                            total: ex.yen("15000"),
                        })
                        .execute();
                }

                #[test]
                fn one_today_deposit_the_same_day() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 15), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(
                            mkdate(9, 15),
                            Action::DepositCancellation(RawAmount::yen("10000")),
                        )
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("15000"),
                            current_recommended_deposit: Some(ex.yen("25000")),
                            current_actual_deposit: Some(ex.yen("15000")),
                            current_withdrawal: None,
                            total_deposit: ex.yen("15000"),
                            total_withdrawal: ex.yen("0"),
                            total: ex.yen("15000"),
                        })
                        .execute();
                }

                #[test]
                fn one_today_cancels_too_much() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 8), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(
                            mkdate(9, 15),
                            Action::DepositCancellation(RawAmount::yen("30000")),
                        )
                        .expect_error("attempt to remove more than was deposited")
                        .execute();
                }

                #[test]
                fn one_cancels_everything() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 8), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(
                            mkdate(9, 15),
                            Action::DepositCancellation(RawAmount::yen("25000")),
                        )
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("0"),
                            current_recommended_deposit: Some(ex.yen("25000")),
                            current_actual_deposit: Some(ex.yen("0")),
                            current_withdrawal: None,
                            total_deposit: ex.yen("0"),
                            total_withdrawal: ex.yen("0"),
                            total: ex.yen("0"),
                        })
                        .execute();
                }

                #[test]
                fn one_cancellation_too_big_followed_by_one_deposit_that_brings_back_the_bucket_to_positive(
                ) {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 8), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(
                            mkdate(9, 13),
                            Action::DepositCancellation(RawAmount::yen("30000")),
                        )
                        .add_line(mkdate(9, 15), Action::Deposit(RawAmount::yen("30000")))
                        .expect_error("attempt to remove more than was deposited")
                        .execute();
                }
            }

            mod before_current_period {
                use super::*;

                #[test]
                fn one_cancellation() {
                    Test::default()
                        .target_set_last_period_one_hundred_thousand_in_five_months()
                        .add_line(mkdate(8, 1), Action::Deposit(RawAmount::yen("20000")))
                        .add_line(
                            mkdate(8, 31),
                            Action::DepositCancellation(RawAmount::yen("10000")),
                        )
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("22500"),
                            current_recommended_deposit: Some(ex.yen("22500")),
                            current_actual_deposit: None,
                            current_withdrawal: None,
                            total_deposit: ex.yen("10000"),
                            total_withdrawal: ex.yen("0"),
                            total: ex.yen("10000"),
                        })
                        .execute();
                }

                #[test]
                fn two_cancellations() {
                    Test::default()
                        .target_set_last_period_one_hundred_thousand_in_five_months()
                        .add_line(mkdate(8, 1), Action::Deposit(RawAmount::yen("20000")))
                        .add_line(
                            mkdate(8, 15),
                            Action::DepositCancellation(RawAmount::yen("5000")),
                        )
                        .add_line(
                            mkdate(8, 31),
                            Action::DepositCancellation(RawAmount::yen("5000")),
                        )
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("22500"),
                            current_recommended_deposit: Some(ex.yen("22500")),
                            current_actual_deposit: None,
                            current_withdrawal: None,
                            total_deposit: ex.yen("10000"),
                            total_withdrawal: ex.yen("0"),
                            total: ex.yen("10000"),
                        })
                        .execute();
                }

                #[test]
                fn one_today_deposit_the_same_day() {
                    Test::default()
                        .target_set_last_period_one_hundred_thousand_in_five_months()
                        .add_line(mkdate(8, 15), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(
                            mkdate(8, 15),
                            Action::DepositCancellation(RawAmount::yen("10000")),
                        )
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("21250"),
                            current_recommended_deposit: Some(ex.yen("21250")),
                            current_actual_deposit: None,
                            current_withdrawal: None,
                            total_deposit: ex.yen("15000"),
                            total_withdrawal: ex.yen("0"),
                            total: ex.yen("15000"),
                        })
                        .execute();
                }

                #[test]
                fn one_today_too_cancels_too_much() {
                    Test::default()
                        .target_set_last_period_one_hundred_thousand_in_five_months()
                        .add_line(mkdate(8, 8), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(
                            mkdate(8, 15),
                            Action::DepositCancellation(RawAmount::yen("30000")),
                        )
                        .expect_error("attempt to remove more than was deposited")
                        .execute();
                }

                #[test]
                fn one_cancellation_too_big_followed_by_one_deposit_that_brings_back_the_bucket_to_positive(
                ) {
                    Test::default()
                        .target_set_last_period_one_hundred_thousand_in_five_months()
                        .add_line(mkdate(8, 8), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(
                            mkdate(8, 13),
                            Action::DepositCancellation(RawAmount::yen("30000")),
                        )
                        .add_line(mkdate(8, 15), Action::Deposit(RawAmount::yen("30000")))
                        .expect_error("attempt to remove more than was deposited")
                        .execute();
                }
            }

            mod after_current_period {
                use super::*;

                #[test]
                fn one_cancellation_tomorrow() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 1), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(
                            mkdate(9, 16),
                            Action::DepositCancellation(RawAmount::yen("25000")),
                        )
                        .expect_bucket_recommended_commit_one_hundred_thousand_in_four_months()
                        .execute();
                }

                #[test]
                fn one_cancellation_this_period_after_tomorrow() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 1), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(
                            mkdate(9, 17),
                            Action::DepositCancellation(RawAmount::yen("25000")),
                        )
                        .expect_bucket_recommended_commit_one_hundred_thousand_in_four_months()
                        .execute();
                }

                #[test]
                fn one_cancellation_next_period() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 1), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(
                            mkdate(10, 18),
                            Action::DepositCancellation(RawAmount::yen("25000")),
                        )
                        .expect_bucket_recommended_commit_one_hundred_thousand_in_four_months()
                        .execute();
                }

                #[test]
                fn one_cancellation_many_period_after() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 1), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(
                            mkdate(12, 18),
                            Action::DepositCancellation(RawAmount::yen("25000")),
                        )
                        .expect_bucket_recommended_commit_one_hundred_thousand_in_four_months()
                        .execute();
                }

                #[test]
                fn many_deposits() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 1), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(
                            mkdate(9, 16),
                            Action::DepositCancellation(RawAmount::yen("5000")),
                        )
                        .add_line(
                            mkdate(9, 17),
                            Action::DepositCancellation(RawAmount::yen("5000")),
                        )
                        .add_line(
                            mkdate(10, 18),
                            Action::DepositCancellation(RawAmount::yen("5000")),
                        )
                        .add_line(
                            mkdate(12, 18),
                            Action::DepositCancellation(RawAmount::yen("5000")),
                        )
                        .add_line(
                            mkdate(12, 19),
                            Action::DepositCancellation(RawAmount::yen("5000")),
                        )
                        .expect_bucket_recommended_commit_one_hundred_thousand_in_four_months()
                        .execute();
                }
                #[test]
                fn one_cancellation_too_big_followed_by_one_deposit_that_brings_back_the_bucket_to_positive(
                ) {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(10, 8), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(
                            mkdate(10, 13),
                            Action::DepositCancellation(RawAmount::yen("30000")),
                        )
                        .add_line(mkdate(10, 15), Action::Deposit(RawAmount::yen("30000")))
                        .expect_error("attempt to remove more than was deposited")
                        .execute();
                }
            }

            mod across_periods {
                use super::*;
                #[test]
                fn one_deposit_this_period_next_period() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 10), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(
                            mkdate(9, 11),
                            Action::DepositCancellation(RawAmount::yen("5000")),
                        )
                        .add_line(
                            mkdate(10, 18),
                            Action::DepositCancellation(RawAmount::yen("5000")),
                        )
                        .expect_bucket(
                            (|ex| BucketAtDate {
                                recommended_or_actual_change: ex.yen("20000"),
                                current_recommended_deposit: Some(ex.yen("25000")),
                                current_actual_deposit: Some(ex.yen("20000")),
                                current_withdrawal: None,
                                total_deposit: ex.yen("20000"),
                                total_withdrawal: ex.yen("0"),
                                total: ex.yen("20000"),
                            }),
                        )
                        .execute();
                }

                #[test]
                fn one_cancellation_this_and_last_period() {
                    Test::default()
                        .target_set_last_period_one_hundred_thousand_in_five_months()
                        .add_line(mkdate(8, 1), Action::Deposit(RawAmount::yen("20000")))
                        .add_line(
                            mkdate(8, 31),
                            Action::DepositCancellation(RawAmount::yen("10000")),
                        )
                        .add_line(
                            mkdate(9, 10),
                            Action::DepositCancellation(RawAmount::yen("10000")),
                        )
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("-10000"),
                            current_recommended_deposit: Some(ex.yen("22500")),
                            current_actual_deposit: Some(ex.yen("-10000")),
                            current_withdrawal: None,
                            total_deposit: ex.yen("0"),
                            total_withdrawal: ex.yen("0"),
                            total: ex.yen("0"),
                        })
                        .execute();
                }

                #[test]
                fn one_deposit_and_one_cancellation_this_and_last_period() {
                    Test::default()
                        .target_set_last_period_one_hundred_thousand_in_five_months()
                        .add_line(mkdate(8, 1), Action::Deposit(RawAmount::yen("20000")))
                        .add_line(
                            mkdate(8, 31),
                            Action::DepositCancellation(RawAmount::yen("10000")),
                        )
                        .add_line(mkdate(9, 4), Action::Deposit(RawAmount::yen("20000")))
                        .add_line(
                            mkdate(9, 10),
                            Action::DepositCancellation(RawAmount::yen("1000")),
                        )
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("19000"),
                            current_recommended_deposit: Some(ex.yen("22500")),
                            current_actual_deposit: Some(ex.yen("19000")),
                            current_withdrawal: None,
                            total_deposit: ex.yen("29000"),
                            total_withdrawal: ex.yen("0"),
                            total: ex.yen("29000"),
                        })
                        .execute();
                }
            }
        }

        mod withdrawal {
            use super::*;

            mod this_period_until_today {
                use super::*;

                #[test]
                fn two_withdrawals() {
                    Test::default()
                        .add_line(mkdate(9, 8), Action::Withdrawal(RawAmount::yen("25000")))
                        .add_line(mkdate(9, 9), Action::Withdrawal(RawAmount::yen("5000")))
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("-30000"),
                            current_recommended_deposit: None,
                            current_actual_deposit: None,
                            current_withdrawal: Some(ex.yen("30000")),
                            total_deposit: ex.yen("0"),
                            total_withdrawal: ex.yen("30000"),
                            total: ex.yen("-30000"),
                        })
                        .execute()
                }

                #[test]
                fn one_today() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 8), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(mkdate(9, 15), Action::Withdrawal(RawAmount::yen("10000")))
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("15000"),
                            current_recommended_deposit: Some(ex.yen("25000")),
                            current_actual_deposit: Some(ex.yen("25000")),
                            current_withdrawal: Some(ex.yen("10000")),
                            total_deposit: ex.yen("25000"),
                            total_withdrawal: ex.yen("10000"),
                            total: ex.yen("15000"),
                        })
                        .execute();
                }

                #[test]
                fn one_today_deposit_last_period() {
                    Test::default()
                        .target_set_last_period_one_hundred_thousand_in_five_months()
                        .add_line(mkdate(8, 8), Action::Deposit(RawAmount::yen("20000")))
                        .add_line(mkdate(9, 15), Action::Withdrawal(RawAmount::yen("15000")))
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("-15000"),
                            current_recommended_deposit: Some(ex.yen("20000")),
                            current_actual_deposit: None,
                            current_withdrawal: Some(ex.yen("15000")),
                            total_deposit: ex.yen("20000"),
                            total_withdrawal: ex.yen("15000"),
                            total: ex.yen("5000"),
                        })
                        .execute();
                }

                #[test]
                fn two_this_period_deposit_last_period() {
                    Test::default()
                        .target_set_last_period_one_hundred_thousand_in_five_months()
                        .add_line(mkdate(8, 8), Action::Deposit(RawAmount::yen("20000")))
                        .add_line(mkdate(9, 14), Action::Withdrawal(RawAmount::yen("5000")))
                        .add_line(mkdate(9, 15), Action::Withdrawal(RawAmount::yen("10000")))
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("-15000"),
                            current_recommended_deposit: Some(ex.yen("20000")),
                            current_actual_deposit: None,
                            current_withdrawal: Some(ex.yen("15000")),
                            total_deposit: ex.yen("20000"),
                            total_withdrawal: ex.yen("15000"),
                            total: ex.yen("5000"),
                        })
                        .execute();
                }

                #[test]
                fn two_this_period() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 8), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(mkdate(9, 14), Action::Withdrawal(RawAmount::yen("5000")))
                        .add_line(mkdate(9, 15), Action::Withdrawal(RawAmount::yen("5000")))
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("15000"),
                            current_recommended_deposit: Some(ex.yen("25000")),
                            current_actual_deposit: Some(ex.yen("25000")),
                            current_withdrawal: Some(ex.yen("10000")),
                            total_deposit: ex.yen("25000"),
                            total_withdrawal: ex.yen("10000"),
                            total: ex.yen("15000"),
                        })
                        .execute();
                }

                #[test]
                fn one_today_deposit_the_same_day() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 15), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(mkdate(9, 15), Action::Withdrawal(RawAmount::yen("10000")))
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("15000"),
                            current_recommended_deposit: Some(ex.yen("25000")),
                            current_actual_deposit: Some(ex.yen("25000")),
                            current_withdrawal: Some(ex.yen("10000")),
                            total_deposit: ex.yen("25000"),
                            total_withdrawal: ex.yen("10000"),
                            total: ex.yen("15000"),
                        })
                        .execute();
                }

                #[test]
                fn one_today_deposit_the_same_day_reverse_order() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 15), Action::Withdrawal(RawAmount::yen("10000")))
                        .add_line(mkdate(9, 15), Action::Deposit(RawAmount::yen("25000")))
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("15000"),
                            current_recommended_deposit: Some(ex.yen("25000")),
                            current_actual_deposit: Some(ex.yen("25000")),
                            current_withdrawal: Some(ex.yen("10000")),
                            total_deposit: ex.yen("25000"),
                            total_withdrawal: ex.yen("10000"),
                            total: ex.yen("15000"),
                        })
                        .execute();
                }

                #[test]
                fn one_today_withdraw_too_much() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 8), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(mkdate(9, 15), Action::Withdrawal(RawAmount::yen("30000")))
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("-5000"),
                            current_recommended_deposit: Some(ex.yen("25000")),
                            current_actual_deposit: Some(ex.yen("25000")),
                            current_withdrawal: Some(ex.yen("30000")),
                            total_deposit: ex.yen("25000"),
                            total_withdrawal: ex.yen("30000"),
                            total: ex.yen("-5000"),
                        })
                        .execute();
                }

                #[test]
                fn one_withdraw_everything() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 8), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(mkdate(9, 15), Action::Withdrawal(RawAmount::yen("25000")))
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("0"),
                            current_recommended_deposit: Some(ex.yen("25000")),
                            current_actual_deposit: Some(ex.yen("25000")),
                            current_withdrawal: Some(ex.yen("25000")),
                            total_deposit: ex.yen("25000"),
                            total_withdrawal: ex.yen("25000"),
                            total: ex.yen("0"),
                        })
                        .execute();
                }

                #[test]
                fn one_withdrawal_too_big_followed_by_one_deposit_that_brings_back_the_bucket_to_positive(
                ) {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 8), Action::Deposit(RawAmount::yen("25000")))
                        // This is a bit of a dumb scenario - saving 30000 yen and then withdrawing them immediately
                        // It is however valid
                        .add_line(mkdate(9, 13), Action::Withdrawal(RawAmount::yen("30000")))
                        .add_line(mkdate(9, 15), Action::Deposit(RawAmount::yen("30000")))
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("25000"),
                            current_recommended_deposit: Some(ex.yen("25000")),
                            current_actual_deposit: Some(ex.yen("55000")),
                            current_withdrawal: Some(ex.yen("30000")),
                            total_deposit: ex.yen("55000"),
                            total_withdrawal: ex.yen("30000"),
                            total: ex.yen("25000"),
                        })
                        .execute();
                }
            }

            mod before_current_period {
                use super::*;

                #[test]
                fn one_withdrawal() {
                    Test::default()
                        .target_set_last_period_one_hundred_thousand_in_five_months()
                        .add_line(mkdate(8, 1), Action::Deposit(RawAmount::yen("20000")))
                        .add_line(mkdate(8, 31), Action::Withdrawal(RawAmount::yen("10000")))
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("20000"),
                            current_recommended_deposit: Some(ex.yen("20000")),
                            current_actual_deposit: None,
                            current_withdrawal: None,
                            total_deposit: ex.yen("20000"),
                            total_withdrawal: ex.yen("10000"),
                            total: ex.yen("10000"),
                        })
                        .execute();
                }

                #[test]
                fn two_withdrawal() {
                    Test::default()
                        .target_set_last_period_one_hundred_thousand_in_five_months()
                        .add_line(mkdate(8, 1), Action::Deposit(RawAmount::yen("20000")))
                        .add_line(mkdate(8, 15), Action::Withdrawal(RawAmount::yen("5000")))
                        .add_line(mkdate(8, 31), Action::Withdrawal(RawAmount::yen("5000")))
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("20000"),
                            current_recommended_deposit: Some(ex.yen("20000")),
                            current_actual_deposit: None,
                            current_withdrawal: None,
                            total_deposit: ex.yen("20000"),
                            total_withdrawal: ex.yen("10000"),
                            total: ex.yen("10000"),
                        })
                        .execute();
                }

                #[test]
                fn one_today_deposit_the_same_day() {
                    Test::default()
                        .target_set_last_period_one_hundred_thousand_in_five_months()
                        .add_line(mkdate(8, 15), Action::Deposit(RawAmount::yen("20000")))
                        .add_line(mkdate(8, 15), Action::Withdrawal(RawAmount::yen("10000")))
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("20000"),
                            current_recommended_deposit: Some(ex.yen("20000")),
                            current_actual_deposit: None,
                            current_withdrawal: None,
                            total_deposit: ex.yen("20000"),
                            total_withdrawal: ex.yen("10000"),
                            total: ex.yen("10000"),
                        })
                        .execute();
                }

                #[test]
                fn withdraw_more_than_deposited() {
                    Test::default()
                        .target_set_last_period_one_hundred_thousand_in_five_months()
                        .add_line(mkdate(8, 8), Action::Deposit(RawAmount::yen("20000")))
                        .add_line(mkdate(8, 15), Action::Withdrawal(RawAmount::yen("30000")))
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("20000"),
                            current_recommended_deposit: Some(ex.yen("20000")),
                            current_actual_deposit: None,
                            current_withdrawal: None,
                            total_deposit: ex.yen("20000"),
                            total_withdrawal: ex.yen("30000"),
                            total: ex.yen("-10000"),
                        })
                        .execute();
                }

                #[test]
                fn one_withdrawal_too_big_followed_by_one_deposit_that_brings_back_the_bucket_to_positive(
                ) {
                    Test::default()
                        .target_set_last_period_one_hundred_thousand_in_five_months()
                        .add_line(mkdate(8, 8), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(
                            mkdate(8, 13),
                            Action::DepositCancellation(RawAmount::yen("30000")),
                        )
                        .add_line(mkdate(8, 15), Action::Deposit(RawAmount::yen("30000")))
                        .expect_error("attempt to remove more than was deposited")
                        .execute();
                }
            }

            mod after_current_period {
                use super::*;

                #[test]
                fn one_withdrawal_tomorrow() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 1), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(mkdate(9, 16), Action::Withdrawal(RawAmount::yen("25000")))
                        .expect_bucket_recommended_commit_one_hundred_thousand_in_four_months()
                        .execute();
                }

                #[test]
                fn one_withdrawal_this_period_after_tomorrow() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 1), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(mkdate(9, 17), Action::Withdrawal(RawAmount::yen("25000")))
                        .expect_bucket_recommended_commit_one_hundred_thousand_in_four_months()
                        .execute();
                }

                #[test]
                fn one_withdrawal_next_period() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 1), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(mkdate(10, 18), Action::Withdrawal(RawAmount::yen("25000")))
                        .expect_bucket_recommended_commit_one_hundred_thousand_in_four_months()
                        .execute();
                }

                #[test]
                fn one_deposit_many_period_after() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 1), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(mkdate(12, 18), Action::Withdrawal(RawAmount::yen("25000")))
                        .expect_bucket_recommended_commit_one_hundred_thousand_in_four_months()
                        .execute();
                }

                #[test]
                fn many_deposits() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 1), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(mkdate(9, 16), Action::Withdrawal(RawAmount::yen("25000")))
                        .add_line(mkdate(9, 17), Action::Withdrawal(RawAmount::yen("25000")))
                        .add_line(mkdate(10, 18), Action::Withdrawal(RawAmount::yen("25000")))
                        .add_line(mkdate(12, 18), Action::Withdrawal(RawAmount::yen("25000")))
                        .expect_bucket_recommended_commit_one_hundred_thousand_in_four_months()
                        .execute();
                }
            }

            mod across_periods {
                use super::*;
                #[test]
                fn one_deposit_this_period_next_period() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 10), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(
                            mkdate(9, 11),
                            Action::DepositCancellation(RawAmount::yen("5000")),
                        )
                        .add_line(
                            mkdate(10, 18),
                            Action::DepositCancellation(RawAmount::yen("5000")),
                        )
                        .expect_bucket(
                            (|ex| BucketAtDate {
                                recommended_or_actual_change: ex.yen("20000"),
                                current_recommended_deposit: Some(ex.yen("25000")),
                                current_actual_deposit: Some(ex.yen("20000")),
                                current_withdrawal: None,
                                total_deposit: ex.yen("20000"),
                                total_withdrawal: ex.yen("0"),
                                total: ex.yen("20000"),
                            }),
                        )
                        .execute();
                }

                #[test]
                fn one_cancellation_this_and_last_period() {
                    Test::default()
                        .target_set_last_period_one_hundred_thousand_in_five_months()
                        .add_line(mkdate(8, 1), Action::Deposit(RawAmount::yen("20000")))
                        .add_line(
                            mkdate(8, 31),
                            Action::DepositCancellation(RawAmount::yen("10000")),
                        )
                        .add_line(
                            mkdate(9, 10),
                            Action::DepositCancellation(RawAmount::yen("10000")),
                        )
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("-10000"),
                            current_recommended_deposit: Some(ex.yen("22500")),
                            current_actual_deposit: Some(ex.yen("-10000")),
                            current_withdrawal: None,
                            total_deposit: ex.yen("0"),
                            total_withdrawal: ex.yen("0"),
                            total: ex.yen("0"),
                        })
                        .execute();
                }

                #[test]
                fn one_deposit_and_one_cancellation_this_and_last_period() {
                    Test::default()
                        .target_set_last_period_one_hundred_thousand_in_five_months()
                        .add_line(mkdate(8, 1), Action::Deposit(RawAmount::yen("20000")))
                        .add_line(
                            mkdate(8, 31),
                            Action::DepositCancellation(RawAmount::yen("10000")),
                        )
                        .add_line(mkdate(9, 4), Action::Deposit(RawAmount::yen("20000")))
                        .add_line(
                            mkdate(9, 10),
                            Action::DepositCancellation(RawAmount::yen("1000")),
                        )
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("19000"),
                            current_recommended_deposit: Some(ex.yen("22500")),
                            current_actual_deposit: Some(ex.yen("19000")),
                            current_withdrawal: None,
                            total_deposit: ex.yen("29000"),
                            total_withdrawal: ex.yen("0"),
                            total: ex.yen("29000"),
                        })
                        .execute();
                }
            }
        }

        mod withdrawal_cancellation {
            use super::*;

            mod this_period_until_today {
                use super::*;

                #[test]
                fn one_today_with_deposit() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 8), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(mkdate(9, 13), Action::Withdrawal(RawAmount::yen("10000")))
                        .add_line(
                            mkdate(9, 15),
                            Action::WithdrawalCancellation(RawAmount::yen("5000")),
                        )
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("20000"),
                            current_recommended_deposit: Some(ex.yen("25000")),
                            current_actual_deposit: Some(ex.yen("25000")),
                            current_withdrawal: Some(ex.yen("5000")),
                            total_deposit: ex.yen("25000"),
                            total_withdrawal: ex.yen("5000"),
                            total: ex.yen("20000"),
                        })
                        .execute();
                }

                #[test]
                fn one_today() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 13), Action::Withdrawal(RawAmount::yen("10000")))
                        .add_line(
                            mkdate(9, 15),
                            Action::WithdrawalCancellation(RawAmount::yen("5000")),
                        )
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("-5000"),
                            current_recommended_deposit: Some(ex.yen("25000")),
                            current_actual_deposit: None,
                            current_withdrawal: Some(ex.yen("5000")),
                            total_deposit: ex.yen("0"),
                            total_withdrawal: ex.yen("5000"),
                            total: ex.yen("-5000"),
                        })
                        .execute();
                }

                #[test]
                fn one_today_withdrawal_last_period() {
                    Test::default()
                        .target_set_last_period_one_hundred_thousand_in_five_months()
                        .add_line(mkdate(8, 13), Action::Withdrawal(RawAmount::yen("10000")))
                        .add_line(
                            mkdate(9, 15),
                            Action::WithdrawalCancellation(RawAmount::yen("5000")),
                        )
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("5000"),
                            current_recommended_deposit: Some(ex.yen("25000")),
                            current_actual_deposit: None,
                            current_withdrawal: Some(ex.yen("-5000")),
                            total_deposit: ex.yen("0"),
                            total_withdrawal: ex.yen("5000"),
                            total: ex.yen("-5000"),
                        })
                        .execute();
                }

                #[test]
                fn two_this_period() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 8), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(mkdate(9, 13), Action::Withdrawal(RawAmount::yen("10000")))
                        .add_line(
                            mkdate(9, 13),
                            Action::WithdrawalCancellation(RawAmount::yen("2500")),
                        )
                        .add_line(
                            mkdate(9, 15),
                            Action::WithdrawalCancellation(RawAmount::yen("2500")),
                        )
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("20000"),
                            current_recommended_deposit: Some(ex.yen("25000")),
                            current_actual_deposit: Some(ex.yen("25000")),
                            current_withdrawal: Some(ex.yen("5000")),
                            total_deposit: ex.yen("25000"),
                            total_withdrawal: ex.yen("5000"),
                            total: ex.yen("20000"),
                        })
                        .execute();
                }

                #[test]
                fn one_this_period_withdrawal_last_period() {
                    Test::default()
                        .target_set_last_period_one_hundred_thousand_in_five_months()
                        .add_line(mkdate(8, 8), Action::Deposit(RawAmount::yen("20000")))
                        .add_line(mkdate(8, 13), Action::Withdrawal(RawAmount::yen("10000")))
                        .add_line(
                            mkdate(9, 15),
                            Action::WithdrawalCancellation(RawAmount::yen("5000")),
                        )
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("5000"),
                            current_recommended_deposit: Some(ex.yen("20000")),
                            current_actual_deposit: None,
                            current_withdrawal: Some(ex.yen("-5000")),
                            total_deposit: ex.yen("20000"),
                            total_withdrawal: ex.yen("5000"),
                            total: ex.yen("15000"),
                        })
                        .execute();
                }

                #[test]
                fn two_this_period_withdrawal_last_period() {
                    Test::default()
                        .target_set_last_period_one_hundred_thousand_in_five_months()
                        .add_line(mkdate(8, 8), Action::Deposit(RawAmount::yen("20000")))
                        .add_line(mkdate(8, 13), Action::Withdrawal(RawAmount::yen("10000")))
                        .add_line(
                            mkdate(9, 13),
                            Action::WithdrawalCancellation(RawAmount::yen("3000")),
                        )
                        .add_line(
                            mkdate(9, 15),
                            Action::WithdrawalCancellation(RawAmount::yen("2000")),
                        )
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("5000"),
                            current_recommended_deposit: Some(ex.yen("20000")),
                            current_actual_deposit: None,
                            current_withdrawal: Some(ex.yen("-5000")),
                            total_deposit: ex.yen("20000"),
                            total_withdrawal: ex.yen("5000"),
                            total: ex.yen("15000"),
                        })
                        .execute();
                }

                #[test]
                fn one_today_withdraws_too_much() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 8), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(mkdate(9, 13), Action::Withdrawal(RawAmount::yen("10000")))
                        .add_line(
                            mkdate(9, 15),
                            Action::WithdrawalCancellation(RawAmount::yen("12000")),
                        )
                        .expect_error("attempt to put back money that was not withdrawn")
                        .execute();
                }

                #[test]
                fn one_today_withdraws_too_much_but_money_is_withdrawn_again() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 8), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(mkdate(9, 10), Action::Withdrawal(RawAmount::yen("10000")))
                        .add_line(
                            mkdate(9, 12),
                            Action::WithdrawalCancellation(RawAmount::yen("12000")),
                        )
                        .add_line(mkdate(9, 14), Action::Withdrawal(RawAmount::yen("3000")))
                        .expect_error("attempt to put back money that was not withdrawn")
                        .execute();
                }
            }

            mod before_current_period {
                use super::*;

                #[test]
                fn one_cancellation() {
                    Test::default()
                        .target_set_last_period_one_hundred_thousand_in_five_months()
                        .add_line(mkdate(8, 1), Action::Deposit(RawAmount::yen("20000")))
                        .add_line(mkdate(8, 28), Action::Withdrawal(RawAmount::yen("15000")))
                        .add_line(
                            mkdate(8, 31),
                            Action::WithdrawalCancellation(RawAmount::yen("5000")),
                        )
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("20000"),
                            current_recommended_deposit: Some(ex.yen("20000")),
                            current_actual_deposit: None,
                            current_withdrawal: None,
                            total_deposit: ex.yen("20000"),
                            total_withdrawal: ex.yen("10000"),
                            total: ex.yen("10000"),
                        })
                        .execute();
                }

                #[test]
                fn two_cancellations() {
                    Test::default()
                        .target_set_last_period_one_hundred_thousand_in_five_months()
                        .add_line(mkdate(8, 1), Action::Deposit(RawAmount::yen("20000")))
                        .add_line(mkdate(8, 28), Action::Withdrawal(RawAmount::yen("15000")))
                        .add_line(
                            mkdate(8, 30),
                            Action::WithdrawalCancellation(RawAmount::yen("3000")),
                        )
                        .add_line(
                            mkdate(8, 31),
                            Action::WithdrawalCancellation(RawAmount::yen("2000")),
                        )
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("20000"),
                            current_recommended_deposit: Some(ex.yen("20000")),
                            current_actual_deposit: None,
                            current_withdrawal: None,
                            total_deposit: ex.yen("20000"),
                            total_withdrawal: ex.yen("10000"),
                            total: ex.yen("10000"),
                        })
                        .execute();
                }

                #[test]
                fn withdraws_too_much() {
                    Test::default()
                        .target_set_last_period_one_hundred_thousand_in_five_months()
                        .add_line(mkdate(8, 8), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(mkdate(8, 13), Action::Withdrawal(RawAmount::yen("10000")))
                        .add_line(
                            mkdate(8, 15),
                            Action::WithdrawalCancellation(RawAmount::yen("12000")),
                        )
                        .expect_error("attempt to put back money that was not withdrawn")
                        .execute();
                }

                #[test]
                fn withdraws_too_much_but_money_is_withdrawn_again() {
                    Test::default()
                        .target_set_last_period_one_hundred_thousand_in_five_months()
                        .add_line(mkdate(8, 8), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(mkdate(8, 10), Action::Withdrawal(RawAmount::yen("10000")))
                        .add_line(
                            mkdate(8, 12),
                            Action::WithdrawalCancellation(RawAmount::yen("12000")),
                        )
                        .add_line(mkdate(8, 14), Action::Withdrawal(RawAmount::yen("3000")))
                        .expect_error("attempt to put back money that was not withdrawn")
                        .execute();
                }
            }

            mod after_current_period {
                use super::*;
                #[test]
                fn one_withdrawal_cancellation_tomorrow() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 3), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(mkdate(9, 15), Action::Withdrawal(RawAmount::yen("5000")))
                        .add_line(mkdate(9, 16), Action::WithdrawalCancellation(RawAmount::yen("5000")))
                        .expect_bucket_recommended_commit_one_hundred_thousand_in_four_months_five_thousand_withdrawn()
                        .execute();
                }

                #[test]
                fn one_withdrawal_cancellation_this_period_after_tomorrow() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 3), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(mkdate(9, 15), Action::Withdrawal(RawAmount::yen("5000")))
                        .add_line(
                            mkdate(9, 17),
                            Action::WithdrawalCancellation(RawAmount::yen("5000")),
                        )
                        .expect_bucket_recommended_commit_one_hundred_thousand_in_four_months_five_thousand_withdrawn()
                        .execute();
                }

                #[test]
                fn one_withdrawal_cancellation_next_period() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 3), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(mkdate(9, 15), Action::Withdrawal(RawAmount::yen("5000")))
                        .add_line(
                            mkdate(10, 18),
                            Action::WithdrawalCancellation(RawAmount::yen("5000")),
                        )
                        .expect_bucket_recommended_commit_one_hundred_thousand_in_four_months_five_thousand_withdrawn()
                        .execute();
                }

                #[test]
                fn one_withdrawal_cancellation_many_period_after() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 3), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(mkdate(9, 15), Action::Withdrawal(RawAmount::yen("5000")))
                        .add_line(
                            mkdate(12, 18),
                            Action::WithdrawalCancellation(RawAmount::yen("5000")),
                        )
                        .expect_bucket_recommended_commit_one_hundred_thousand_in_four_months_five_thousand_withdrawn()
                        .execute();
                }

                #[test]
                fn many_deposits() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 3), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(mkdate(9, 15), Action::Withdrawal(RawAmount::yen("5000")))
                        .add_line(
                            mkdate(9, 16),
                            Action::WithdrawalCancellation(RawAmount::yen("1000")),
                        )
                        .add_line(
                            mkdate(9, 17),
                            Action::WithdrawalCancellation(RawAmount::yen("1000")),
                        )
                        .add_line(
                            mkdate(10, 18),
                            Action::WithdrawalCancellation(RawAmount::yen("1000")),
                        )
                        .add_line(
                            mkdate(12, 18),
                            Action::WithdrawalCancellation(RawAmount::yen("1000")),
                        )
                        .expect_bucket_recommended_commit_one_hundred_thousand_in_four_months_five_thousand_withdrawn()
                        .execute();
                }
                #[test]
                fn one_cancellation_too_big_followed_by_one_deposit_that_brings_back_the_bucket_to_positive(
                ) {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(10, 8), Action::Withdrawal(RawAmount::yen("25000")))
                        .add_line(
                            mkdate(10, 13),
                            Action::WithdrawalCancellation(RawAmount::yen("30000")),
                        )
                        .add_line(mkdate(10, 15), Action::Withdrawal(RawAmount::yen("30000")))
                        .expect_error("attempt to put back money that was not withdrawn")
                        .execute();
                }
            }
        }

        mod every_line_type {
            use super::*;

            mod this_period_until_today {
                use super::*;
                #[test]
                fn one_today() {
                    Test::default()
                        .target_set_in_current_period_one_hundred_thousand_in_four_months()
                        .add_line(mkdate(9, 8), Action::Deposit(RawAmount::yen("25000")))
                        .add_line(
                            mkdate(9, 9),
                            Action::DepositCancellation(RawAmount::yen("5000")),
                        )
                        .add_line(mkdate(9, 15), Action::Withdrawal(RawAmount::yen("5000")))
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("15000"),
                            current_recommended_deposit: Some(ex.yen("25000")),
                            current_actual_deposit: Some(ex.yen("20000")),
                            current_withdrawal: Some(ex.yen("5000")),
                            total_deposit: ex.yen("20000"),
                            total_withdrawal: ex.yen("5000"),
                            total: ex.yen("15000"),
                        })
                        .execute();
                }

                #[test]
                fn two_withdrawals_one_deposit() {
                    Test::default()
                        .add_line(mkdate(9, 8), Action::Withdrawal(RawAmount::yen("25000")))
                        .add_line(mkdate(9, 9), Action::Withdrawal(RawAmount::yen("5000")))
                        .add_line(mkdate(9, 10), Action::Deposit(RawAmount::yen("30000")))
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("0"),
                            current_recommended_deposit: None,
                            current_actual_deposit: Some(ex.yen("30000")),
                            current_withdrawal: Some(ex.yen("30000")),
                            total_deposit: ex.yen("30000"),
                            total_withdrawal: ex.yen("30000"),
                            total: ex.yen("0"),
                        })
                        .execute()
                }
            }

            mod across_periods {
                use super::*;
                #[test]
                fn last_period_cancellation_this_period_withdrawal() {
                    Test::default()
                        .target_set_last_period_one_hundred_thousand_in_five_months()
                        .add_line(mkdate(8, 1), Action::Deposit(RawAmount::yen("20000")))
                        .add_line(
                            mkdate(8, 31),
                            Action::DepositCancellation(RawAmount::yen("10000")),
                        )
                        .add_line(mkdate(9, 5), Action::Withdrawal(RawAmount::yen("3000")))
                        .expect_bucket(|ex| BucketAtDate {
                            recommended_or_actual_change: ex.yen("-3000"),
                            current_recommended_deposit: Some(ex.yen("22500")),
                            current_actual_deposit: None,
                            current_withdrawal: Some(ex.yen("3000")),
                            total_deposit: ex.yen("10000"),
                            total_withdrawal: ex.yen("3000"),
                            total: ex.yen("7000"),
                        })
                        .execute();
                }
            }
        }
    }

    mod create_operand {
        use super::*;
        use pretty_assertions::assert_eq;
        #[test]
        fn nominal() -> () {
            let ex = ExchangeRates::for_tests();
            let period_configuration =
                PeriodConfigurationVaultValue::CalendarMonth(CalendarMonthPeriodConfiguration {});
            let today = mkdate(9, 15);

            let bucket = Bucket {
                name: "test-bucket".to_string(),
                lines: vec![
                    Line((
                        mkdate(8, 13),
                        Action::SetTarget {
                            amount: RawAmount::yen("3000"),
                            target_date: mkdate(10, 30),
                        },
                    )),
                    Line((mkdate(8, 13), Action::Deposit(RawAmount::yen("1100")))),
                    Line((mkdate(8, 20), Action::Withdrawal(RawAmount::yen("500")))),
                    Line((
                        mkdate(8, 20),
                        Action::DepositCancellation(RawAmount::yen("100")),
                    )),
                    Line((mkdate(9, 15), Action::Deposit(RawAmount::yen("1000")))),
                ],
                archived_since: None,
            };

            assert_eq!(
                bucket.build(&period_configuration, &today, &ex),
                Ok(vec![Operand {
                    name: "test-bucket".to_string(),
                    amount: ex.yen("-1000"),
                    illustration: vec![
                        (
                            "This period - recommended deposit".to_string(),
                            IllustrationValue::Amount(ex.yen("1000"))
                        ),
                        (
                            "This period - actual deposit".to_string(),
                            IllustrationValue::Amount(ex.yen("1000"))
                        ),
                        (
                            "This period - actual withdrawal".to_string(),
                            IllustrationValue::Null
                        ),
                        (
                            "Deposited".to_string(),
                            IllustrationValue::Amount(ex.yen("2000"))
                        ),
                        (
                            "Withdrawn".to_string(),
                            IllustrationValue::Amount(ex.yen("500"))
                        ),
                        (
                            "Total".to_string(),
                            IllustrationValue::Amount(ex.yen("1500"))
                        )
                    ],
                    archived_from: None,
                }])
            );
        }

        #[test]
        fn withdrawal_only() -> () {
            let ex = ExchangeRates::for_tests();
            let period_configuration =
                PeriodConfigurationVaultValue::CalendarMonth(CalendarMonthPeriodConfiguration {});
            let today = mkdate(9, 15);

            let bucket = Bucket {
                name: "test-bucket".to_string(),
                lines: vec![
                    Line((
                        mkdate(8, 13),
                        Action::SetTarget {
                            amount: RawAmount::yen("3000"),
                            target_date: mkdate(10, 30),
                        },
                    )),
                    Line((mkdate(9, 14), Action::Withdrawal(RawAmount::yen("500")))),
                ],
                archived_since: None,
            };

            assert_eq!(
                bucket.build(&period_configuration, &today, &ex),
                Ok(vec![Operand {
                    name: "test-bucket".to_string(),
                    amount: ex.yen("500"),
                    illustration: vec![
                        (
                            "This period - recommended deposit".to_string(),
                            IllustrationValue::Amount(ex.yen("1500"))
                        ),
                        (
                            "This period - actual deposit".to_string(),
                            IllustrationValue::Null
                        ),
                        (
                            "This period - actual withdrawal".to_string(),
                            IllustrationValue::Amount(ex.yen("500"))
                        ),
                        (
                            "Deposited".to_string(),
                            IllustrationValue::Amount(ex.yen("0"))
                        ),
                        (
                            "Withdrawn".to_string(),
                            IllustrationValue::Amount(ex.yen("500"))
                        ),
                        (
                            "Total".to_string(),
                            IllustrationValue::Amount(ex.yen("-500"))
                        )
                    ],
                    archived_from: None,
                }])
            );
        }

        #[test]
        fn no_goal() {
            {
                let ex = ExchangeRates::for_tests();
                let period_configuration = PeriodConfigurationVaultValue::CalendarMonth(
                    CalendarMonthPeriodConfiguration {},
                );
                let today = mkdate(9, 15);

                let bucket = Bucket {
                    name: "test-bucket".to_string(),
                    lines: vec![
                        Line((mkdate(8, 13), Action::Deposit(RawAmount::yen("1100")))),
                        Line((mkdate(8, 20), Action::Withdrawal(RawAmount::yen("500")))),
                        Line((
                            mkdate(8, 20),
                            Action::DepositCancellation(RawAmount::yen("100")),
                        )),
                        Line((mkdate(9, 15), Action::Deposit(RawAmount::yen("1000")))),
                    ],
                    archived_since: None,
                };

                assert_eq!(
                    bucket.build(&period_configuration, &today, &ex),
                    Ok(vec![Operand {
                        name: "test-bucket".to_string(),
                        amount: ex.yen("-1000"),
                        illustration: vec![
                            (
                                "This period - recommended deposit".to_string(),
                                IllustrationValue::Null
                            ),
                            (
                                "This period - actual deposit".to_string(),
                                IllustrationValue::Amount(ex.yen("1000"))
                            ),
                            (
                                "This period - actual withdrawal".to_string(),
                                IllustrationValue::Null
                            ),
                            (
                                "Deposited".to_string(),
                                IllustrationValue::Amount(ex.yen("2000"))
                            ),
                            (
                                "Withdrawn".to_string(),
                                IllustrationValue::Amount(ex.yen("500"))
                            ),
                            (
                                "Total".to_string(),
                                IllustrationValue::Amount(ex.yen("1500"))
                            )
                        ],
                        archived_from: None,
                    }])
                );
            }
        }
    }

    mod vault_value_parser {
        use super::*;
        use crate::vault::VaultImpl;
        use pretty_assertions::assert_eq;
        use serde_json::{json, Value};
        use std::io::Write;
        use tempfile::TempDir;

        #[test]
        fn nominal() {
            let (_dir, vault) = VaultImpl::create_mocked_vault(buckets_json_definition(false));

            assert_eq!(
                BucketsVaultValue::from_vault(&vault),
                Ok(expected_buckets(false))
            );
        }

        #[test]
        fn archived() {
            let (_dir, vault) = VaultImpl::create_mocked_vault(buckets_json_definition(true));

            assert_eq!(
                BucketsVaultValue::from_vault(&vault),
                Ok(expected_buckets(true))
            );
        }

        fn buckets_json_definition(archived: bool) -> Value {
            let mut buckets_json_definition = json!({"buckets": [
                {
                    "name": "test-bucket",
                    "lines": [
                        "2025/08/13 TARG ¥3000 2025/10/30",
                        "2025/08/13 DEPO ¥1100 #Comment",
                        "2025/08/20 WITH ¥500",
                        "2025/08/20 DEPO- ¥100",
                        "2025/09/15 DEPO ¥1000",
                        "2025/09/15 WITH- ¥50"
                    ]
                }
            ]});
            if archived {
                buckets_json_definition
                    .as_object_mut()
                    .expect("can read JSON")["buckets"][0]["archived_since"] = "2025-10-03".into();
            }
            buckets_json_definition
        }

        fn expected_buckets(archived: bool) -> Vec<Bucket> {
            let expected_bucket = vec![Bucket {
                name: "test-bucket".to_string(),
                lines: vec![
                    Line((
                        mkdate(8, 13),
                        Action::SetTarget {
                            amount: RawAmount::yen("3000"),
                            target_date: mkdate(10, 30),
                        },
                    )),
                    Line((mkdate(8, 13), Action::Deposit(RawAmount::yen("1100")))),
                    Line((mkdate(8, 20), Action::Withdrawal(RawAmount::yen("500")))),
                    Line((
                        mkdate(8, 20),
                        Action::DepositCancellation(RawAmount::yen("100")),
                    )),
                    Line((mkdate(9, 15), Action::Deposit(RawAmount::yen("1000")))),
                    Line((
                        mkdate(9, 15),
                        Action::WithdrawalCancellation(RawAmount::yen("50")),
                    )),
                ],
                archived_since: if archived {
                    Some(NaiveDate::from_ymd_opt(2025, 10, 3).expect("can create date"))
                } else {
                    None
                },
            }];
            expected_bucket
        }
    }
}
