use crate::period::periods::Period;
use crate::period::periods_configuration::PeriodsConfiguration;
use chrono::{Days, NaiveDate};
use serde::Deserialize;

pub type PeriodNumber = u16;
#[derive(Deserialize)]
pub struct FixedLengthPeriodConfiguration {
    start_date: NaiveDate,
    period_in_days: u8,
}

pub struct ErrorStartBeforePeriodConfiguration;

impl FixedLengthPeriodConfiguration {
    fn period_number_for_date(
        &self,
        date: &NaiveDate,
    ) -> Result<PeriodNumber, ErrorStartBeforePeriodConfiguration> {
        if date < &self.start_date {
            return Err(ErrorStartBeforePeriodConfiguration {});
        }
        let days_since_start = (*date - self.start_date).num_days() as u16; // u16: Up to 179 years
        return Ok((days_since_start / self.period_in_days as u16) as PeriodNumber);
    }
}

impl PeriodsConfiguration for FixedLengthPeriodConfiguration {
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

    fn number_of_periods_between(
        &self,
        start: &NaiveDate,
        end: &NaiveDate,
    ) -> Result<PeriodNumber, String> {
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

        // +1 because we return 1 if both dates are in the same period, 2 is they are in two contiguous period
        return Ok((end_period_number + 1) - start_period_number);
    }

    fn periods_between(&self, start: &NaiveDate, end: &NaiveDate) -> Result<Vec<Period>, String> {
        todo!()
    }
}

#[allow(non_snake_case)]
#[cfg(test)]
mod tests {
    use super::{FixedLengthPeriodConfiguration, PeriodsConfiguration};
    use crate::period::periods::Period;
    use chrono::NaiveDate;

    fn date(day_of_month: u32) -> NaiveDate {
        return NaiveDate::from_ymd_opt(2023, 04, day_of_month).unwrap();
    }

    fn config() -> FixedLengthPeriodConfiguration {
        return FixedLengthPeriodConfiguration {
            start_date: date(11),
            period_in_days: 4,
        };
    }

    #[test]
    fn period_between__length_of_period__first_period() {
        assert_eq!(
            config()
                .number_of_periods_between(&date(11), &date(14))
                .unwrap(),
            1
        )
    }

    #[test]
    fn period_between__length_of_period__not_first_period() {
        assert_eq!(
            config()
                .number_of_periods_between(&date(15), &date(18))
                .unwrap(),
            1
        )
    }

    #[test]
    fn period_between__several_periods__start_finish__first_period() {
        assert_eq!(
            config()
                .number_of_periods_between(&date(11), &date(22))
                .unwrap(),
            3
        )
    }

    #[test]
    fn period_between__several_periods__start_finish__not_first_period() {
        assert_eq!(
            config()
                .number_of_periods_between(&date(15), &date(22))
                .unwrap(),
            2
        )
    }

    #[test]
    fn period_between__several_periods__start_start__first_period() {
        assert_eq!(
            config()
                .number_of_periods_between(&date(11), &date(23))
                .unwrap(),
            4
        )
    }

    #[test]
    fn period_between__several_periods__start_start__not_first_period() {
        assert_eq!(
            config()
                .number_of_periods_between(&date(15), &date(23))
                .unwrap(),
            3
        )
    }

    #[test]
    fn period_between__several_periods__finish_finish__first_period() {
        assert_eq!(
            config()
                .number_of_periods_between(&date(14), &date(22))
                .unwrap(),
            3
        )
    }

    #[test]
    fn period_between__several_periods__finish_finish__not_first_period() {
        assert_eq!(
            config()
                .number_of_periods_between(&date(18), &date(22))
                .unwrap(),
            2
        )
    }

    #[test]
    fn period_between__several_periods__middle_to_other_period_middle__first_period() {
        assert_eq!(
            config()
                .number_of_periods_between(&date(13), &date(20))
                .unwrap(),
            3
        )
    }

    #[test]
    fn period_between__several_periods__middle_to_other_period_middle__not_first_period() {
        assert_eq!(
            config()
                .number_of_periods_between(&date(16), &date(22))
                .unwrap(),
            2
        )
    }

    #[test]
    fn period_between__several_periods__middle_to_same_period_middle__first_period() {
        assert_eq!(
            config()
                .number_of_periods_between(&date(12), &date(13))
                .unwrap(),
            1
        )
    }

    #[test]
    fn period_between__several_periods__middle_to_same_period_middle__not_first_period() {
        assert_eq!(
            config()
                .number_of_periods_between(&date(20), &date(21))
                .unwrap(),
            1
        )
    }

    #[test]
    fn period_between__regression_test__long_period() {
        let period_config = FixedLengthPeriodConfiguration {
            start_date: NaiveDate::from_ymd_opt(2024, 4, 27).unwrap(),
            period_in_days: 28,
        };
        assert_eq!(
            period_config
                .number_of_periods_between(
                    &NaiveDate::from_ymd_opt(2024, 5, 1).unwrap(),
                    &NaiveDate::from_ymd_opt(2067, 8, 27).unwrap(),
                )
                .unwrap(),
            566
        )
    }

    #[test]
    fn period_between__before_period_config_start__start_date() {
        assert_eq!(
            config()
                .number_of_periods_between(&date(9), &date(21))
                .unwrap_err(),
            "Start date is before PeriodsConfiguration's start"
        )
    }

    #[test]
    fn period_between__before_period_config_start__both_date() {
        assert_eq!(
            config()
                .number_of_periods_between(&date(7), &date(9))
                .unwrap_err(),
            "Dates before PeriodsConfiguration's start"
        )
    }

    #[test]
    fn period_between__start_date_after_end_date() {
        assert_eq!(
            config()
                .number_of_periods_between(&date(21), &date(20))
                .unwrap_err(),
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
