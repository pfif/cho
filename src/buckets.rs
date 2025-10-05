use crate::amounts::exchange_rates::ExchangeRates;
use crate::amounts::{Amount, Figure, RawAmount};
use crate::period::{
    ErrorPeriodsBetween, Period, PeriodConfigurationVaultValue, PeriodsConfiguration,
};
use crate::remaining_operation::core_types::{Operand, OperandBuilder};
use crate::vault::VaultReadable;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::Deserialize;
use serde_json::value::Index;

pub type BucketsVaultValue = Vec<Bucket>;
impl VaultReadable for BucketsVaultValue {
    const KEY: &'static str = "buckets";
}

#[derive(Deserialize)]
pub struct Bucket {
    lines: Vec<(NaiveDate, Line)>,
}

#[derive(Deserialize, Clone)]
#[serde(tag = "action")]
enum Line {
    DepositCancellation(RawAmount),
    Deposit(RawAmount),
    SetTarget {
        amount: RawAmount,
        target_date: NaiveDate,
    },
}

/// Amounts to deposit
// TODO rename to "BucketForDate"
#[derive(Debug, Eq, PartialEq)]
pub struct BucketThisPeriod {
    // TODO this name is bad
    recommended_or_actual_change: Amount,
    current_recommended_deposit: Amount,
    current_actual_deposit: Option<Amount>,
    total_deposit: Amount,
}

impl Bucket {
    // TODO make the buckets truly multi currency (replace any "JPY")

    /* TODO (before merging this PR) Idea for a refactor

    ChronoStackWalker. It takes an implementation of the ChronoStackVisitor. It also takes a list of
    CalendarEntry, a (date, T) tuple.
    When executed, it calls visitor.visit(date, obj: T) for a slice of the CalendarEntry list.

    The slice is configurable:
    - CalendarEntries in in a period
    - CalendarEntries up until a period
    - All calendar entries

    It verifies if entries are in order

    Another object, the BucketChronoStackWalker is built upon the ChronoStack walker.
    Features:
    - it filters what type of lines are passed in
    - after filtering, and it fails if it returns two line for the same date

    BEFORE STARTING TO IMPLEMENT, DO NOT FORGET TO UNCOMMENT THE TESTS THAT ARE CURRENTLY COMMENTED OUT WAITING FOR THIS REFACTOR

    Problem - where should it be tested? At its own level, or at the level of its callers?
    */

    /* TODO (before merging this PR?) idea for a refactor:
            Isolate the aggregation of Lines for a period (all DepositCancelation this period until today, all Deposits until period start ...)
            Refactor tests so that they are done in isolation
     */
    fn for_period(
        &self,
        period_config: &PeriodConfigurationVaultValue,
        date: &NaiveDate,
        ex: &ExchangeRates,
    ) -> Result<BucketThisPeriod, String> {
        let (target_amount, target_date) = self
            .lines
            .iter()
            .find_map(|(_, line)| match line {
                Line::SetTarget {
                    amount,
                    target_date,
                } => Some((amount, target_date)),
                _ => None,
            })
            .ok_or("No target for bucket".to_string())?;
        let target_amount = ex.new_amount_from_raw_amount(&target_amount)?;

        let deposited =
            self.lines
                .iter()
                .try_fold(
                    RawAmount::zero(&"JPY".to_string()),
                    |acc, (line_date, line)| {
                        if line_date <= date {
                            match line {
                                Line::Deposit(amount) => acc.add(amount),
                                Line::DepositCancellation(amount) => {
                                    acc
                                        .minus(amount)
                                        .and_then(|acc| {
                                            if acc.figure < dec!(0) {
                                                Err("attempt to withdraw more money than the Bucket contains".to_string())
                                            } else {
                                                Ok(acc)
                                            }
                                        })
                                },
                                _ => Ok(acc),
                            }
                        } else {
                            Ok(acc)
                        }
                    },
                )?;
        let deposited = ex.new_amount_from_raw_amount(&deposited)?;

        let current_period = period_config.period_for_date(date)?;

        let deposited_until_period_start = self.lines.iter().try_fold(
            RawAmount::zero(&"JPY".to_string()),
            |acc, (line_date, line)| {
                if line_date < &current_period.start_date {
                    match line {
                        Line::Deposit(amount) => acc.add(amount),
                        Line::DepositCancellation(amount) => acc.minus(amount),
                        _ => Ok(acc),
                    }
                } else {
                    Ok(acc)
                }
            },
        )?;
        let deposited_until_period_start =
            ex.new_amount_from_raw_amount(&deposited_until_period_start)?;

        let deposited_this_period = self
            .lines
            .iter()
            .try_fold(None, |acc, (line_date, line)| {
                if line_date >= &current_period.start_date && line_date <= date {
                    match line {
                        Line::Deposit(amount) => {
                            let acc = acc.unwrap_or(RawAmount::zero(&"JPY".to_string()));
                            acc.add(amount).map(Some)
                        },
                        Line::DepositCancellation(amount) => {
                            let acc = acc.unwrap_or(RawAmount::zero(&"JPY".to_string()));
                            acc.minus(amount).map(Some)
                        }
                        _ => Ok(acc),
                    }
                } else {
                    Ok(acc)
                }
            })?
            .map(|raw_amount: RawAmount| ex.new_amount_from_raw_amount(&raw_amount))
            .transpose()?;

        let number_of_periods = match period_config.periods_between(date, target_date) {
            Ok(nb) => nb,
            Err(ErrorPeriodsBetween::EndBeforeStart) => 1,
            any => any?,
        };

        let zero = ex.new_amount_from_raw_amount(&RawAmount::zero(&"JPY".to_string()))?;
        let recommended_deposit_figure = Amount::maximum(
            &target_amount.minus(&Amount::maximum(&deposited_until_period_start, &zero)),
            &zero)
            .div_decimal(&Decimal::from(number_of_periods));

        Ok(BucketThisPeriod {
            recommended_or_actual_change: deposited_this_period
                .clone()
                .unwrap_or(recommended_deposit_figure.clone()),
            current_recommended_deposit: recommended_deposit_figure,
            current_actual_deposit: deposited_this_period,
            total_deposit: deposited,
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
        todo!()
    }
}

#[cfg(test)]
mod test {
    /*
    What do I want to see on the remaining table
    - name
    - amount (different from deposit - there may be withdrawals)
    - Current recommended deposit (does not change if it has been committed or not)
    - Current actual deposit
    - Current withdrawal
    - Total deposit
    - Total withdrawal
    - Total
    - Target sum
    - Target date

    Test list:
    - two deposits the same day (no)

    - test__for_period__yen__one_deposit_last_period__two_deposit_cencellation_this_period
    - test__for_period__yen__one_deposit_last_period__one_deposit_cencellation_last_period
    - test__for_period__yen__one_deposit_last_period__two_deposit_cencellation_last_period
    - test__for_period__yen__one_deposits_this_last_period__one_deposit_cencellation_this_last_period
    - test__for_period__yen__two_deposits_next_period__one_deposit_cencellation_next_period
    - test__for_period__yen__one_deposits_last_period__deposit_cencellation_all_periods

    - test__for_period__yen__one_withdrawal_this_period
    - test__for_period__yen__one_withdrawal_last_period
    - test__for_period__yen__one_withdrawal_next_period
    - test__for_period__yen__one_withdrawal_this_last_next_period
    - test__for_period__yen__two_withdrawal_this_last_next_period

    - (test target set separately?)

    - test__for_period__yen__one_of_each_this_period
    - test__for_period__yen__one_of_each_this_last_period
    - test__for_period__yen__one_of_each_this_last_next_period

    - test__for_period__yen__two_of_each_this_last_next_period

    - (test to see if formatting the buckets in json is easy)

    - (test if any lines is set to a currency that isn't JPY -> fail)

    FUTURE TESTS

    - (forbid any negative amounts - deposits, deposit cancellations, targets, .....)

    - test__for_period__yen_euro__one_deposit_this_period
    - test__for_period__yen_euro__one_withdrawal_this_period

    - test__for_period__yen_euro__one_deposit_this_period
    - test__for_period__yen_euro__one_withdrawal_this_period

    ...


    Test table fields:
    - yen: no deposit this period / one deposit this period / two deposits this period
    - yen: no deposit last period / one deposit last period / two deposits last period
    - yen: no deposit next period / one deposit next period / two deposits next period

    - euro: no deposit this period / one deposit this period / two deposits this period
    - euro: no deposit last period / one deposit last period / two deposits last period
    - euro: no deposit next period / one deposit next period / two deposits next period

    - yen: no deposit cancellation this period / one deposit cancellation this period / two deposit cancellations this period
    - yen: no deposit cancellation last period / one deposit cancellation last period / two deposit cancellations last period
    - yen: no deposit cancellation next period / one deposit cancellation next period / two deposit cancellations next period

    - euro: no deposit cancellation this period / one deposit cancellation this period / two deposit cancellations this period
    - euro: no deposit cancellation last period / one deposit cancellation last period / two deposit cancellations last period
    - euro: no deposit cancellation next period / one deposit cancellation next period / two deposit cancellations next period

    - yen: no withdrawal this period / one withdrawal this period / two withdrawals this period
    - yen: no withdrawal last period / one withdrawal last period / two withdrawals last period
    - yen: no withdrawal next period / one withdrawal next period / two withdrawals next period

    - euro: no withdrawal this period / one withdrawal this period / two withdrawals this period
    - euro: no withdrawal last period / one withdrawal last period / two withdrawals last period
    - euro: no withdrawal next period / one withdrawal next period / two withdrawals next period

    - currency conversion: no this period / one this period / two this period
    - currency conversion: no last period / one last period / two last period
    - currency conversion: no next period / one next period / two next period

    - target: no target this period / some target this period / two target this period
    - current target: last period / this period / next period / period after next
     */
    use super::*;
    use crate::period::{CalendarMonthPeriodConfiguration, PeriodsConfiguration};
    use pretty_assertions::assert_eq;

    fn mkdate(month: u32, date: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2025, month, date).expect("Can create date")
    }

    type TestResult = Result<BucketThisPeriod, String>;
    type ExpectedFn = Box<dyn Fn(&ExchangeRates) -> TestResult>;

    struct Test {
        executed: bool,
        lines: Vec<(NaiveDate, Line)>,
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
        pub fn add_line(mut self, date: NaiveDate, line: Line) -> Self {
            self.lines.push((date, line));
            self
        }

        pub fn target_set_in_current_period_one_hundred_thousand_in_four_months(mut self) -> Self {
            self.add_line(
                mkdate(9, 1),
                Line::SetTarget {
                    amount: RawAmount::yen("100000"),
                    target_date: mkdate(12, 30),
                },
            )
        }

        pub fn target_set_last_period_one_hundred_thousand_in_five_months(mut self) -> Self {
            self.add_line(
                mkdate(8, 1),
                Line::SetTarget {
                    amount: RawAmount::yen("100000"),
                    target_date: mkdate(12, 30),
                },
            )
        }

        pub fn target_set_many_periods_ago_twelve_hundred_in_twelve_months(mut self) -> Self {
            self.add_line(
                mkdate(1, 1),
                Line::SetTarget {
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
            bucket_builder: impl Fn(&ExchangeRates) -> BucketThisPeriod + 'static,
        ) -> Self {
            self.expected = Box::new(move |ex| Ok(bucket_builder(ex)));
            self
        }

        pub fn expect_bucket_no_commits_one_hundred_thousand_in_four_months(mut self) -> Self {
            self.expect_bucket(
                |ex| BucketThisPeriod {
                    recommended_or_actual_change: ex.yen("25000"),
                    current_recommended_deposit: ex.yen("25000"),
                    current_actual_deposit: None,
                    total_deposit: ex.yen("0"),
                }
            )
        }
    }

    impl Test {
        fn execute(&mut self) -> () {
            self.executed = true;
            let ex = ExchangeRates::for_tests();
            let period_configuration =
                PeriodConfigurationVaultValue::CalendarMonth(CalendarMonthPeriodConfiguration {});
            let today = mkdate(9, 15);

            let bucket = Bucket { lines: self.lines.clone() };

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
                .add_line(mkdate(9, 15), Line::Deposit(RawAmount::yen("10000")))
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
                    Line::SetTarget {
                        amount: RawAmount::yen("100000"),
                        target_date: mkdate(8, 31),
                    },
                )
                .expect_bucket(|ex| BucketThisPeriod {
                    recommended_or_actual_change: ex.yen("100000"),
                    current_recommended_deposit: ex.yen("100000"),
                    current_actual_deposit: None,
                    total_deposit: ex.yen("0"),
                })
                .execute()
        }

        #[test]
        fn this_period() {
            Test::default()
                .add_line(
                    mkdate(9, 15),
                    Line::SetTarget {
                        amount: RawAmount::yen("100000"),
                        target_date: mkdate(9, 30),
                    },
                )
                .expect_bucket(|ex| BucketThisPeriod {
                    recommended_or_actual_change: ex.yen("100000"),
                    current_recommended_deposit: ex.yen("100000"),
                    current_actual_deposit: None,
                    total_deposit: ex.yen("0"),
                })
                .execute()
        }

        #[test]
        fn next_period() {
            Test::default()
                .add_line(
                    mkdate(9, 15),
                    Line::SetTarget {
                        amount: RawAmount::yen("100000"),
                        target_date: mkdate(10, 31),
                    },
                )
                .expect_bucket(|ex| BucketThisPeriod {
                    recommended_or_actual_change: ex.yen("50000"),
                    current_recommended_deposit: ex.yen("50000"),
                    current_actual_deposit: None,
                    total_deposit: ex.yen("0"),
                })
                .execute()
        }

        #[test]
        fn next_next_period() {
            Test::default()
                .add_line(
                    mkdate(9, 15),
                    Line::SetTarget {
                        amount: RawAmount::yen("100000"),
                        target_date: mkdate(11, 30),
                    },
                )
                .expect_bucket(|ex| BucketThisPeriod {
                    recommended_or_actual_change: ex.yen("33333.33"),
                    current_recommended_deposit: ex.yen("33333.33"),
                    current_actual_deposit: None,
                    total_deposit: ex.yen("0"),
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
                    .add_line(mkdate(9, 15), Line::Deposit(RawAmount::yen("10000")))
                    .expect_bucket(|ex| BucketThisPeriod {
                        recommended_or_actual_change: ex.yen("10000"),
                        current_recommended_deposit: ex.yen("25000"),
                        current_actual_deposit: Some(ex.yen("10000")),
                        total_deposit: ex.yen("10000"),
                    })
                    .execute();
            }

            #[test]
            fn two_deposits__recommended() {
                Test::default()
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(9, 3), Line::Deposit(RawAmount::yen("10000")))
                    .add_line(mkdate(9, 5), Line::Deposit(RawAmount::yen("15000")))
                    .expect_bucket(|ex| BucketThisPeriod {
                        recommended_or_actual_change: ex.yen("25000"),
                        current_recommended_deposit: ex.yen("25000"),
                        current_actual_deposit: Some(ex.yen("25000")),
                        total_deposit: ex.yen("25000"),
                    })
                    .execute();
            }

            /*
            TODO before merging PR
                 uncomment before big refactor
            #[test]
            fn two_deposits_same_day() {
                Test::default()
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(9, 5), Line::Deposit(RawAmount::yen("10000")))
                    .add_line(mkdate(9, 5), Line::Deposit(RawAmount::yen("15000")))
                    .expect_error("two deposit operation on the same day")
                    .execute();
            }

             */

            #[test]
            fn one_deposit_period_start__partial() {
                Test::default()
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(9, 1), Line::Deposit(RawAmount::yen("10000")))
                    .expect_bucket(|ex| BucketThisPeriod {
                        recommended_or_actual_change: ex.yen("10000"),
                        current_recommended_deposit: ex.yen("25000"),
                        current_actual_deposit: Some(ex.yen("10000")),
                        total_deposit: ex.yen("10000"),
                    })
                    .execute();
            }

            #[test]
            fn one_deposit_before_today__partial() {
                Test::default()
                    .target_set_last_period_one_hundred_thousand_in_five_months()
                    .add_line(mkdate(9, 3), Line::Deposit(RawAmount::yen("10000")))
                    .expect_bucket(|ex| BucketThisPeriod {
                        recommended_or_actual_change: ex.yen("10000"),
                        current_recommended_deposit: ex.yen("25000"), // This is correct. Even if the target was set for five months, there was no deposit last month
                        current_actual_deposit: Some(ex.yen("10000")),
                        total_deposit: ex.yen("10000"),
                    })
                    .execute();
            }

            #[test]
            fn one_deposit_before_today__over() {
                Test::default()
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(9, 3), Line::Deposit(RawAmount::yen("30000")))
                    .expect_bucket(|ex| BucketThisPeriod {
                        recommended_or_actual_change: ex.yen("30000"),
                        current_recommended_deposit: ex.yen("25000"),
                        current_actual_deposit: Some(ex.yen("30000")),
                        total_deposit: ex.yen("30000"),
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
                    .add_line(mkdate(8, 31), Line::Deposit(RawAmount::yen("20000")))
                    .expect_bucket(|ex| BucketThisPeriod {
                        recommended_or_actual_change: ex.yen("20000"),
                        current_recommended_deposit: ex.yen("20000"),
                        current_actual_deposit: None,
                        total_deposit: ex.yen("20000"),
                    })
                    .execute();
            }

            #[test]
            fn two_deposits__partial() {
                Test::default()
                    .target_set_last_period_one_hundred_thousand_in_five_months()
                    .add_line(mkdate(8, 15), Line::Deposit(RawAmount::yen("5000")))
                    .add_line(mkdate(8, 31), Line::Deposit(RawAmount::yen("5000")))
                    .expect_bucket(|ex| BucketThisPeriod {
                        recommended_or_actual_change: ex.yen("22500"),
                        current_recommended_deposit: ex.yen("22500"),
                        current_actual_deposit: None,
                        total_deposit: ex.yen("10000"),
                    })
                    .execute();
            }

            /*
            TODO (before merging PR)
                 This is too hard to implement before we proceed to a big refactor. Implement then

            #[test]
            fn two_deposits_same_day() {
                Test::default()
                    .target_set_last_period_one_hundred_thousand_in_five_months()
                    .add_line(mkdate(8, 31), Line::Deposit(RawAmount::yen("5000")))
                    .add_line(mkdate(8, 31), Line::Deposit(RawAmount::yen("5000")))
                    .expect_error("two deposit operation on the same day")
                    .execute();
            }
            */

            #[test]
            fn many_periods_ago() {
                Test::default()
                    .target_set_many_periods_ago_twelve_hundred_in_twelve_months()
                    .add_line(mkdate(1, 15), Line::Deposit(RawAmount::yen("100")))
                    .add_line(mkdate(2, 28), Line::Deposit(RawAmount::yen("100")))
                    .expect_bucket(|ex| BucketThisPeriod {
                        recommended_or_actual_change: ex.yen("250"),
                        current_recommended_deposit: ex.yen("250"),
                        current_actual_deposit: None,
                        total_deposit: ex.yen("200"),
                    })
                    .execute();
            }

            #[test]
            fn one_deposit__over_recommendation() {
                Test::default()
                    .target_set_last_period_one_hundred_thousand_in_five_months()
                    .add_line(mkdate(8, 31), Line::Deposit(RawAmount::yen("60000")))
                    .expect_bucket(|ex| BucketThisPeriod {
                        recommended_or_actual_change: ex.yen("10000"),
                        current_recommended_deposit: ex.yen("10000"),
                        current_actual_deposit: None,
                        total_deposit: ex.yen("60000"),
                    })
                    .execute();
            }

            #[test]
            fn one_deposit__over_target() {
                Test::default()
                    .target_set_last_period_one_hundred_thousand_in_five_months()
                    .add_line(mkdate(8, 31), Line::Deposit(RawAmount::yen("200000")))
                    .expect_bucket(|ex| BucketThisPeriod {
                        recommended_or_actual_change: ex.yen("0"),
                        current_recommended_deposit: ex.yen("0"),
                        current_actual_deposit: None,
                        total_deposit: ex.yen("200000"),
                    })
                    .execute();
            }
        }
        mod after_current_period {
            use crate::amounts::RawAmount;
            use crate::buckets::{BucketThisPeriod, Line};
            use crate::buckets::test::{mkdate, Test};

            #[test]
            fn one_deposit_tomorrow() {
                Test::default()
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(9, 16), Line::Deposit(RawAmount::yen("25000")))
                    .expect_bucket_no_commits_one_hundred_thousand_in_four_months()
                    .execute();
            }

            #[test]
            fn one_deposit_this_period_after_tomorrow() {
                Test::default()
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(9, 17), Line::Deposit(RawAmount::yen("25000")))
                    .expect_bucket_no_commits_one_hundred_thousand_in_four_months()
                    .execute();
            }

            #[test]
            fn one_deposit_next_period() {
                Test::default()
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(10, 18), Line::Deposit(RawAmount::yen("25000")))
                    .expect_bucket_no_commits_one_hundred_thousand_in_four_months()
                    .execute();
            }

            #[test]
            fn one_deposit_many_period_after() {
                Test::default()
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(12, 18), Line::Deposit(RawAmount::yen("25000")))
                    .expect_bucket_no_commits_one_hundred_thousand_in_four_months()
                    .execute();
            }

            #[test]
            fn many_deposits() {
                Test::default()
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(9, 16), Line::Deposit(RawAmount::yen("25000")))
                    .add_line(mkdate(9, 17), Line::Deposit(RawAmount::yen("25000")))
                    .add_line(mkdate(10, 18), Line::Deposit(RawAmount::yen("25000")))
                    .add_line(mkdate(12, 18), Line::Deposit(RawAmount::yen("25000")))
                    .expect_bucket_no_commits_one_hundred_thousand_in_four_months()
                    .execute();
            }

            #[test]
            fn one_deposit_this_period_next_period() {
                Test::default()
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(9, 10), Line::Deposit(RawAmount::yen("25000")))
                    .add_line(mkdate(10, 18), Line::Deposit(RawAmount::yen("25000")))
                    .expect_bucket((|ex| BucketThisPeriod {
                        recommended_or_actual_change: ex.yen("25000"),
                        current_recommended_deposit: ex.yen("25000"),
                        current_actual_deposit: Some(ex.yen("25000")),
                        total_deposit: ex.yen("25000"),
                    }))
                    .execute();
            }
        }
        mod all_periods_mixed {
            use super::*;

            #[test]
            fn all_periods () {
                Test::default()
                    .target_set_many_periods_ago_twelve_hundred_in_twelve_months()
                    .add_line(mkdate(1, 15), Line::Deposit(RawAmount::yen("50")))
                    .add_line(mkdate(2, 1), Line::Deposit(RawAmount::yen("55")))
                    .add_line(mkdate(8, 31), Line::Deposit(RawAmount::yen("55")))
                    .add_line(mkdate(9, 1), Line::Deposit(RawAmount::yen("200")))
                    .add_line(mkdate(9, 15), Line::Deposit(RawAmount::yen("50")))
                    .add_line(mkdate(9, 20), Line::Deposit(RawAmount::yen("10")))
                    .add_line(mkdate(10, 25), Line::Deposit(RawAmount::yen("260")))
                    .add_line(mkdate(12, 31), Line::Deposit(RawAmount::yen("260")))
                    .expect_bucket(|ex| BucketThisPeriod {
                        recommended_or_actual_change: ex.yen("250"),
                        current_recommended_deposit: ex.yen("260"),
                        current_actual_deposit: Some(ex.yen("250")),
                        total_deposit: ex.yen("410"),
                    })
                    .execute();
            }

            #[test]
            fn all_periods_multiple_deposits () {
                Test::default()
                    .target_set_many_periods_ago_twelve_hundred_in_twelve_months()
                    .add_line(mkdate(1, 15), Line::Deposit(RawAmount::yen("25")))
                    .add_line(mkdate(1, 16), Line::Deposit(RawAmount::yen("25")))
                    .add_line(mkdate(2, 1), Line::Deposit(RawAmount::yen("50")))
                    .add_line(mkdate(2, 2), Line::Deposit(RawAmount::yen("5")))
                    .add_line(mkdate(8, 30), Line::Deposit(RawAmount::yen("25")))
                    .add_line(mkdate(8, 31), Line::Deposit(RawAmount::yen("30")))
                    .add_line(mkdate(9, 1), Line::Deposit(RawAmount::yen("200")))
                    .add_line(mkdate(9, 15), Line::Deposit(RawAmount::yen("50")))
                    .add_line(mkdate(9, 20), Line::Deposit(RawAmount::yen("10")))
                    .add_line(mkdate(10, 25), Line::Deposit(RawAmount::yen("200")))
                    .add_line(mkdate(10, 26), Line::Deposit(RawAmount::yen("60")))
                    .add_line(mkdate(12, 30), Line::Deposit(RawAmount::yen("200")))
                    .add_line(mkdate(12, 31), Line::Deposit(RawAmount::yen("60")))
                    .expect_bucket(|ex| BucketThisPeriod {
                        recommended_or_actual_change: ex.yen("250"),
                        current_recommended_deposit: ex.yen("260"),
                        current_actual_deposit: Some(ex.yen("250")),
                        total_deposit: ex.yen("410"),
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
                    .add_line(mkdate(9, 8), Line::Deposit(RawAmount::yen("25000")))
                    .add_line(mkdate(9, 15), Line::DepositCancellation(RawAmount::yen("10000")))
                    .expect_bucket(|ex| BucketThisPeriod {
                        recommended_or_actual_change: ex.yen("15000"),
                        current_recommended_deposit: ex.yen("25000"),
                        current_actual_deposit: Some(ex.yen("15000")),
                        total_deposit: ex.yen("15000"),
                    })
                    .execute();
            }

            #[test]
            fn one_today_deposit_last_period() {
                Test::default()
                    .target_set_last_period_one_hundred_thousand_in_five_months()
                    .add_line(mkdate(8, 8), Line::Deposit(RawAmount::yen("20000")))
                    .add_line(mkdate(9, 15), Line::DepositCancellation(RawAmount::yen("10000")))
                    .expect_bucket(|ex| BucketThisPeriod {
                        recommended_or_actual_change: ex.yen("-10000"),
                        current_recommended_deposit: ex.yen("20000"),
                        current_actual_deposit: Some(ex.yen("-10000")),
                        total_deposit: ex.yen("10000"),
                    })
                    .execute();
            }

            #[test]
            fn two_this_period_deposit_last_period() {
                Test::default()
                    .target_set_last_period_one_hundred_thousand_in_five_months()
                    .add_line(mkdate(8, 8), Line::Deposit(RawAmount::yen("20000")))
                    .add_line(mkdate(9, 14), Line::DepositCancellation(RawAmount::yen("5000")))
                    .add_line(mkdate(9, 15), Line::DepositCancellation(RawAmount::yen("5000")))
                    .expect_bucket(|ex| BucketThisPeriod {
                        recommended_or_actual_change: ex.yen("-10000"),
                        current_recommended_deposit: ex.yen("20000"),
                        current_actual_deposit: Some(ex.yen("-10000")),
                        total_deposit: ex.yen("10000"),
                    })
                    .execute();
            }

            #[test]
            fn two_this_period() {
                Test::default()
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(9, 8), Line::Deposit(RawAmount::yen("25000")))
                    .add_line(mkdate(9, 14), Line::DepositCancellation(RawAmount::yen("5000")))
                    .add_line(mkdate(9, 15), Line::DepositCancellation(RawAmount::yen("5000")))
                    .expect_bucket(|ex| BucketThisPeriod {
                        recommended_or_actual_change: ex.yen("15000"),
                        current_recommended_deposit: ex.yen("25000"),
                        current_actual_deposit: Some(ex.yen("15000")),
                        total_deposit: ex.yen("15000"),
                    })
                    .execute();
            }

            /* TODO before merging PR
                    uncomment before big refactor
            #[test]
            fn one_today_deposit_the_same_day() {
                Test::default()
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(9, 15), Line::Deposit(RawAmount::yen("25000")))
                    .add_line(mkdate(9, 15), Line::DepositCancellation(RawAmount::yen("10000")))
                    .expect_error("two deposit operation on the same day")
                    .execute();
            }
             */

            #[test]
            fn one_today__too_big() {
                Test::default()
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(9, 8), Line::Deposit(RawAmount::yen("25000")))
                    .add_line(mkdate(9, 15), Line::DepositCancellation(RawAmount::yen("30000")))
                    .expect_error("attempt to withdraw more money than the Bucket contains")
                    .execute();
            }

            #[test]
            fn one_cancellation_too_big_followed_by_one_deposit_that_brings_back_the_bucket_to_positive() {
                Test::default()
                    .target_set_in_current_period_one_hundred_thousand_in_four_months()
                    .add_line(mkdate(9, 8), Line::Deposit(RawAmount::yen("25000")))
                    .add_line(mkdate(9, 13), Line::DepositCancellation(RawAmount::yen("30000")))
                    .add_line(mkdate(9, 15), Line::Deposit(RawAmount::yen("30000")))
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
                    .add_line(mkdate(8, 1), Line::Deposit(RawAmount::yen("20000")))
                    .add_line(mkdate(8, 31), Line::DepositCancellation(RawAmount::yen("10000")))
                    .expect_bucket(|ex| BucketThisPeriod{
                        recommended_or_actual_change: ex.yen("22500"),
                        current_recommended_deposit: ex.yen("22500"),
                        current_actual_deposit: None,
                        total_deposit: ex.yen("10000"),
                    })
                    .execute();
            }
        }
    }
}
