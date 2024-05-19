use chrono::NaiveDate;
#[cfg(test)]
use mockall::automock;
use serde::Deserialize;
use crate::period::fixed_length_period::{FixedLengthPeriodConfiguration};
use crate::period::calendar_month_period::{CalendarMonthPeriodConfiguration};
use crate::vault::VaultReadable;


#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum PeriodVaultValues {
    #[serde(rename = "fixed_length")]
    FixedLength(FixedLengthPeriodConfiguration),
    #[serde(rename = "monthly")]
    CalendarMonth(CalendarMonthPeriodConfiguration)
}

impl VaultReadable for PeriodVaultValues {
    const KEY: &'static str = "periods_configuration";
}

impl PeriodVaultValues{
    fn unpack(&self) -> &dyn PeriodsConfiguration{
        match self {
           PeriodVaultValues::FixedLength(p) => p,
           PeriodVaultValues::CalendarMonth(p) => p
        }
    }
}

impl PeriodsConfiguration for PeriodVaultValues{
    fn period_for_date(&self, date: &NaiveDate) -> Result<Period, String> {
        return self.unpack().period_for_date(date);
    }

    fn periods_between(&self, start: &NaiveDate, end: &NaiveDate) -> Result<u16, String> {
        return self.unpack().periods_between(start, end);
    }
}

#[cfg_attr(test, automock)]
pub trait PeriodsConfiguration {
    fn period_for_date(&self, date: &NaiveDate) -> Result<Period, String>;
    fn periods_between(&self, start: &NaiveDate, end: &NaiveDate) -> Result<u16, String>;
}
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Period {
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

