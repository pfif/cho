use chrono::NaiveDate;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::Deserialize;
use crate::amounts::{Amount, RawAmount};
use crate::amounts::exchange_rates::ExchangeRates;
use crate::period::{ErrorPeriodsBetween, Period, PeriodConfigurationVaultValue, PeriodsConfiguration};
use crate::remaining_operation::core_types::{Operand, OperandBuilder};
use crate::vault::VaultReadable;

pub type BucketsVaultValue = Vec<Bucket>;
impl VaultReadable for BucketsVaultValue {
    const KEY: &'static str = "buckets";
}

#[derive(Deserialize)]
pub struct Bucket {
    lines: Vec<(NaiveDate, Line)>
}

#[derive(Deserialize)]
#[serde(tag = "action")]
enum Line{
    Deposit (RawAmount),
    SetTarget {
        amount: RawAmount,
        target_date: NaiveDate
    }
}

/// Amounts to deposit
#[derive(Debug, Eq, PartialEq)]
pub struct BucketThisPeriod {
    // TODO this name is bad
    changed: Amount,
    current_recommended_deposit: Amount,
}

impl Bucket {
   fn for_period(&self, period_config: &PeriodConfigurationVaultValue, date: &NaiveDate, ex: &ExchangeRates) -> Result<BucketThisPeriod, String> {
       let (target_amount, target_date) = self.lines
           .iter()
           .find_map(|(_, line)| match line {
               Line::SetTarget{amount, target_date} => Some((amount, target_date)),
               _ => None
           })
           .ok_or("No target for bucket".to_string())?;

       let number_of_periods = match period_config.periods_between(date, target_date) {
           Ok(nb) => nb,
           Err(ErrorPeriodsBetween::EndBeforeStart) => 1,
           any => any?
       };

       let recommended_deposit_figure = ex.new_amount(
           &target_amount.currency,
           target_amount.figure / &Decimal::from(number_of_periods)
       )?;
   
        Ok(BucketThisPeriod{
            changed: recommended_deposit_figure.clone(),
            current_recommended_deposit: recommended_deposit_figure.clone(),
        }) 
   }
}

impl OperandBuilder for Bucket {
    fn build(self, period_configuration: &PeriodConfigurationVaultValue, today: &NaiveDate, exchange_rates: &ExchangeRates) -> Result<Option<Operand>, String> {
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
    - Target sum
    - Target date

    Test list:
    - test__for_period__yen__one_deposit_this_period
    - test__for_period__yen__one_deposit_last_period
    - test__for_period__yen__one_deposit_next_period
    - test__for_period__yen__one_deposit_this_last_next_period
    - test__for_period__yen__two_deposit_this_last_next_period


    - test__for_period__yen__one_withdrawal_this_period
    - test__for_period__yen__one_withdrawal_last_period
    - test__for_period__yen__one_withdrawal_next_period
    - test__for_period__yen__one_withdrawal_this_last_next_period
    - test__for_period__yen__two_withdrawal_this_last_next_period

    (test target set separately?)

    - test__for_period__yen__one_of_each_this_period
    - test__for_period__yen__one_of_each_this_last_period
    - test__for_period__yen__one_of_each_this_last_next_period

    - test__for_period__yen__two_of_each_this_last_next_period

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
    use pretty_assertions::assert_eq;
    use crate::period::{CalendarMonthPeriodConfiguration, PeriodsConfiguration};
    use super::*;

    fn mkdate(month: u32, date: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2025, month, date).expect("Can create date")
    }
    
    type TestResult = Result<BucketThisPeriod, String>;
    type ExpectedFn = Box<dyn Fn(&ExchangeRates) -> TestResult>;
    
    struct Test {
        lines: Vec<(NaiveDate, Line)>,
        expected: ExpectedFn
    }

    impl Default for Test {
        fn default() -> Self {
            Test{
                lines: Vec::new(),
                expected: Box::new(|_| Err("Please setup the test".to_string()))
            }
        }
    }

    impl Test {
        pub fn add_line(mut self, line: (NaiveDate, Line)) -> Self {
            self.lines.push(line);
            self
        }

        pub fn expect_error(mut self, error: &str) -> Self {
            let error_string = error.to_string();
            self.expected = Box::new(move |_| Err(error_string.clone()));
            self
        }
        
        pub fn expect_bucket(mut self, bucket_builder: impl Fn(&ExchangeRates) -> BucketThisPeriod + 'static) -> Self {
            self.expected = Box::new(move |ex| Ok(bucket_builder(ex)));
            self
        }
    }

    impl Test {
        fn execute(self) -> () {
            let ex = ExchangeRates::for_tests();
            let period_configuration = PeriodConfigurationVaultValue::CalendarMonth(CalendarMonthPeriodConfiguration {});
            let today = mkdate(9, 15);

            let bucket = Bucket { lines: self.lines };
            
            assert_eq!(
                bucket.for_period(&period_configuration, &today, &ex),
                (self.expected)(&ex)
            );
        }
    }

    #[test]
    fn test__for_period__period_change__yen__one_deposit_this_period__no_target() {
        Test::default()
            .add_line((mkdate(9, 15), Line::Deposit(RawAmount::yen("10000"))))
            .expect_error("No target for bucket")
            .execute()
    }

    fn test__for_period__period_change__yen__one_deposit_this_period__no_lines() {
        Test::default()
            .expect_error("No target for bucket")
            .execute()
    }

    #[test]
    fn test__for_period__current_target_last_period() {
        Test::default()
            .add_line((mkdate(9, 15), Line::SetTarget{amount: RawAmount::yen("100000"), target_date: mkdate(8, 31)}))
            .expect_bucket(|ex| BucketThisPeriod {
                changed: ex.yen("100000"),
                current_recommended_deposit: ex.yen("100000")
            })
            .execute()
    }

    #[test]
    fn test__for_period__current_target_this_period() {
        Test::default()
            .add_line((mkdate(9, 15), Line::SetTarget{amount: RawAmount::yen("100000"), target_date: mkdate(9, 30)}))
            .expect_bucket(|ex| BucketThisPeriod {
                changed: ex.yen("100000"),
                current_recommended_deposit: ex.yen("100000")
            })
            .execute()
    }
    
    #[test]
    fn test__for_period__current_target_next_period() {
        Test::default()
            .add_line((mkdate(9, 15), Line::SetTarget{amount: RawAmount::yen("100000"), target_date: mkdate(10, 31)}))
            .expect_bucket(|ex| BucketThisPeriod {
                changed: ex.yen("50000"),
                current_recommended_deposit: ex.yen("50000")
            })
            .execute()
    }
    
    #[test]
    fn test__for_period__current_target_next_next_period() {
        Test::default()
            .add_line((mkdate(9, 15), Line::SetTarget{amount: RawAmount::yen("100000"), target_date: mkdate(11, 30)}))
            .expect_bucket(|ex| BucketThisPeriod {
                changed: ex.yen("33333.33"),
                current_recommended_deposit: ex.yen("33333.33")
            })
            .execute()
    }
}