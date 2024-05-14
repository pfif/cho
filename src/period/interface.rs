use chrono::NaiveDate;
#[cfg(test)]
use mockall::automock;
use crate::period::fixed_length_period::{ErrorStartBeforePeriodConfiguration, PeriodVaultValues};
use crate::vault::VaultReadable;

pub type PeriodNumber = u16;

impl VaultReadable for PeriodVaultValues {
    const KEY: &'static str = "periods_configuration";
}

#[cfg_attr(test, automock)]
pub trait PeriodsConfiguration {
    fn period_number_for_date(
        &self,
        date: &NaiveDate,
    ) -> Result<PeriodNumber, ErrorStartBeforePeriodConfiguration>;
    fn period_for_date(&self, date: &NaiveDate) -> Result<Period, String>;
    fn periods_between(&self, start: &NaiveDate, end: &NaiveDate) -> Result<PeriodNumber, String>;
}
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Period {
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

