use crate::amounts::exchange_rates::ExchangeRates;
use crate::amounts::{Add, Amount, Div, Figure, Minus, RawAmount};
use crate::period::{
    ErrorPeriodsBetween, Period, PeriodConfigurationVaultValue, PeriodsConfiguration,
};
use crate::remaining_operation::core_types::{GroupBuilder, IllustrationValue, Operand, OperandBuilder};
use crate::vault::VaultReadable;
use chrono::format::parse;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::de::{Error, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::value::Index;
use std::fmt::{Formatter, Write};
use std::str::{FromStr, Split};

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
}


#[derive(Debug, Eq, PartialEq, Clone)]
struct Line((NaiveDate, Action));

impl<'de> Deserialize<'de> for Line {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct LineVisitor;
        impl LineVisitor {
            fn parse_amount<E: Error>(line: &mut Split<&str>) -> Result<RawAmount, E> {
                let raw_amount_str = line.next().ok_or(Error::custom("No amounts specified"))?;

                let mut raw_amount_str_itr = raw_amount_str.chars();
                let sign = raw_amount_str_itr
                    .next()
                    .map(|sign| sign.to_string())
                    .ok_or(Error::custom("amount is too short"))?;

                let figure_raw: String = raw_amount_str_itr.collect();
                let figure = Decimal::from_str_exact(&figure_raw).map_err(|err| {
                    Error::custom(format!(
                        "Error parsing amount: {}. Error: {}",
                        figure_raw, err
                    ))
                })?;

                Ok(RawAmount { sign, figure })
            }
        }

        impl<'de> Visitor<'de> for LineVisitor {
            type Value = Line;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("a line")
            }

            fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
                // TODO rewrite with nom
                let mut line = v.split(" ");
                let raw_date = line
                    .next()
                    .ok_or(Error::custom("Could not find the date"))?;
                let date = NaiveDate::parse_from_str(raw_date, "%Y/%m/%d").map_err(|err| {
                    Error::custom(format!(
                        "Failed to parse date: {}. Error: {}",
                        raw_date, err
                    ))
                })?;

                let tag = line.next().ok_or(Error::custom("No tag specified"))?;
                let line_data = match tag {
                    "TARG" => {
                        let raw_amount = LineVisitor::parse_amount(&mut line)?;

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
                    "DEPO" => Ok(Action::Deposit(LineVisitor::parse_amount(&mut line)?)),
                    "DEPO-" => Ok(Action::DepositCancellation(LineVisitor::parse_amount(
                        &mut line,
                    )?)),
                    "WITH" => Ok(Action::Withdrawal(LineVisitor::parse_amount(&mut line)?)),
                    "WITH-" => Ok(Action::WithdrawalCancellation(LineVisitor::parse_amount(
                        &mut line,
                    )?)),
                    _ => Err(Error::custom("Unknown tag")),
                }?;

                Ok(Line((date, line_data)))
            }
        }

        deserializer.deserialize_str(LineVisitor)
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
    current_recommended_deposit: Amount,
    current_actual_deposit: Option<Amount>,
    current_withdrawal: Option<Amount>,
    total_deposit: Amount,
    total_withdrawal: Amount,
    total: Amount,
}

impl Bucket {
    // TODO make the buckets truly multi currency (replace any "JPY")

    /* TODO Idea for a refactor

    ChronoStackWalker. It takes an implementation of the ChronoStackVisitor. It also takes a list of
    CalendarEntry, a (date, T) tuple.
    When executed, it calls visitor.visit(date, obj: T) for a slice of the CalendarEntry list.

    The slice is configurable:
    - CalendarEntries in in a period
    - CalendarEntries up until a period
    - All calendar entries

    It verifies if entries are in order (over the whole stack - even if it won't call visitor on it)

    Another object, the BucketChronoStackWalker is built upon the ChronoStack walker.
    Features:
    - it filters what type of lines are passed in

    BEFORE STARTING TO IMPLEMENT, DO NOT FORGET TO UNCOMMENT THE TESTS THAT ARE CURRENTLY COMMENTED OUT WAITING FOR THIS REFACTOR
    */

    fn for_period(
        &self,
        period_config: &PeriodConfigurationVaultValue,
        date: &NaiveDate,
        ex: &ExchangeRates,
    ) -> Result<BucketAtDate, String> {
        let (target_amount, target_date) = self
            .lines
            .iter()
            .find_map(|Line((_, line))| match line {
                Action::SetTarget {
                    amount,
                    target_date,
                } => Some((amount, target_date)),
                _ => None,
            })
            .ok_or("No target for bucket".to_string())?;
        let target_amount = ex.new_amount_from_raw_amount(&target_amount)?;

        let total = self.lines.iter().try_fold(
            ex.zero(&"JPY".to_string())?,
            |acc, Line((line_date, action))| {
                if line_date <= date {
                    match action {
                        Action::Deposit(amount) | Action::WithdrawalCancellation(amount) => ex
                            .new_amount_from_raw_amount(amount)
                            .map(|parsed_amount| acc.add(&parsed_amount)),
                        Action::Withdrawal(amount) => ex
                            .new_amount_from_raw_amount(amount)
                            .map(|parsed_amount| acc.minus(&parsed_amount)),
                        Action::DepositCancellation(amount) => ex
                            .new_amount_from_raw_amount(amount)
                            .map(|parsed_amount| acc.minus(&parsed_amount))
                            .and_then(|new_acc| {
                                if new_acc.is_negative() {
                                    Err("attempt to withdraw more money than the Bucket contains"
                                        .to_string())
                                } else {
                                    Ok(new_acc)
                                }
                            }),
                        _ => Ok(acc),
                    }
                } else {
                    Ok(acc)
                }
            },
        )?;

        let deposited = self.lines.iter().try_fold(
            ex.zero(&"JPY".to_string())?,
            |acc, Line((line_date, action))| {
                if line_date <= date {
                    match action {
                        Action::Deposit(amount) => ex
                            .new_amount_from_raw_amount(amount)
                            .map(|parsed_amount| acc.add(&parsed_amount)),
                        Action::DepositCancellation(amount) => ex
                            .new_amount_from_raw_amount(amount)
                            .map(|parsed_amount| acc.minus(&parsed_amount)),
                        _ => Ok(acc),
                    }
                } else {
                    Ok(acc)
                }
            },
        )?;

        let withdrawn = self.lines.iter().try_fold(
            ex.zero(&"JPY".to_string())?,
            |acc, Line((line_date, action))| {
                if line_date <= date {
                    match action {
                        Action::Withdrawal(amount) => ex
                            .new_amount_from_raw_amount(amount)
                            .map(|parsed_amount| acc.add(&parsed_amount)),
                        Action::WithdrawalCancellation(amount) => ex
                            .new_amount_from_raw_amount(amount)
                            .map(|parsed_amount| acc.minus(&parsed_amount))
                            .and_then(|new_acc| {
                                if new_acc.is_negative() {
                                    Err("attempt to put back money that was not withdrawn"
                                        .to_string())
                                } else {
                                    Ok(new_acc)
                                }
                            }),
                        _ => Ok(acc),
                    }
                } else {
                    Ok(acc)
                }
            },
        )?;

        let current_period = period_config.period_for_date(date)?;

        let deposited_until_period_start = self.lines.iter().try_fold(
            ex.zero(&"JPY".to_string())?,
            |acc, Line((line_date, action))| {
                if line_date < &current_period.start_date {
                    match action {
                        // Withdrawals should never count toward what was deposited
                        Action::Deposit(amount) => ex
                            .new_amount_from_raw_amount(amount)
                            .map(|parsed_amount| acc.add(&parsed_amount)),
                        Action::DepositCancellation(amount) => ex
                            .new_amount_from_raw_amount(amount)
                            .map(|parsed_amount| acc.minus(&parsed_amount)),
                        _ => Ok(acc),
                    }
                } else {
                    Ok(acc)
                }
            },
        )?;

        let total_this_period =
            self.lines
                .iter()
                .try_fold(None, |acc, Line((line_date, action))| {
                    if line_date >= &current_period.start_date && line_date <= date {
                        match action {
                            Action::Deposit(amount) | Action::WithdrawalCancellation(amount) => {
                                let acc = acc.unwrap_or(ex.zero(&"JPY".to_string())?);
                                ex.new_amount_from_raw_amount(amount)
                                    .map(|parsed_amount| Some(acc.add(&parsed_amount)))
                            }
                            Action::DepositCancellation(amount) | Action::Withdrawal(amount) => {
                                let acc = acc.unwrap_or(ex.zero(&"JPY".to_string())?);
                                ex.new_amount_from_raw_amount(amount)
                                    .map(|parsed_amount| Some(acc.minus(&parsed_amount)))
                            }
                            _ => Ok(acc),
                        }
                    } else {
                        Ok(acc)
                    }
                })?;

        let deposited_this_period =
            self.lines
                .iter()
                .try_fold(None, |acc, Line((line_date, action))| {
                    if line_date >= &current_period.start_date && line_date <= date {
                        match action {
                            Action::Deposit(amount) => {
                                let acc = acc.unwrap_or(ex.zero(&"JPY".to_string())?);
                                ex.new_amount_from_raw_amount(amount)
                                    .map(|parsed_amount| Some(acc.add(&parsed_amount)))
                            }
                            Action::DepositCancellation(amount) => {
                                let acc = acc.unwrap_or(ex.zero(&"JPY".to_string())?);
                                ex.new_amount_from_raw_amount(amount)
                                    .map(|parsed_amount| Some(acc.minus(&parsed_amount)))
                            }
                            _ => Ok(acc),
                        }
                    } else {
                        Ok(acc)
                    }
                })?;

        let withdrawned_this_period =
            self.lines
                .iter()
                .try_fold(None, |acc, Line((line_date, action))| {
                    if line_date >= &current_period.start_date && line_date <= date {
                        match action {
                            Action::Withdrawal(amount) => {
                                let acc = acc.unwrap_or(ex.zero(&"JPY".to_string())?);
                                ex.new_amount_from_raw_amount(amount)
                                    .map(|parsed_amount| Some(acc.add(&parsed_amount)))
                            }
                            Action::WithdrawalCancellation(amount) => {
                                let acc = acc.unwrap_or(ex.zero(&"JPY".to_string())?);
                                ex.new_amount_from_raw_amount(amount)
                                    .map(|parsed_amount| Some(acc.minus(&parsed_amount)))
                            }
                            _ => Ok(acc),
                        }
                    } else {
                        Ok(acc)
                    }
                })?;

        let number_of_periods = match period_config.periods_between(date, target_date) {
            Ok(nb) => nb,
            Err(ErrorPeriodsBetween::EndBeforeStart) => 1,
            any => any?,
        };

        let recommended_deposit_figure = Amount::maximum(
            &target_amount.minus(&deposited_until_period_start),
            &ex.zero(&"JPY".to_string())?,
        )
        .div(&Decimal::from(number_of_periods));

        Ok(BucketAtDate {
            recommended_or_actual_change: total_this_period
                .clone()
                .unwrap_or(recommended_deposit_figure.clone()),
            current_recommended_deposit: recommended_deposit_figure,
            current_actual_deposit: deposited_this_period,
            current_withdrawal: withdrawned_this_period,
            total_deposit: deposited,
            total_withdrawal: withdrawn,
            total,
        })
    }
}

impl OperandBuilder for Bucket {
    fn build(
        self,
        period_configuration: &PeriodConfigurationVaultValue,
        today: &NaiveDate,
        exchange_rates: &ExchangeRates,
    ) -> Result<Option<Operand>, String> {
        let period = self.for_period(period_configuration, today, exchange_rates)?;
        Ok(Some(Operand {
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
        }))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::period::CalendarMonthPeriodConfiguration;
    use crate::vault::VaultImpl;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    fn mkdate(month: u32, date: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2025, month, date).expect("Can create date")
    }

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

        pub fn target_set_in_current_period_one_hundred_thousand_in_four_months(mut self) -> Self {
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
                current_recommended_deposit: ex.yen("25000"),
                current_actual_deposit: None,
                current_withdrawal: None,
                total_deposit: ex.yen("0"),
                total_withdrawal: ex.yen("0"),
                total: ex.yen("0"),
            })
        }

        pub fn expect_bucket_recommended_commit_one_hundred_thousand_in_four_months(self) -> Self {
            self.expect_bucket(|ex| BucketAtDate {
                recommended_or_actual_change: ex.yen("25000"),
                current_recommended_deposit: ex.yen("25000"),
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
                current_recommended_deposit: ex.yen("25000"),
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
            let period_configuration =
                PeriodConfigurationVaultValue::CalendarMonth(CalendarMonthPeriodConfiguration {});
            let today = mkdate(9, 15);

            let bucket = Bucket {
                name: "test bucket inner".to_string(),
                lines: self.lines.clone(),
            };

            assert_eq!(
                bucket.for_period(&period_configuration, &today, &ex),
                (self.expected)(&ex)
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

    mod incorrect_configuration {
        use super::*;

        #[test]
        fn one_deposit_but_no_target() {
            Test::default()
                .add_line(mkdate(9, 15), Action::Deposit(RawAmount::yen("10000")))
                .expect_error("No target for bucket")
                .execute()
        }

        #[test]
        fn no_lines() {
            Test::default()
                .expect_error("No target for bucket")
                .execute()
        }
    }

    mod target_setting {
        use super::*;

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
                    current_recommended_deposit: ex.yen("100000"),
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
                    current_recommended_deposit: ex.yen("100000"),
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
                    current_recommended_deposit: ex.yen("50000"),
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
                    current_recommended_deposit: ex.yen("33333.33"),
                    current_actual_deposit: None,
                    current_withdrawal: None,
                    total_deposit: ex.yen("0"),
                    total_withdrawal: ex.yen("0"),
                    total: ex.yen("0"),
                })
                .execute()
        }
    }

    mod deposits {
        use super::*;

        mod this_period_until_today {
            use super::*;

            #[test]
            fn one_deposit_today__partial() {
                Test::default()
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(9, 15), Action::Deposit(RawAmount::yen("10000")))
                    .expect_bucket(|ex| BucketAtDate {
                        recommended_or_actual_change: ex.yen("10000"),
                        current_recommended_deposit: ex.yen("25000"),
                        current_actual_deposit: Some(ex.yen("10000")),
                        current_withdrawal: None,
                        total_deposit: ex.yen("10000"),
                        total_withdrawal: ex.yen("0"),
                        total: ex.yen("10000"),
                    })
                    .execute();
            }

            #[test]
            fn one_deposit_today__zero() {
                Test::default()
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(9, 15), Action::Deposit(RawAmount::yen("0")))
                    .expect_bucket(|ex| BucketAtDate {
                        recommended_or_actual_change: ex.yen("0"),
                        current_recommended_deposit: ex.yen("25000"),
                        current_actual_deposit: Some(ex.yen("0")),
                        current_withdrawal: None,
                        total_deposit: ex.yen("0"),
                        total_withdrawal: ex.yen("0"),
                        total: ex.yen("0"),
                    })
                    .execute();
            }

            #[test]
            fn two_deposits__recommended() {
                Test::default()
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(9, 3), Action::Deposit(RawAmount::yen("10000")))
                    .add_line(mkdate(9, 5), Action::Deposit(RawAmount::yen("15000")))
                    .expect_bucket(|ex| BucketAtDate {
                        recommended_or_actual_change: ex.yen("25000"),
                        current_recommended_deposit: ex.yen("25000"),
                        current_actual_deposit: Some(ex.yen("25000")),
                        current_withdrawal: None,
                        total_deposit: ex.yen("25000"),
                        total_withdrawal: ex.yen("0"),
                        total: ex.yen("25000"),
                    })
                    .execute();
            }

            #[test]
            fn two_deposits_same_day() {
                Test::default()
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(9, 5), Action::Deposit(RawAmount::yen("10000")))
                    .add_line(mkdate(9, 5), Action::Deposit(RawAmount::yen("15000")))
                    .expect_bucket(|ex| BucketAtDate {
                        recommended_or_actual_change: ex.yen("25000"),
                        current_recommended_deposit: ex.yen("25000"),
                        current_actual_deposit: Some(ex.yen("25000")),
                        current_withdrawal: None,
                        total_deposit: ex.yen("25000"),
                        total_withdrawal: ex.yen("0"),
                        total: ex.yen("25000"),
                    })
                    .execute();
            }

            #[test]
            fn one_deposit_period_start__partial() {
                Test::default()
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(9, 1), Action::Deposit(RawAmount::yen("10000")))
                    .expect_bucket(|ex| BucketAtDate {
                        recommended_or_actual_change: ex.yen("10000"),
                        current_recommended_deposit: ex.yen("25000"),
                        current_actual_deposit: Some(ex.yen("10000")),
                        current_withdrawal: None,
                        total_deposit: ex.yen("10000"),
                        total_withdrawal: ex.yen("0"),
                        total: ex.yen("10000"),
                    })
                    .execute();
            }

            #[test]
            fn one_deposit_before_today__partial() {
                Test::default()
                    .target_set_last_period_one_hundred_thousand_in_five_months()
                    .add_line(mkdate(9, 3), Action::Deposit(RawAmount::yen("10000")))
                    .expect_bucket(|ex| BucketAtDate {
                        recommended_or_actual_change: ex.yen("10000"),
                        current_recommended_deposit: ex.yen("25000"), // This is correct. Even if the target was set for five months, there was no deposit last month
                        current_actual_deposit: Some(ex.yen("10000")),
                        current_withdrawal: None,
                        total_deposit: ex.yen("10000"),
                        total_withdrawal: ex.yen("0"),
                        total: ex.yen("10000"),
                    })
                    .execute();
            }

            #[test]
            fn one_deposit_before_today__over() {
                Test::default()
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(9, 3), Action::Deposit(RawAmount::yen("30000")))
                    .expect_bucket(|ex| BucketAtDate {
                        recommended_or_actual_change: ex.yen("30000"),
                        current_recommended_deposit: ex.yen("25000"),
                        current_actual_deposit: Some(ex.yen("30000")),
                        current_withdrawal: None,
                        total_deposit: ex.yen("30000"),
                        total_withdrawal: ex.yen("0"),
                        total: ex.yen("30000"),
                    })
                    .execute();
            }
        }
        mod before_current_period {
            use super::*;

            #[test]
            fn one_deposit__recommended() {
                Test::default()
                    .target_set_last_period_one_hundred_thousand_in_five_months()
                    .add_line(mkdate(8, 31), Action::Deposit(RawAmount::yen("20000")))
                    .expect_bucket(|ex| BucketAtDate {
                        recommended_or_actual_change: ex.yen("20000"),
                        current_recommended_deposit: ex.yen("20000"),
                        current_actual_deposit: None,
                        current_withdrawal: None,
                        total_deposit: ex.yen("20000"),
                        total_withdrawal: ex.yen("0"),
                        total: ex.yen("20000"),
                    })
                    .execute();
            }

            #[test]
            fn two_deposits__partial() {
                Test::default()
                    .target_set_last_period_one_hundred_thousand_in_five_months()
                    .add_line(mkdate(8, 15), Action::Deposit(RawAmount::yen("5000")))
                    .add_line(mkdate(8, 31), Action::Deposit(RawAmount::yen("5000")))
                    .expect_bucket(|ex| BucketAtDate {
                        recommended_or_actual_change: ex.yen("22500"),
                        current_recommended_deposit: ex.yen("22500"),
                        current_actual_deposit: None,
                        current_withdrawal: None,
                        total_deposit: ex.yen("10000"),
                        total_withdrawal: ex.yen("0"),
                        total: ex.yen("10000"),
                    })
                    .execute();
            }

            #[test]
            fn two_deposits_same_day() {
                Test::default()
                    .target_set_last_period_one_hundred_thousand_in_five_months()
                    .add_line(mkdate(8, 31), Action::Deposit(RawAmount::yen("5000")))
                    .add_line(mkdate(8, 31), Action::Deposit(RawAmount::yen("5000")))
                    .expect_bucket(|ex| BucketAtDate {
                        recommended_or_actual_change: ex.yen("22500"),
                        current_recommended_deposit: ex.yen("22500"),
                        current_actual_deposit: None,
                        current_withdrawal: None,
                        total_deposit: ex.yen("10000"),
                        total_withdrawal: ex.yen("0"),
                        total: ex.yen("10000"),
                    })
                    .execute();
            }

            #[test]
            fn many_periods_ago() {
                Test::default()
                    .target_set_many_periods_ago_twelve_hundred_in_twelve_months()
                    .add_line(mkdate(1, 15), Action::Deposit(RawAmount::yen("100")))
                    .add_line(mkdate(2, 28), Action::Deposit(RawAmount::yen("100")))
                    .expect_bucket(|ex| BucketAtDate {
                        recommended_or_actual_change: ex.yen("250"),
                        current_recommended_deposit: ex.yen("250"),
                        current_actual_deposit: None,
                        current_withdrawal: None,
                        total_deposit: ex.yen("200"),
                        total_withdrawal: ex.yen("0"),
                        total: ex.yen("200"),
                    })
                    .execute();
            }

            #[test]
            fn one_deposit__over_recommendation() {
                Test::default()
                    .target_set_last_period_one_hundred_thousand_in_five_months()
                    .add_line(mkdate(8, 31), Action::Deposit(RawAmount::yen("60000")))
                    .expect_bucket(|ex| BucketAtDate {
                        recommended_or_actual_change: ex.yen("10000"),
                        current_recommended_deposit: ex.yen("10000"),
                        current_actual_deposit: None,
                        current_withdrawal: None,
                        total_deposit: ex.yen("60000"),
                        total_withdrawal: ex.yen("0"),
                        total: ex.yen("60000"),
                    })
                    .execute();
            }

            #[test]
            fn one_deposit__over_target() {
                Test::default()
                    .target_set_last_period_one_hundred_thousand_in_five_months()
                    .add_line(mkdate(8, 31), Action::Deposit(RawAmount::yen("200000")))
                    .expect_bucket(|ex| BucketAtDate {
                        recommended_or_actual_change: ex.yen("0"),
                        current_recommended_deposit: ex.yen("0"),
                        current_actual_deposit: None,
                        current_withdrawal: None,
                        total_deposit: ex.yen("200000"),
                        total_withdrawal: ex.yen("0"),
                        total: ex.yen("200000"),
                    })
                    .execute();
            }
        }
        mod after_current_period {
            use crate::amounts::RawAmount;
            use crate::buckets::test::{mkdate, Test};
            use crate::buckets::{Action, BucketAtDate};

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
                            current_recommended_deposit: ex.yen("25000"),
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
                        current_recommended_deposit: ex.yen("260"),
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
                        current_recommended_deposit: ex.yen("260"),
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
                        current_recommended_deposit: ex.yen("25000"),
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
                        current_recommended_deposit: ex.yen("20000"),
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
                        current_recommended_deposit: ex.yen("20000"),
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
                        current_recommended_deposit: ex.yen("25000"),
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
                    .add_line(mkdate(9, 15), Action::DepositCancellation(RawAmount::yen("10000")))
                    .expect_bucket(|ex| BucketAtDate {
                        recommended_or_actual_change: ex.yen("15000"),
                        current_recommended_deposit: ex.yen("25000"),
                        current_actual_deposit: Some(ex.yen("15000")),
                        current_withdrawal: None,
                        total_deposit: ex.yen("15000"),
                        total_withdrawal: ex.yen("0"),
                        total: ex.yen("15000"),
                    })
                    .execute();
            }

            #[test]
            fn one_today__cancels_too_much() {
                Test::default()
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(9, 8), Action::Deposit(RawAmount::yen("25000")))
                    .add_line(
                        mkdate(9, 15),
                        Action::DepositCancellation(RawAmount::yen("30000")),
                    )
                    .expect_error("attempt to withdraw more money than the Bucket contains")
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
                        current_recommended_deposit: ex.yen("25000"),
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
                    .expect_error("attempt to withdraw more money than the Bucket contains")
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
                        current_recommended_deposit: ex.yen("22500"),
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
                        current_recommended_deposit: ex.yen("22500"),
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
                    .add_line(mkdate(8, 15), Action::DepositCancellation(RawAmount::yen("10000")))
                    .expect_bucket(|ex| BucketAtDate {
                        recommended_or_actual_change: ex.yen("21250"),
                        current_recommended_deposit: ex.yen("21250"),
                        current_actual_deposit: None,
                        current_withdrawal: None,
                        total_deposit: ex.yen("15000"),
                        total_withdrawal: ex.yen("0"),
                        total: ex.yen("15000"),
                    })
                    .execute();
            }

            #[test]
            fn one_today__too_cancels_too_much() {
                Test::default()
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(8, 8), Action::Deposit(RawAmount::yen("25000")))
                    .add_line(
                        mkdate(8, 15),
                        Action::DepositCancellation(RawAmount::yen("30000")),
                    )
                    .expect_error("attempt to withdraw more money than the Bucket contains")
                    .execute();
            }

            #[test]
            fn one_cancellation_too_big_followed_by_one_deposit_that_brings_back_the_bucket_to_positive(
            ) {
                Test::default()
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(8, 8), Action::Deposit(RawAmount::yen("25000")))
                    .add_line(
                        mkdate(8, 13),
                        Action::DepositCancellation(RawAmount::yen("30000")),
                    )
                    .add_line(mkdate(8, 15), Action::Deposit(RawAmount::yen("30000")))
                    .expect_error("attempt to withdraw more money than the Bucket contains")
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
                        Action::DepositCancellation(RawAmount::yen("25000")),
                    )
                    .add_line(
                        mkdate(9, 17),
                        Action::DepositCancellation(RawAmount::yen("25000")),
                    )
                    .add_line(
                        mkdate(10, 18),
                        Action::DepositCancellation(RawAmount::yen("25000")),
                    )
                    .add_line(
                        mkdate(12, 18),
                        Action::DepositCancellation(RawAmount::yen("25000")),
                    )
                    .expect_bucket_recommended_commit_one_hundred_thousand_in_four_months()
                    .execute();
            }
            /*
            #[test]
            fn one_cancellation_too_big_followed_by_one_deposit_that_brings_back_the_bucket_to_positive() {
                Test::default()
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(10, 8), Line::Deposit(RawAmount::yen("25000")))
                    .add_line(mkdate(10, 13), Line::DepositCancellation(RawAmount::yen("30000")))
                    .add_line(mkdate(10, 15), Line::Deposit(RawAmount::yen("30000")))
                    .expect_error("attempt to withdraw more money than the Bucket contains")
                    .execute();
            }

             */
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
                            current_recommended_deposit: ex.yen("25000"),
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
                        current_recommended_deposit: ex.yen("22500"),
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
                        current_recommended_deposit: ex.yen("22500"),
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
            fn one_today() {
                Test::default()
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(9, 8), Action::Deposit(RawAmount::yen("25000")))
                    .add_line(mkdate(9, 15), Action::Withdrawal(RawAmount::yen("10000")))
                    .expect_bucket(|ex| BucketAtDate {
                        recommended_or_actual_change: ex.yen("15000"),
                        current_recommended_deposit: ex.yen("25000"),
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
                        current_recommended_deposit: ex.yen("20000"),
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
                        current_recommended_deposit: ex.yen("20000"),
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
                        current_recommended_deposit: ex.yen("25000"),
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
                        current_recommended_deposit: ex.yen("25000"),
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
                        current_recommended_deposit: ex.yen("25000"),
                        current_actual_deposit: Some(ex.yen("25000")),
                        current_withdrawal: Some(ex.yen("10000")),
                        total_deposit: ex.yen("25000"),
                        total_withdrawal: ex.yen("10000"),
                        total: ex.yen("15000"),
                    })
                    .execute();
            }

            #[test]
            fn one_today__withdraw_too_much() {
                Test::default()
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(9, 8), Action::Deposit(RawAmount::yen("25000")))
                    .add_line(mkdate(9, 15), Action::Withdrawal(RawAmount::yen("30000")))
                    .expect_bucket(|ex| BucketAtDate {
                        recommended_or_actual_change: ex.yen("-5000"),
                        current_recommended_deposit: ex.yen("25000"),
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
                        current_recommended_deposit: ex.yen("25000"),
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
                        current_recommended_deposit: ex.yen("25000"),
                        current_actual_deposit: Some(ex.yen("55000")),
                        current_withdrawal: Some(ex.yen("30000")),
                        total_deposit: ex.yen("55000"),
                        total_withdrawal: ex.yen("30000"),
                        total: ex.yen("25000")
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
                        current_recommended_deposit: ex.yen("20000"),
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
                        current_recommended_deposit: ex.yen("20000"),
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
                        current_recommended_deposit: ex.yen("20000"),
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
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(8, 8), Action::Deposit(RawAmount::yen("20000")))
                    .add_line(mkdate(8, 15), Action::Withdrawal(RawAmount::yen("30000")))
                    .expect_bucket(|ex| BucketAtDate {
                        recommended_or_actual_change: ex.yen("20000"),
                        current_recommended_deposit: ex.yen("20000"),
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
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(8, 8), Action::Deposit(RawAmount::yen("25000")))
                    .add_line(
                        mkdate(8, 13),
                        Action::DepositCancellation(RawAmount::yen("30000")),
                    )
                    .add_line(mkdate(8, 15), Action::Deposit(RawAmount::yen("30000")))
                    .expect_error("attempt to withdraw more money than the Bucket contains")
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
            /*
            #[test]
            fn one_cancellation_too_big_followed_by_one_deposit_that_brings_back_the_bucket_to_positive() {
                Test::default()
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(10, 8), Line::Deposit(RawAmount::yen("25000")))
                    .add_line(mkdate(10, 13), Line::Withdrawal(RawAmount::yen("30000")))
                    .add_line(mkdate(10, 15), Line::Deposit(RawAmount::yen("30000")))
                    .expect_error("attempt to withdraw more money than the Bucket contains")
                    .execute();
            }

             */
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
                            current_recommended_deposit: ex.yen("25000"),
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
                        current_recommended_deposit: ex.yen("22500"),
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
                        current_recommended_deposit: ex.yen("22500"),
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
            fn one_today() {
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
                        current_recommended_deposit: ex.yen("25000"),
                        current_actual_deposit: Some(ex.yen("25000")),
                        current_withdrawal: Some(ex.yen("5000")),
                        total_deposit: ex.yen("25000"),
                        total_withdrawal: ex.yen("5000"),
                        total: ex.yen("20000"),
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
                        current_recommended_deposit: ex.yen("25000"),
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
                        current_recommended_deposit: ex.yen("20000"),
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
                        current_recommended_deposit: ex.yen("20000"),
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
                        current_recommended_deposit: ex.yen("20000"),
                        current_actual_deposit: None,
                        current_withdrawal: None,
                        total_deposit: ex.yen("20000"),
                        total_withdrawal: ex.yen("10000"),
                        total: ex.yen("10000"),
                    })
                    .execute();
            }

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
                        current_recommended_deposit: ex.yen("20000"),
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
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
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
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
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
                        current_recommended_deposit: ex.yen("25000"),
                        current_actual_deposit: Some(ex.yen("20000")),
                        current_withdrawal: Some(ex.yen("5000")),
                        total_deposit: ex.yen("20000"),
                        total_withdrawal: ex.yen("5000"),
                        total: ex.yen("15000"),
                    })
                    .execute();
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
                        current_recommended_deposit: ex.yen("22500"),
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

    #[test]
    fn create_operand() -> () {
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
        };

        assert_eq!(
            bucket.build(&period_configuration, &today, &ex),
            Ok(Some(Operand {
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
                        IllustrationValue::NullAmount
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
                ]
            }))
        );
    }

    #[test]
    fn create_operand_withdrawal_only() -> () {
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
        };

        assert_eq!(
            bucket.build(&period_configuration, &today, &ex),
            Ok(Some(Operand {
                name: "test-bucket".to_string(),
                amount: ex.yen("500"),
                illustration: vec![
                    (
                        "This period - recommended deposit".to_string(),
                        IllustrationValue::Amount(ex.yen("1500"))
                    ),
                    (
                        "This period - actual deposit".to_string(),
                        IllustrationValue::NullAmount
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
                ]
            }))
        );
    }

    mod vault_value_parser {
        use super::*;
        use pretty_assertions::assert_eq;
        use std::io::Write;
        use tempfile::TempDir;

        #[test]
        fn nominal() {
            let (_dir, vault) = VaultImpl::create_mocked_vault(json!({"buckets": [
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
            ]}));

            assert_eq!(
                BucketsVaultValue::from_vault(&vault),
                Ok(vec![Bucket {
                    name: "test-bucket".to_string(),
                    lines: vec![
                        Line((
                            mkdate(8, 13),
                            Action::SetTarget {
                                amount: RawAmount::yen("3000"),
                                target_date: mkdate(10, 30)
                            }
                        )),
                        Line((mkdate(8, 13), Action::Deposit(RawAmount::yen("1100")))),
                        Line((mkdate(8, 20), Action::Withdrawal(RawAmount::yen("500")))),
                        Line((
                            mkdate(8, 20),
                            Action::DepositCancellation(RawAmount::yen("100"))
                        )),
                        Line((mkdate(9, 15), Action::Deposit(RawAmount::yen("1000")))),
                        Line((
                            mkdate(9, 15),
                            Action::WithdrawalCancellation(RawAmount::yen("50"))
                        ))
                    ]
                }])
            );
        }
    }
}
