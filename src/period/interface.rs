use chrono::NaiveDate;
#[cfg(test)]
use mockall::automock;
use serde::Deserialize;
use crate::period::fixed_length_period::FixedLengthPeriodConfiguration;
use crate::period::calendar_month_period::CalendarMonthPeriodConfiguration;


#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum AnyPeriodsConfiguration {
    #[serde(rename = "fixed_length")]
    FixedLength(FixedLengthPeriodConfiguration),
    #[serde(rename = "monthly")]
    CalendarMonth(CalendarMonthPeriodConfiguration)
}

impl AnyPeriodsConfiguration {
    fn unpack(&self) -> &dyn PeriodsConfiguration{
        match self {
           AnyPeriodsConfiguration::FixedLength(p) => p,
           AnyPeriodsConfiguration::CalendarMonth(p) => p
        }
    }
}

impl PeriodsConfiguration for AnyPeriodsConfiguration {
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

