use chrono::{Days, NaiveDate};
#[cfg(test)]
use mockall::automock;
use serde::Deserialize;

use crate::vault::VaultReadable;

pub type PeriodNumber = u16;

#[derive(Deserialize)]
pub struct PeriodVaultValues {
    start_date: NaiveDate,
    period_in_days: u8,
}

// TODO Make a macro to generate this
impl VaultReadable for PeriodVaultValues {
    const KEY: &'static str = "periods_configuration";
}

pub struct ErrorStartBeforePeriodConfiguration;

#[cfg_attr(test, automock)]
pub trait PeriodsConfiguration {
    fn period_number_for_date(
        &self,
        date: &NaiveDate,
    ) -> Result<PeriodNumber, ErrorStartBeforePeriodConfiguration>;
    fn period_for_date(&self, date: &NaiveDate) -> Result<Period, String>;
    fn periods_between(&self, start: &NaiveDate, end: &NaiveDate) -> Result<PeriodNumber, String>;
}

impl PeriodsConfiguration for PeriodVaultValues {
    fn period_number_for_date(
        &self,
        date: &NaiveDate,
    ) -> Result<PeriodNumber, ErrorStartBeforePeriodConfiguration> {
        if date < &self.start_date {
            return Err(ErrorStartBeforePeriodConfiguration {});
        }
        let days_since_start = (*date - self.start_date).num_days() as u8;
        return Ok((days_since_start / self.period_in_days) as PeriodNumber);
    }
    fn period_for_date(&self, date: &NaiveDate) -> Result<Period, String> {
        let period_number_for_date = self.period_number_for_date(date).or_else(
            |_errortypecheck: ErrorStartBeforePeriodConfiguration| {
                return Err("Date is before PeriodsConfiguration's start".to_string());
            },
        )?;

        let start_from_config_start = period_number_for_date as u64 * self.period_in_days as u64;

        // Adding the period length to the period starts results in the next period start
        // The period end is the day before
        // Hence: we remove 1 to the period length
        let end_from_config_start = start_from_config_start + (self.period_in_days as u64 - 1);

        return Ok(Period {
            start_date: self.start_date + Days::new(start_from_config_start),
            end_date: self.start_date + Days::new(end_from_config_start),
        });
    }

    fn periods_between(&self, start: &NaiveDate, end: &NaiveDate) -> Result<PeriodNumber, String> {
        if start > end {
            return Err("Start date is after end date".to_string());
        }

        let (start_period_number, end_period_number) = match (
            self.period_number_for_date(start),
            self.period_number_for_date(end),
        ) {
            (Ok(start_period_number), Ok(end_period_number)) => {
                (start_period_number, end_period_number)
            }
            (
                Err(ErrorStartBeforePeriodConfiguration),
                Err(ErrorStartBeforePeriodConfiguration),
            ) => {
                return Err("Dates before PeriodsConfiguration's start".to_string());
            }
            (Err(ErrorStartBeforePeriodConfiguration), _) => {
                return Err("Start date is before PeriodsConfiguration's start".to_string());
            }
            (_, Err(ErrorStartBeforePeriodConfiguration)) => {
                return Err("End date is before PeriodsConfiguration's start".to_string());
            }
        };

        return Ok((end_period_number + 1) - start_period_number);
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Period {
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

#[allow(non_snake_case)]
#[cfg(test)]
mod tests {
    use super::{Period, PeriodVaultValues, PeriodsConfiguration};
    use chrono::NaiveDate;

    fn date(day_of_month: u32) -> NaiveDate {
        return NaiveDate::from_ymd_opt(2023, 04, day_of_month).unwrap();
    }

    fn config() -> PeriodVaultValues {
        return PeriodVaultValues {
            start_date: date(11),
            period_in_days: 4,
        };
    }

    #[test]
    fn period_between__length_of_period__first_period() {
        assert_eq!(config().periods_between(&date(11), &date(14)).unwrap(), 1)
    }

    #[test]
    fn period_between__length_of_period__not_first_period() {
        assert_eq!(config().periods_between(&date(15), &date(18)).unwrap(), 1)
    }

    #[test]
    fn period_between__several_periods__start_finish__first_period() {
        assert_eq!(config().periods_between(&date(11), &date(22)).unwrap(), 3)
    }

    #[test]
    fn period_between__several_periods__start_finish__not_first_period() {
        assert_eq!(config().periods_between(&date(15), &date(22)).unwrap(), 2)
    }

    #[test]
    fn period_between__several_periods__start_start__first_period() {
        assert_eq!(config().periods_between(&date(11), &date(23)).unwrap(), 4)
    }

    #[test]
    fn period_between__several_periods__start_start__not_first_period() {
        assert_eq!(config().periods_between(&date(15), &date(23)).unwrap(), 3)
    }

    #[test]
    fn period_between__several_periods__finish_finish__first_period() {
        assert_eq!(config().periods_between(&date(14), &date(22)).unwrap(), 3)
    }

    #[test]
    fn period_between__several_periods__finish_finish__not_first_period() {
        assert_eq!(config().periods_between(&date(18), &date(22)).unwrap(), 2)
    }

    #[test]
    fn period_between__several_periods__middle_to_other_period_middle__first_period() {
        assert_eq!(config().periods_between(&date(13), &date(20)).unwrap(), 3)
    }

    #[test]
    fn period_between__several_periods__middle_to_other_period_middle__not_first_period() {
        assert_eq!(config().periods_between(&date(16), &date(22)).unwrap(), 2)
    }

    #[test]
    fn period_between__several_periods__middle_to_same_period_middle__first_period() {
        assert_eq!(config().periods_between(&date(12), &date(13)).unwrap(), 1)
    }

    #[test]
    fn period_between__several_periods__middle_to_same_period_middle__not_first_period() {
        assert_eq!(config().periods_between(&date(20), &date(21)).unwrap(), 1)
    }

    #[test]
    fn period_between__before_period_config_start__start_date() {
        assert_eq!(
            config().periods_between(&date(9), &date(21)).unwrap_err(),
            "Start date is before PeriodsConfiguration's start"
        )
    }

    #[test]
    fn period_between__before_period_config_start__both_date() {
        assert_eq!(
            config().periods_between(&date(7), &date(9)).unwrap_err(),
            "Dates before PeriodsConfiguration's start"
        )
    }

    #[test]
    fn period_between__start_date_after_end_date() {
        assert_eq!(
            config().periods_between(&date(21), &date(20)).unwrap_err(),
            "Start date is after end date"
        )
    }

    #[test]
    fn period_for_date__first_period__before_first_date() {
        assert_eq!(
            config().period_for_date(&date(10)).unwrap_err(),
            "Date is before PeriodsConfiguration's start"
        )
    }
    fn first_period() -> Period {
        return Period {
            start_date: date(11),
            end_date: date(14),
        };
    }

    #[test]
    fn period_for_date__first_period__first_day() {
        assert_eq!(config().period_for_date(&date(11)).unwrap(), first_period())
    }

    #[test]
    fn period_for_date__first_period__middle_day() {
        assert_eq!(config().period_for_date(&date(12)).unwrap(), first_period())
    }

    #[test]
    fn period_for_date__first_period__last_day() {
        assert_eq!(config().period_for_date(&date(14)).unwrap(), first_period())
    }

    fn second_period() -> Period {
        return Period {
            start_date: date(15),
            end_date: date(18),
        };
    }

    #[test]
    fn period_for_date__not_first_period__first_day() {
        assert_eq!(
            config().period_for_date(&date(15)).unwrap(),
            second_period()
        )
    }

    #[test]
    fn period_for_date__not_first_period__middle_day() {
        assert_eq!(
            config().period_for_date(&date(17)).unwrap(),
            second_period()
        )
    }

    #[test]
    fn period_for_date__not_first_period__last_day() {
        assert_eq!(
            config().period_for_date(&date(18)).unwrap(),
            second_period()
        )
    }
}
