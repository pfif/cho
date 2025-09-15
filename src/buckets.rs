use chrono::NaiveDate;
use rust_decimal_macros::dec;
use serde::Deserialize;
use crate::amounts::{Amount, RawAmount};
use crate::amounts::exchange_rates::ExchangeRates;
use crate::period::{Period, PeriodConfigurationVaultValue};
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
   fn for_period(&self, period: &Period, ex: &ExchangeRates) -> Result<BucketThisPeriod, String> {
       let (target_amount, _) = self.lines
           .iter()
           .find_map(|(_, line)| match line {
               Line::SetTarget{amount, target_date} => Some((amount, target_date)),
               _ => None
           })
           .ok_or("No target for bucket".to_string())?;
   
        Ok(BucketThisPeriod{
            changed: target_amount.to_amount(ex)?,
            current_recommended_deposit: target_amount.to_amount(ex)?,
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
    - test__for_period__no_lines

    - test__for_period__current_target_last_period
    - test__for_period__current_target_next_period
    - test__for_period__current_target_next_next_period

    - test__for_period__yen__one_deposit_this_period
    - test__for_period__yen__one_deposit_last_period
    - test__for_period__yen__one_deposit_next_period
    - test__for_period__yen__one_deposit_this_last_next_period


    - test__for_period__yen__one_withdrawal_this_period
    - test__for_period__yen__one_withdrawal_last_period
    - test__for_period__yen__one_withdrawal_next_period
    - test__for_period__yen__one_withdrawal_this_last_next_period

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

    #[test]
    fn test__for_period__period_change__yen__one_deposit_this_period__no_target() {
        let ex = ExchangeRates::for_tests();
        let period_configuration = PeriodConfigurationVaultValue::CalendarMonth(CalendarMonthPeriodConfiguration {});
        let today = mkdate(9, 15);
        let period = period_configuration.period_for_date(&today).expect("can create period");

        let bucket = Bucket { lines:
        vec![
            (today.clone(), Line::Deposit(RawAmount::yen("10000")))
        ] };

        assert_eq!(
            bucket.for_period(&period, &ex),
            Err("No target for bucket".into())
        )
    }

    #[test]
    fn test__for_period__current_target_this_period() {
        let ex = ExchangeRates::for_tests();
        let period_configuration = PeriodConfigurationVaultValue::CalendarMonth(CalendarMonthPeriodConfiguration {});
        let today = NaiveDate::from_ymd_opt(2025, 9, 15).expect("can create date");
        let period = period_configuration.period_for_date(&today).expect("can create period");

        let bucket = Bucket { lines: vec![
            (today.clone(), Line::SetTarget{amount: RawAmount::yen("100000"), target_date: mkdate(9, 30)})
        ] };

        assert_eq!(
            bucket.for_period(&period, &ex),
            Ok(
                BucketThisPeriod{
                    changed: ex.yen("100000"),
                    current_recommended_deposit: ex.yen("100000")
                }
            )
        )
    }
}