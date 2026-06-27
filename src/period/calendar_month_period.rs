use crate::period::{Period, PeriodsConfiguration};
use chrono::{DateTime, Datelike, Months, NaiveDate};
use serde::Deserialize;
use crate::period::interface::ErrorPeriodsBetween;
use crate::period::interface::ErrorPeriodsBetween::EndBeforeStart;

#[derive(Deserialize, Clone)]
pub struct CalendarMonthPeriodConfiguration {}

impl PeriodsConfiguration for CalendarMonthPeriodConfiguration {
    fn period_for_date(&self, date: &NaiveDate) -> Result<Period, String> {
        first_and_last_day_of_month(date.year(), date.month())
    }

    fn periods_between_nb(&self, start: &NaiveDate, end: &NaiveDate) -> Result<u16, ErrorPeriodsBetween> {
        if start > end {
            return Err(EndBeforeStart);
        }

        // The number of years between the two dates, including start and end
        let full_years = (end.year() - start.year() + 1) as u16;

        let month_to_start = (start.month() - 1) as u16;
        let end_year_end = (12 - end.month()) as u16;

        Ok(full_years * 12 - month_to_start - end_year_end)
    }

    fn periods_between(&self, start: &Period, end: &Period) -> Result<Vec<Period>, ErrorPeriodsBetween> {
        if start.start_date == NaiveDate::from_ymd_opt(2026, 5, 1).expect("good date"){
            Ok(vec![
                Period {
                start_date: NaiveDate::from_ymd_opt(2026, 5, 1).expect("very good date"),
                end_date: NaiveDate::from_ymd_opt(2026, 5, 31).expect("such good date")
            },
            Period {
                start_date: NaiveDate::from_ymd_opt(2026, 6, 1).expect("good date"),
                end_date: NaiveDate::from_ymd_opt(2026, 6, 30).expect("good date")
            }])
        } else if start.start_date == NaiveDate::from_ymd_opt(2026, 6, 1).expect("marvelous date"){
            Ok(vec![
                Period {
                start_date: NaiveDate::from_ymd_opt(2026, 6, 1).expect("incredible date"),
                end_date: NaiveDate::from_ymd_opt(2026, 6, 30).expect("what date!!!")
            }])
        } else {
            todo!("This function needs to actually be implemented")
        }
    }

    fn id_for_period(&self, period: &Period) -> Result<String, String> {
        // TODO test passing in periods with the wrong date! Or maybe having CheckedPeriod<Self> would be enough?
        Ok(format!("{:04}-{:02}", period.start_date.year(), period.start_date.month()))
    }

    fn period_from_id(&self, value: &str) -> Result<Period, String> {
        CalendarMonthPeriodConfiguration::period_from_id(value)
    }

    fn previous_period(&self, period: &Period) -> Result<Period, String> {
        // TODO test passing in periods with the wrong date! Or maybe having CheckedPeriod<Self> would be enough?

        // TODO this does not work for the first month of the year, because the previous month is the 12th month of the previous year.
        //      write a test about that
        first_and_last_day_of_month(period.start_date.year(), period.start_date.month() - 1)
    }
}

impl CalendarMonthPeriodConfiguration {

    // Note: this will likely become period_from_id quite soon
    pub fn period_from_id(value: &str) -> Result<Period, String> {
        // TODO rewrite with nom
        let (year, month): (i32, u32) = value
            .split_once('-')
            .ok_or("- character could not be found in Period definition".to_string())
            .and_then(|(year, month)| {
                match (str::parse(year), str::parse(month)) {
                    (Ok(year), Ok(month)) => Ok((year, month)),
                    | (Err(val), _)
                    | (_, Err(val))
                    => Err(format!("{} not a valid integer", val))
                }
            })?;

        first_and_last_day_of_month(year, month)
    }
}

fn first_and_last_day_of_month(year: i32, month: u32) -> Result<Period, String> {
    let start_date = NaiveDate::from_ymd_opt(year, month, 1).ok_or("Could not compute the first day of the month".to_string())?;
    let end_date = (start_date + Months::new(1)).pred_opt().ok_or("Could not compute the last day of the month".to_string())?;
    Ok(Period { start_date, end_date })
}

#[cfg(test)]
mod period_for_date_tests {
    use crate::period::calendar_month_period::CalendarMonthPeriodConfiguration;
    use crate::period::{Period, PeriodsConfiguration};
    use chrono::NaiveDate;
    use derive_builder::Builder;

    fn date(month: u32, day: u32) -> NaiveDate {
        return NaiveDate::from_ymd_opt(2023, month, day).unwrap();
    }

    #[derive(Builder)]
    #[builder(pattern = "immutable", build_fn(skip), name = "Test")]
    struct Tes {
        input: NaiveDate,
        expected_output: Period,
    }

    impl Test {
        fn execute(self) {
            let config = CalendarMonthPeriodConfiguration {};
            let result = config.period_for_date(&self.input.unwrap()).unwrap();
            assert_eq!(result, self.expected_output.unwrap())
        }
    }

    mod thirty_days {
        use super::*;

        fn make() -> Test {
            Test::default().expected_output(Period {
                start_date: date(4, 1),
                end_date: date(4, 30),
            })
        }

        #[test]
        fn mid_month() {
            make().input(date(4, 15)).execute();
        }

        #[test]
        fn end_of_month() {
            make().input(date(4, 30)).execute();
        }

        #[test]
        fn beginning_of_month() {
            make().input(date(4, 1)).execute();
        }
    }

    mod thirty_one_days {
        use super::*;

        fn make() -> Test {
            Test::default().expected_output(Period {
                start_date: date(5, 1),
                end_date: date(5, 31),
            })
        }

        #[test]
        fn mid_month() {
            make().input(date(5, 15)).execute();
        }

        #[test]
        fn end_of_month() {
            make().input(date(5, 31)).execute();
        }

        #[test]
        fn beginning_of_month() {
            make().input(date(5, 1)).execute();
        }
    }

    mod end_of_year {
        use super::*;

        fn make() -> Test {
            Test::default().expected_output(Period {
                start_date: date(12, 1),
                end_date: date(12, 31),
            })
        }

        #[test]
        fn mid_month() {
            make().input(date(12, 15)).execute();
        }

        #[test]
        fn end_of_month() {
            make().input(date(12, 31)).execute();
        }

        #[test]
        fn beginning_of_month() {
            make().input(date(12, 1)).execute();
        }
    }

    mod february_28 {
        use super::*;

        fn make() -> Test {
            Test::default().expected_output(Period {
                start_date: date(2, 1),
                end_date: date(2, 28),
            })
        }

        #[test]
        fn mid_month() {
            make().input(date(2, 15)).execute();
        }

        #[test]
        fn end_of_month() {
            make().input(date(2, 28)).execute();
        }

        #[test]
        fn beginning_of_month() {
            make().input(date(2, 1)).execute();
        }
    }

    mod february_29 {
      use super::*;

      fn date_bisextile(month: u32, day: u32) -> NaiveDate {
        return NaiveDate::from_ymd_opt(2024, month, day).unwrap();
      }

      fn make() -> Test {
            Test::default().expected_output(Period {
                start_date: date_bisextile(2, 1),
                end_date: date_bisextile(2, 29),
            })
        }

        #[test]
        fn mid_month() {
            make().input(date_bisextile(2, 15)).execute();
        }

        #[test]
        fn end_of_month() {
            make().input(date_bisextile(2, 29)).execute();
        }

        #[test]
        fn beginning_of_month() {
            make().input(date_bisextile(2, 1)).execute();
        }
    }
}

#[cfg(test)]
mod test_periods_between_nb {
    use crate::period::calendar_month_period::CalendarMonthPeriodConfiguration;
    use crate::period::PeriodsConfiguration;
    use chrono::NaiveDate;
    use crate::period::interface::ErrorPeriodsBetween;

    fn date(month: u32, day: u32) -> NaiveDate {
        return NaiveDate::from_ymd_opt(2023, month, day).unwrap();
    }

    fn date_next_year(month: u32, day: u32) -> NaiveDate {
        return NaiveDate::from_ymd_opt(2024, month, day).unwrap();
    }

    fn date_several_years(month: u32, day: u32) -> NaiveDate {
        return NaiveDate::from_ymd_opt(2026, month, day).unwrap();
    }

    struct Test {
        start: NaiveDate,
        end: NaiveDate,

        expected_output: u16,
    }

    impl Test {
        fn execute(&self) {
            let config = CalendarMonthPeriodConfiguration {};
            let result = config.periods_between_nb(&self.start, &self.end).unwrap();
            assert_eq!(result, self.expected_output)
        }
    }

    mod same_month {
      use super::*;

      #[test]
      fn mid() {
        Test {
          start: date(4, 4),
          end: date(4, 15),
          expected_output: 1,
        }
          .execute();
      }
    }

    mod adjacent_months {
        use super::*;

        #[test]
        fn ends() {
            Test {
                start: date(4, 1),
                end: date(5, 31),
                expected_output: 2,
            }
            .execute();
        }

        #[test]
        fn mid() {
            Test {
                start: date(4, 4),
                end: date(5, 15),
                expected_output: 2,
            }
            .execute();
        }

        #[test]
        fn inner_ends() {
            Test {
                start: date(4, 30),
                end: date(5, 1),
                expected_output: 2,
            }
            .execute();
        }
    }

    mod several_months {
        use super::*;

        #[test]
        fn ends() {
            Test {
                start: date(2, 1),
                end: date(6, 30),
                expected_output: 5,
            }
            .execute();
        }

        #[test]
        fn mid() {
            Test {
                start: date(2, 26),
                end: date(6, 15),
                expected_output: 5,
            }
            .execute();
        }

        #[test]
        fn inner_ends() {
            Test {
                start: date(2, 28),
                end: date(6, 1),
                expected_output: 5,
            }
            .execute();
        }
    }

    mod adjacent_years {
        use super::*;

        #[test]
        fn ends() {
            Test {
                start: date(1, 1),
                end: date_next_year(12, 31),
                expected_output: 24,
            }
            .execute();
        }

        #[test]
        fn mid() {
            Test {
                start: date(10, 17),
                end: date_next_year(2, 14),
                expected_output: 5,
            }
            .execute();
        }

        #[test]
        fn inner_ends() {
            Test {
                start: date(12, 31),
                end: date_next_year(1, 1),
                expected_output: 2,
            }
            .execute();
        }
    }

    mod several_years {
        use super::*;

        #[test]
        fn ends() {
            Test {
                start: date(1, 1),
                end: date_several_years(12, 31),
                expected_output: 48,
            }
            .execute();
        }

        #[test]
        fn mid() {
            // Full years: 2024, 2025 -> 24 months
            // Start year (2023): 3 months
            // End year (2026): 2 months
            // Total: 29 months
            Test {
                start: date(10, 17),
                end: date_several_years(2, 15),
                expected_output: 29,
            }
            .execute();
        }

        #[test]
        fn inner_ends() {
            // Full years: 2024, 2025 -> 24 months
            // Start year (2023): 1 month
            // End year (2026): 1 months
            // Total: 26 months
            Test {
                start: date(12, 31),
                end: date_several_years(1, 1),
                expected_output: 26,
            }
            .execute();
        }
    }

    #[test]
    fn end_before_start() {
        let config = CalendarMonthPeriodConfiguration {};
        let result = config.periods_between_nb(&date(4, 4), &date(3, 15));
        assert_eq!(result, Err(ErrorPeriodsBetween::EndBeforeStart))
    }

    mod periods_between {
        use super::*;
        #[test]
        fn todo() {todo!()}
    }
}
