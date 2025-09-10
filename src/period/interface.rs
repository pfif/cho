use crate::period::calendar_month_period::CalendarMonthPeriodConfiguration;
use crate::period::fixed_length_period::FixedLengthPeriodConfiguration;
use crate::vault::VaultReadable;
use chrono::NaiveDate;
#[cfg(test)]
use mockall::automock;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum PeriodConfigurationVaultValue {
    #[serde(rename = "fixed_length")]
    FixedLength(FixedLengthPeriodConfiguration),
    #[serde(rename = "monthly")]
    CalendarMonth(CalendarMonthPeriodConfiguration),
}

impl VaultReadable for PeriodConfigurationVaultValue {
    const KEY: &'static str = "periods_configuration";
}

impl PeriodConfigurationVaultValue {
    fn unpack(&self) -> &dyn PeriodsConfiguration {
        match self {
            PeriodConfigurationVaultValue::FixedLength(p) => p,
            PeriodConfigurationVaultValue::CalendarMonth(p) => p,
        }
    }
}

impl PeriodsConfiguration for PeriodConfigurationVaultValue {
    fn period_for_date(&self, date: &NaiveDate) -> Result<Period, String> {
        self.unpack().period_for_date(date)
    }

    fn periods_between(&self, start: &NaiveDate, end: &NaiveDate) -> Result<u16, String> {
        self.unpack().periods_between(start, end)
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

impl Period {
    pub fn contains(&self, date: &NaiveDate) -> bool {
        self.start_date <= *date && *date <= self.end_date
    }
}
